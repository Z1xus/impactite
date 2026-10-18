use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{delete, get};
use axum::{Json, Router};
use crater::{Dim, Field, Filter, Op};
use rusqlite::params;
use serde::Deserialize;
use serde_json::json;

use crate::app::{App, Site};
use crate::auth::accounts::Viewer;
use crate::site::shares::Reader;
use crate::stats::day_number;
use crate::web::{Error, Reply, Web};

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
) -> Reply {
	suggested(&web, &web.editable(viewer.user, &domain)?)
}

async fn ignore(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path(domain): Path<String>,
	Json(body): Json<Ignore>,
) -> Reply {
	let site = web.editable(viewer.user, &domain)?;
	web.app.ignore_event(site.id, &body.name)?;
	suggested(&web, &site)
}

fn suggested(web: &Web, site: &Site) -> Reply {
	let goals = web.app.goals(site.id)?;
	let ignored = web.app.ignored_events(site.id)?;
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
	let rows = report
		.map_err(|err| {
			tracing::error!(%err, "goal suggestions");
			Error::new(StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong.")
		})?
		.rows;
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
	Ok(Json(json!({ "events": events })))
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

async fn list(State(web): State<Arc<Web>>, reader: Reader, Path(domain): Path<String>) -> Reply {
	respond(&web, web.readable(&reader, &domain)?.id)
}

async fn add(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path(domain): Path<String>,
	Json(body): Json<NewGoal>,
) -> Reply {
	let Site { id: site, .. } = web.editable(viewer.user, &domain)?;
	let value = body.value.trim();
	if value.is_empty() || value.len() > 300 || (body.page && !value.starts_with('/')) {
		return Err(Error::bad("Use an event name or a path like /pricing."));
	}
	let scroll = if body.page { body.scroll.min(100) } else { 0 };
	let currency = body.currency.trim().to_uppercase();
	let lettered = currency.len() == 3 && currency.bytes().all(|b| b.is_ascii_alphabetic());
	if (!currency.is_empty() && !lettered) || (body.page && !currency.is_empty()) {
		return Err(Error::bad(
			"Revenue goals use an event and a 3 letter currency.",
		));
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
	web.app
		.add_goal(site, body.page, value, &name, scroll, &currency)
		.map_err(|_| Error::conflict("This goal already exists."))?;
	respond(&web, site)
}

async fn rename(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path((domain, id)): Path<(String, i64)>,
	Json(body): Json<Rename>,
) -> Reply {
	let Site { id: site, .. } = web.editable(viewer.user, &domain)?;
	let name = body.name.trim();
	if name.is_empty() || name.len() > 120 {
		return Err(Error::bad("Name the goal."));
	}
	web.app
		.rename_goal(site, id, name)
		.map_err(|_| Error::conflict("Another goal has this name."))?;
	respond(&web, site)
}

async fn remove(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path((domain, id)): Path<(String, i64)>,
) -> Reply {
	let Site { id: site, .. } = web.editable(viewer.user, &domain)?;
	web.app.remove_goal(site, id)?;
	respond(&web, site)
}

fn respond(web: &Web, site: u64) -> Reply {
	Ok(Json(json!({
		"goals": web
			.app
			.goals(site)?
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
	})))
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
		self.forget_goal(site, id)
	}
}
