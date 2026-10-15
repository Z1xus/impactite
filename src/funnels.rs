use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use rusqlite::params;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::accounts::{Viewer, Web, fail, internal};
use crate::app::App;
use crate::shares::Reader;
use crate::stats::{self, day_number};

pub fn routes() -> Router<Arc<Web>> {
	Router::new()
		.route("/api/app/sites/{domain}/funnels", get(list).post(add))
		.route("/api/app/sites/{domain}/funnels/{id}", delete(remove))
		.route("/api/app/funnel", post(compute))
}

#[derive(Deserialize)]
struct NewFunnel {
	name: String,
	steps: Vec<i64>,
}

#[derive(Deserialize)]
pub struct Request {
	site_id: String,
	funnel: i64,
	date_range: Value,
	#[serde(default)]
	filters: Vec<Value>,
}

fn site(web: &Web, user: i64, domain: &str, write: bool) -> Option<u64> {
	let role = web.app.site_role(user, domain).ok().flatten()?;
	if write && role == "viewer" {
		return None;
	}
	web.app.site(domain).ok().flatten().map(|site| site.id)
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
	Json(body): Json<NewFunnel>,
) -> Response {
	let Some(site) = site(&web, viewer.user, &domain, true) else {
		return fail(StatusCode::FORBIDDEN, "You can't change funnels here.");
	};
	let goals = web.app.goals(site).unwrap_or_default();
	let known = body
		.steps
		.iter()
		.all(|id| goals.iter().any(|goal| goal.id == *id));
	let name = body.name.trim();
	if name.is_empty() || !(2..=8).contains(&body.steps.len()) || !known {
		return fail(
			StatusCode::BAD_REQUEST,
			"Name the funnel and pick 2 to 8 goals.",
		);
	}
	match web
		.app
		.add_funnel(site, name, &json!(body.steps).to_string())
	{
		Ok(()) => respond(&web, site),
		Err(err) => internal(err),
	}
}

async fn remove(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path((domain, id)): Path<(String, i64)>,
) -> Response {
	let Some(site) = site(&web, viewer.user, &domain, true) else {
		return fail(StatusCode::FORBIDDEN, "You can't change funnels here.");
	};
	match web.app.remove_funnel(site, id) {
		Ok(()) => respond(&web, site),
		Err(err) => internal(err),
	}
}

async fn compute(
	State(web): State<Arc<Web>>,
	reader: Reader,
	Json(body): Json<Request>,
) -> Response {
	if !reader.reads(&web, &body.site_id) {
		return fail(StatusCode::NOT_FOUND, "Site not found.");
	}
	let result = (|| -> Result<Value, String> {
		let period = stats::period(&web.stats, &body.site_id, &body.date_range, false)?;
		let goals = web
			.app
			.goals(period.site.id)
			.map_err(|err| err.to_string())?;
		let funnel = web
			.app
			.funnels(period.site.id)
			.map_err(|err| err.to_string())?
			.into_iter()
			.find(|funnel| funnel["id"] == body.funnel)
			.ok_or("Funnel not found.")?;
		let steps: Vec<_> = funnel["steps"]
			.as_array()
			.into_iter()
			.flatten()
			.filter_map(|id| goals.iter().find(|goal| Some(goal.id) == id.as_i64()))
			.collect();
		let mut filters = Vec::new();
		for raw in &body.filters {
			for (filter, from_goal) in stats::filters(raw, &goals)? {
				if !from_goal {
					filters.push(filter);
				}
			}
		}
		let visitors = web
			.stats
			.db
			.funnel(
				period.site.id,
				day_number(period.from)..=day_number(period.to),
				&filters,
				&steps.iter().map(|goal| goal.filters()).collect::<Vec<_>>(),
			)
			.map_err(|err| err.to_string())?;
		Ok(json!({
			"name": funnel["name"],
			"steps": steps
				.iter()
				.zip(visitors)
				.map(|(goal, visitors)| json!({ "name": goal.name, "visitors": visitors }))
				.collect::<Vec<_>>()
		}))
	})();
	match result {
		Ok(value) => Json(value).into_response(),
		Err(message) => fail(StatusCode::BAD_REQUEST, &message),
	}
}

fn respond(web: &Web, site: u64) -> Response {
	match web.app.funnels(site) {
		Ok(funnels) => Json(json!({ "funnels": funnels })).into_response(),
		Err(err) => internal(err),
	}
}

impl App {
	fn funnels(&self, site: u64) -> rusqlite::Result<Vec<Value>> {
		let conn = self.conn();
		let mut stmt =
			conn.prepare("select id, name, steps from funnels where site = ?1 order by name")?;
		stmt.query_map([site as i64], |row| {
			let steps: String = row.get(2)?;
			Ok(json!({
				"id": row.get::<_, i64>(0)?,
				"name": row.get::<_, String>(1)?,
				"steps": serde_json::from_str::<Value>(&steps).unwrap_or_default(),
			}))
		})?
		.collect()
	}

	fn add_funnel(&self, site: u64, name: &str, steps: &str) -> rusqlite::Result<()> {
		self.conn().execute(
			"insert into funnels (site, name, steps, created_at) values (?1, ?2, ?3, unixepoch())",
			params![site as i64, name, steps],
		)?;
		Ok(())
	}

	fn remove_funnel(&self, site: u64, id: i64) -> rusqlite::Result<()> {
		self.conn().execute(
			"delete from funnels where site = ?1 and id = ?2",
			params![site as i64, id],
		)?;
		Ok(())
	}
}
