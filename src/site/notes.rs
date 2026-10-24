use std::sync::Arc;

use axum::extract::{Path, State};
use axum::routing::{delete, get};
use axum::{Json, Router};
use jiff::civil::Date;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::app::App;
use crate::auth::accounts::Viewer;
use crate::web::{Error, Reply, Web};

pub fn routes() -> Router<Arc<Web>> {
	Router::new()
		.route("/api/app/sites/{domain}/notes", get(list).post(add))
		.route("/api/app/sites/{domain}/notes/{id}", delete(remove))
}

#[derive(Serialize)]
pub struct Note {
	pub id: i64,
	pub day: String,
	pub text: String,
}

#[derive(Deserialize)]
struct NewNote {
	day: String,
	text: String,
}

pub fn checked(day: &str, text: &str) -> Result<(String, String), &'static str> {
	let day = day
		.parse::<Date>()
		.map_err(|_| "Use a date like 2026-10-24.")?;
	let text = text.trim();
	if text.is_empty() || text.chars().count() > 200 {
		return Err("Write a note of up to 200 characters.");
	}
	Ok((day.to_string(), text.to_owned()))
}

async fn list(State(web): State<Arc<Web>>, viewer: Viewer, Path(domain): Path<String>) -> Reply {
	if web.app.site_role(viewer.user, &domain)?.is_none() {
		return Err(Error::not_found("Site not found."));
	}
	let site = web
		.app
		.site(&domain)?
		.ok_or_else(|| Error::not_found("Site not found."))?;
	Ok(Json(json!({ "notes": web.app.site_notes(site.id)? })))
}

async fn add(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path(domain): Path<String>,
	Json(body): Json<NewNote>,
) -> Reply {
	let site = web.editable(viewer.user, &domain)?;
	let (day, text) = checked(&body.day, &body.text).map_err(Error::bad)?;
	web.app.add_site_note(site.id, &day, &text)?;
	Ok(Json(json!({ "notes": web.app.site_notes(site.id)? })))
}

async fn remove(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path((domain, id)): Path<(String, i64)>,
) -> Reply {
	let site = web.editable(viewer.user, &domain)?;
	web.app.conn().execute(
		"delete from notes where site = ?1 and id = ?2",
		params![site.id as i64, id],
	)?;
	Ok(Json(json!({ "notes": web.app.site_notes(site.id)? })))
}

impl App {
	pub fn add_site_note(&self, site: u64, day: &str, text: &str) -> rusqlite::Result<i64> {
		let conn = self.conn();
		conn.execute(
			"insert into notes (site, day, text, created_at) values (?1, ?2, ?3, unixepoch())",
			params![site as i64, day, text],
		)?;
		Ok(conn.last_insert_rowid())
	}

	pub fn site_notes(&self, site: u64) -> rusqlite::Result<Vec<Note>> {
		let conn = self.conn();
		let mut stmt =
			conn.prepare("select id, day, text from notes where site = ?1 order by day, id")?;
		stmt.query_map([site as i64], |row| {
			Ok(Note {
				id: row.get(0)?,
				day: row.get(1)?,
				text: row.get(2)?,
			})
		})?
		.collect()
	}
}
