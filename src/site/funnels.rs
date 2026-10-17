use std::sync::Arc;

use axum::extract::{Path, State};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use rusqlite::params;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::app::{App, Site};
use crate::auth::accounts::Viewer;
use crate::site::shares::Reader;
use crate::stats::{self, day_number};
use crate::web::{Error, Reply, Web};

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

async fn list(State(web): State<Arc<Web>>, reader: Reader, Path(domain): Path<String>) -> Reply {
	respond(&web, web.readable(&reader, &domain)?.id)
}

async fn add(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path(domain): Path<String>,
	Json(body): Json<NewFunnel>,
) -> Reply {
	let Site { id: site, .. } = web.editable(viewer.user, &domain)?;
	let goals = web.app.goals(site)?;
	let known = body
		.steps
		.iter()
		.all(|id| goals.iter().any(|goal| goal.id == *id));
	let name = body.name.trim();
	if name.is_empty() || !(2..=8).contains(&body.steps.len()) || !known {
		return Err(Error::bad("Name the funnel and pick 2 to 8 goals."));
	}
	web.app
		.add_funnel(site, name, &json!(body.steps).to_string())?;
	respond(&web, site)
}

async fn remove(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path((domain, id)): Path<(String, i64)>,
) -> Reply {
	let Site { id: site, .. } = web.editable(viewer.user, &domain)?;
	web.app.remove_funnel(site, id)?;
	respond(&web, site)
}

async fn compute(State(web): State<Arc<Web>>, reader: Reader, Json(body): Json<Request>) -> Reply {
	if !reader.reads(&web, &body.site_id) {
		return Err(Error::not_found("Site not found."));
	}
	let bad = |message: String| Error::bad(message);
	let period = stats::period(&web.stats, &body.site_id, &body.date_range, false).map_err(bad)?;
	let goals = web.app.goals(period.site.id)?;
	let funnel = web
		.app
		.funnels(period.site.id)?
		.into_iter()
		.find(|funnel| funnel["id"] == body.funnel)
		.ok_or_else(|| Error::bad("Funnel not found."))?;
	let steps: Vec<_> = funnel["steps"]
		.as_array()
		.into_iter()
		.flatten()
		.filter_map(|id| goals.iter().find(|goal| Some(goal.id) == id.as_i64()))
		.collect();
	let mut filters = Vec::new();
	for raw in &body.filters {
		for (filter, from_goal) in stats::filters(raw, &goals).map_err(bad)? {
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
		.map_err(|err| bad(err.to_string()))?;
	Ok(Json(json!({
		"name": funnel["name"],
		"steps": steps
			.iter()
			.zip(visitors)
			.map(|(goal, visitors)| json!({ "name": goal.name, "visitors": visitors }))
			.collect::<Vec<_>>()
	})))
}

fn respond(web: &Web, site: u64) -> Reply {
	Ok(Json(json!({ "funnels": web.app.funnels(site)? })))
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

	// a funnel loses a removed goal as a step, and goes when fewer than two steps are left
	pub(crate) fn forget_goal(&self, site: u64, goal: i64) -> rusqlite::Result<()> {
		for funnel in self.funnels(site)? {
			let steps: Vec<i64> = funnel["steps"]
				.as_array()
				.into_iter()
				.flatten()
				.filter_map(Value::as_i64)
				.collect();
			if !steps.contains(&goal) {
				continue;
			}
			let kept: Vec<_> = steps.into_iter().filter(|step| *step != goal).collect();
			let id = funnel["id"].as_i64().unwrap_or_default();
			match kept.len() {
				0..=1 => self.remove_funnel(site, id)?,
				_ => {
					self.conn().execute(
						"update funnels set steps = ?3 where site = ?1 and id = ?2",
						params![site as i64, id, json!(kept).to_string()],
					)?;
				}
			}
		}
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
