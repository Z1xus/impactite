use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get};
use axum::{Json, Router};
use crater::{Dim, Field, Filter, Op};
use rusqlite::params;
use serde::Deserialize;
use serde_json::json;

use crate::accounts::{Viewer, Web, broken, fail, internal};
use crate::app::{App, Site};
use crate::shares::Reader;
use crate::stats::day_number;

#[derive(Debug, Clone)]
pub struct Goal {
	pub id: i64,
	pub page: bool,
	pub value: String,
	pub name: String,
	pub scroll: u32,
	pub currency: String,
}

impl Goal {
	pub fn filters(&self) -> Vec<Filter> {
		let is = |dim: Dim, value: &str, op: Op| Filter {
			field: Field::Dim(dim),
			op,
			values: vec![value.to_owned()],
		};
		match (self.page, self.scroll) {
			(true, 0) => vec![
				is(Dim::Name, "pageview", Op::Is),
				is(Dim::Page, &self.value, Op::Matches),
			],
			(true, scroll) => vec![
				is(Dim::Page, &self.value, Op::Matches),
				Filter {
					field: Field::Scroll,
					op: Op::Is,
					values: vec![scroll.to_string()],
				},
			],
			(false, _) if !self.currency.is_empty() => vec![
				is(Dim::Name, &self.value, Op::Is),
				Filter {
					field: Field::Currency,
					op: Op::Is,
					values: vec![self.currency.clone()],
				},
			],
			(false, _) => vec![is(Dim::Name, &self.value, Op::Is)],
		}
	}
}

pub fn routes() -> Router<Arc<Web>> {
	Router::new()
		.route("/api/app/sites/{domain}/goals", get(list).post(add))
		.route(
			"/api/app/sites/{domain}/goals/suggestions",
			get(suggestions).post(ignore),
		)
		.route(
			"/api/app/sites/{domain}/goals/{id}",
			delete(remove).patch(rename),
		)
}

#[derive(Deserialize)]
struct Ignore {
	name: String,
}

async fn suggestions(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path(domain): Path<String>,
) -> Response {
	match web.app.editable(viewer.user, &domain) {
		Some(site) => suggested(&web, &site),
		None => fail(StatusCode::FORBIDDEN, "You can't change this site."),
	}
}

async fn ignore(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path(domain): Path<String>,
	Json(body): Json<Ignore>,
) -> Response {
	let Some(site) = web.app.editable(viewer.user, &domain) else {
		return fail(StatusCode::FORBIDDEN, "You can't change this site.");
	};
	match web.app.ignore_event(site.id, &body.name) {
		Ok(()) => suggested(&web, &site),
		Err(err) => internal(err),
	}
}

// event names from the last 30 days that are neither goals nor ignored
fn suggested(web: &Web, site: &Site) -> Response {
	let (goals, ignored) = match (web.app.goals(site.id), web.app.ignored_events(site.id)) {
		(Ok(goals), Ok(ignored)) => (goals, ignored),
		(Err(err), _) | (_, Err(err)) => return internal(err),
	};
	let tz = jiff::tz::TimeZone::get(&site.timezone).unwrap_or(jiff::tz::TimeZone::UTC);
	let today = day_number(jiff::Timestamp::now().to_zoned(tz).date());
	let report = web.stats.db.query(&crater::Query {
		site: site.id,
		days: today - 29..=today,
		filters: Vec::new(),
		breakdown: Some(Field::Dim(Dim::Name)),
		limit: 50,
		daily: false,
		imports: false,
		since: None,
	});
	let rows = match report {
		Ok(report) => report.rows,
		Err(err) => {
			tracing::error!(%err, "goal suggestions");
			return broken();
		}
	};
	let events: Vec<_> = rows
		.iter()
		.filter(|(name, _)| {
			!matches!(&**name, "pageview" | "engagement")
				&& !goals.iter().any(|goal| !goal.page && goal.value == **name)
				&& !ignored.iter().any(|ignored| ignored == &**name)
		})
		.take(20)
		.map(|(name, metrics)| json!({ "name": &**name, "count": metrics.events, "revenue": metrics.revenue > 0 }))
		.collect();
	Json(json!({ "events": events })).into_response()
}

#[derive(Deserialize)]
struct NewGoal {
	page: bool,
	value: String,
	name: Option<String>,
	#[serde(default)]
	scroll: u32,
	#[serde(default)]
	currency: String,
}

#[derive(Deserialize)]
struct Rename {
	name: String,
}

async fn list(State(web): State<Arc<Web>>, reader: Reader, Path(domain): Path<String>) -> Response {
	match web.app.site(&domain) {
		Ok(Some(site)) if reader.reads(&web, &domain) => respond(&web, site.id),
		_ => fail(StatusCode::NOT_FOUND, "Site not found."),
	}
}

