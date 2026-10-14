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

use crate::accounts::{Viewer, Web, fail};
use crate::app::App;

#[derive(Debug, Clone)]
pub struct Goal {
	pub id: i64,
	pub page: bool,
	pub value: String,
	pub name: String,
}

impl Goal {
	pub fn filters(&self) -> Vec<Filter> {
		let is = |dim: Dim, value: &str, op: Op| Filter {
			field: Field::Dim(dim),
			op,
			values: vec![value.to_owned()],
		};
		match self.page {
			true => vec![
				is(Dim::Name, "pageview", Op::Is),
				is(Dim::Page, &self.value, Op::Matches),
			],
			false => vec![is(Dim::Name, &self.value, Op::Is)],
		}
	}
}

pub fn routes() -> Router<Arc<Web>> {
	Router::new()
		.route("/api/app/sites/{domain}/goals", get(list).post(add))
		.route("/api/app/sites/{domain}/goals/{id}", delete(remove))
}

#[derive(Deserialize)]
struct NewGoal {
	page: bool,
	value: String,
}

async fn list(State(web): State<Arc<Web>>, viewer: Viewer, Path(domain): Path<String>) -> Response {
	let Some(site) = allowed(&web, viewer.user, &domain, false) else {
		return fail(StatusCode::NOT_FOUND, "Site not found.");
	};
	respond(&web, site)
}

async fn add(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path(domain): Path<String>,
	Json(body): Json<NewGoal>,
) -> Response {
	let Some(site) = allowed(&web, viewer.user, &domain, true) else {
		return fail(StatusCode::FORBIDDEN, "You can't change goals here.");
	};
	let value = body.value.trim();
	if value.is_empty() || value.len() > 300 || (body.page && !value.starts_with('/')) {
		return fail(
			StatusCode::BAD_REQUEST,
			"Use an event name or a path like /pricing.",
		);
	}
	let name = match body.page {
		true => format!("Visit {value}"),
		false => value.to_owned(),
	};
	match web.app.add_goal(site, body.page, value, &name) {
		Ok(()) => respond(&web, site),
		Err(_) => fail(StatusCode::CONFLICT, "This goal already exists."),
	}
}

async fn remove(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path((domain, id)): Path<(String, i64)>,
) -> Response {
	let Some(site) = allowed(&web, viewer.user, &domain, true) else {
		return fail(StatusCode::FORBIDDEN, "You can't change goals here.");
	};
	match web.app.remove_goal(site, id) {
		Ok(()) => respond(&web, site),
		Err(_) => fail(StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong."),
	}
}

fn allowed(web: &Web, user: i64, domain: &str, write: bool) -> Option<u64> {
	let role = web.app.site_role(user, domain).ok().flatten()?;
	if write && role == "viewer" {
		return None;
	}
	web.app.site(domain).ok().flatten().map(|site| site.id)
}

fn respond(web: &Web, site: u64) -> Response {
	match web.app.goals(site) {
		Ok(goals) => Json(json!({
			"goals": goals
				.iter()
				.map(|goal| json!({ "id": goal.id, "page": goal.page, "value": goal.value, "name": goal.name }))
				.collect::<Vec<_>>()
		}))
		.into_response(),
		Err(_) => fail(StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong."),
	}
}

impl App {
	pub fn goals(&self, site: u64) -> rusqlite::Result<Vec<Goal>> {
		let conn = self.conn();
		let mut stmt =
			conn.prepare("select id, page, value, name from goals where site = ?1 order by id")?;
		stmt.query_map([site as i64], |row| {
			Ok(Goal {
				id: row.get(0)?,
				page: row.get(1)?,
				value: row.get(2)?,
				name: row.get(3)?,
			})
		})?
		.collect()
	}

	pub fn add_goal(&self, site: u64, page: bool, value: &str, name: &str) -> rusqlite::Result<()> {
		self.conn().execute(
			"insert into goals (site, page, value, name, created_at) values (?1, ?2, ?3, ?4, unixepoch())",
			params![site as i64, page, value, name],
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
