use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get};
use axum::{Json, Router};
use rusqlite::params;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::app::App;
use crate::auth::accounts::Viewer;
use crate::web::{Web, fail, internal};

pub fn routes() -> Router<Arc<Web>> {
	Router::new()
		.route("/api/app/sites/{domain}/segments", get(list).post(add))
		.route("/api/app/sites/{domain}/segments/{id}", delete(remove))
}

#[derive(Deserialize)]
struct NewSegment {
	name: String,
	filters: Vec<String>,
	shared: bool,
}

fn access(web: &Web, user: i64, domain: &str) -> Option<(u64, String)> {
	let role = web.app.site_role(user, domain).ok().flatten()?;
	let site = web.app.site(domain).ok().flatten()?;
	Some((site.id, role))
}

async fn list(State(web): State<Arc<Web>>, viewer: Viewer, Path(domain): Path<String>) -> Response {
	match access(&web, viewer.user, &domain) {
		Some((site, _)) => respond(&web, site, viewer.user),
		None => fail(StatusCode::NOT_FOUND, "Site not found."),
	}
}

async fn add(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path(domain): Path<String>,
	Json(body): Json<NewSegment>,
) -> Response {
	let Some((site, role)) = access(&web, viewer.user, &domain) else {
		return fail(StatusCode::NOT_FOUND, "Site not found.");
	};
	let name = body.name.trim();
	if name.is_empty() || name.len() > 80 || body.filters.is_empty() {
		return fail(
			StatusCode::BAD_REQUEST,
			"Name the segment and add a filter.",
		);
	}
	if body.shared && role == "viewer" {
		return fail(
			StatusCode::FORBIDDEN,
			"Viewers can only save personal segments.",
		);
	}
	let filters = json!(body.filters).to_string();
	match web
		.app
		.add_segment(site, viewer.user, name, &filters, body.shared)
	{
		Ok(()) => respond(&web, site, viewer.user),
		Err(err) => internal(err),
	}
}

async fn remove(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path((domain, id)): Path<(String, i64)>,
) -> Response {
	let Some((site, role)) = access(&web, viewer.user, &domain) else {
		return fail(StatusCode::NOT_FOUND, "Site not found.");
	};
	let manager = matches!(role.as_str(), "owner" | "admin");
	match web.app.remove_segment(site, id, viewer.user, manager) {
		Ok(()) => respond(&web, site, viewer.user),
		Err(err) => internal(err),
	}
}

fn respond(web: &Web, site: u64, user: i64) -> Response {
	match web.app.segments(site, user) {
		Ok(segments) => Json(json!({ "segments": segments })).into_response(),
		Err(err) => internal(err),
	}
}

impl App {
	fn segments(&self, site: u64, user: i64) -> rusqlite::Result<Vec<Value>> {
		let conn = self.conn();
		let mut stmt = conn.prepare(
			"select id, name, filters, shared, user = ?2 from segments
			where site = ?1 and (shared or user = ?2) order by name",
		)?;
		stmt.query_map(params![site as i64, user], |row| {
			let filters: String = row.get(2)?;
			Ok(json!({
				"id": row.get::<_, i64>(0)?,
				"name": row.get::<_, String>(1)?,
				"filters": serde_json::from_str::<Value>(&filters).unwrap_or_default(),
				"shared": row.get::<_, bool>(3)?,
				"mine": row.get::<_, bool>(4)?,
			}))
		})?
		.collect()
	}

	fn add_segment(
		&self,
		site: u64,
		user: i64,
		name: &str,
		filters: &str,
		shared: bool,
	) -> rusqlite::Result<()> {
		self.conn().execute(
			"insert into segments (site, user, name, filters, shared, created_at)
			values (?1, ?2, ?3, ?4, ?5, unixepoch())",
			params![site as i64, user, name, filters, shared],
		)?;
		Ok(())
	}

	fn remove_segment(&self, site: u64, id: i64, user: i64, manager: bool) -> rusqlite::Result<()> {
		self.conn().execute(
			"delete from segments where site = ?1 and id = ?2 and (user = ?3 or ?4)",
			params![site as i64, id, user, manager],
		)?;
		Ok(())
	}
}