async fn add(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path(domain): Path<String>,
	Json(body): Json<NewGoal>,
) -> Response {
	let Some(Site { id: site, .. }) = web.app.editable(viewer.user, &domain) else {
		return fail(StatusCode::FORBIDDEN, "You can't change goals here.");
	};
	let value = body.value.trim();
	if value.is_empty() || value.len() > 300 || (body.page && !value.starts_with('/')) {
		return fail(
			StatusCode::BAD_REQUEST,
			"Use an event name or a path like /pricing.",
		);
	}
	let scroll = body.scroll.min(100);
	let currency = body.currency.trim().to_uppercase();
	if currency.len() > 3 || (body.page && !currency.is_empty()) {
		return fail(
			StatusCode::BAD_REQUEST,
			"Revenue goals use an event and a 3 letter currency.",
		);
	}
	let name = match body
		.name
		.as_deref()
		.map(str::trim)
		.filter(|name| !name.is_empty())
	{
		Some(name) => name.to_owned(),
		None if body.page && scroll > 0 => format!("Scroll {value} {scroll}%"),
		None if body.page => format!("Visit {value}"),
		None => value.to_owned(),
	};
	match web
		.app
		.add_goal(site, body.page, value, &name, scroll, &currency)
	{
		Ok(()) => respond(&web, site),
		Err(_) => fail(StatusCode::CONFLICT, "This goal already exists."),
	}
}

async fn rename(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path((domain, id)): Path<(String, i64)>,
	Json(body): Json<Rename>,
) -> Response {
	let Some(Site { id: site, .. }) = web.app.editable(viewer.user, &domain) else {
		return fail(StatusCode::FORBIDDEN, "You can't change goals here.");
	};
	let name = body.name.trim();
	if name.is_empty() || name.len() > 120 {
		return fail(StatusCode::BAD_REQUEST, "Name the goal.");
	}
	match web.app.rename_goal(site, id, name) {
		Ok(()) => respond(&web, site),
		Err(_) => fail(StatusCode::CONFLICT, "Another goal has this name."),
	}
}

async fn remove(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path((domain, id)): Path<(String, i64)>,
) -> Response {
	let Some(Site { id: site, .. }) = web.app.editable(viewer.user, &domain) else {
		return fail(StatusCode::FORBIDDEN, "You can't change goals here.");
	};
	match web.app.remove_goal(site, id) {
		Ok(()) => respond(&web, site),
		Err(err) => internal(err),
	}
}

fn respond(web: &Web, site: u64) -> Response {
	match web.app.goals(site) {
		Ok(goals) => Json(json!({
			"goals": goals
				.iter()
				.map(|goal| {
					json!({
						"id": goal.id,
						"page": goal.page,
						"value": goal.value,
						"name": goal.name,
						"scroll": goal.scroll,
						"currency": goal.currency,
					})
				})
				.collect::<Vec<_>>()
		}))
		.into_response(),
		Err(err) => internal(err),
	}
}

impl App {
	pub fn ignored_events(&self, site: u64) -> rusqlite::Result<Vec<String>> {
		let conn = self.conn();
		let mut stmt = conn.prepare("select name from ignored_events where site = ?1")?;
		stmt.query_map([site as i64], |row| row.get(0))?.collect()
	}

	pub fn ignore_event(&self, site: u64, name: &str) -> rusqlite::Result<()> {
		self.conn().execute(
			"insert or ignore into ignored_events (site, name) values (?1, ?2)",
			params![site as i64, name],
		)?;
		Ok(())
	}

	pub fn goals(&self, site: u64) -> rusqlite::Result<Vec<Goal>> {
		let conn = self.conn();
		let mut stmt = conn.prepare(
			"select id, page, value, name, scroll, currency from goals where site = ?1 order by id",
		)?;
		stmt.query_map([site as i64], |row| {
			Ok(Goal {
				id: row.get(0)?,
				page: row.get(1)?,
				value: row.get(2)?,
				name: row.get(3)?,
				scroll: row.get(4)?,
				currency: row.get(5)?,
			})
		})?
		.collect()
	}

	pub fn add_goal(
		&self,
		site: u64,
		page: bool,
		value: &str,
		name: &str,
		scroll: u32,
		currency: &str,
	) -> rusqlite::Result<()> {
		self.conn().execute(
			"insert into goals (site, page, value, name, scroll, currency, created_at)
			values (?1, ?2, ?3, ?4, ?5, ?6, unixepoch())",
			params![site as i64, page, value, name, scroll, currency],
		)?;
		Ok(())
	}

	pub fn rename_goal(&self, site: u64, id: i64, name: &str) -> rusqlite::Result<()> {
		self.conn().execute(
			"update goals set name = ?3 where site = ?1 and id = ?2",
			params![site as i64, id, name],
		)?;
		Ok(())
	}

	pub fn remove_goal(&self, site: u64, id: i64) -> rusqlite::Result<()> {
		self.conn().execute(
			"delete from goals where site = ?1 and id = ?2",
			params![site as i64, id],
		)?;
		Ok(())
	}
}
