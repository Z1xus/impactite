use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use rusqlite::params;

use crate::app::App;
use crate::site::shares::Reader;
use crate::web::{Web, fail, internal};

pub fn routes() -> Router<Arc<Web>> {
	Router::new().route("/api/app/sites/{domain}/titles", get(list))
}

async fn list(State(web): State<Arc<Web>>, reader: Reader, Path(domain): Path<String>) -> Response {
	let site = match web.app.site(&domain) {
		Ok(Some(site)) if reader.reads(&web, &domain) => site,
		_ => return fail(StatusCode::NOT_FOUND, "Site not found."),
	};
	match web.app.titles(site.id) {
		Ok(titles) => Json(titles).into_response(),
		Err(err) => internal(err),
	}
}

impl App {
	pub fn set_title(&self, site: u64, path: &str, title: &str) -> rusqlite::Result<()> {
		self.conn().execute(
			"insert into titles (site, path, title) values (?1, ?2, ?3)
			on conflict (site, path) do update set title = excluded.title",
			params![site as i64, path, title],
		)?;
		Ok(())
	}

	pub fn titles(&self, site: u64) -> rusqlite::Result<HashMap<String, String>> {
		let conn = self.conn();
		let mut stmt = conn.prepare("select path, title from titles where site = ?1")?;
		stmt.query_map([site as i64], |row| Ok((row.get(0)?, row.get(1)?)))?
			.collect()
	}
}
