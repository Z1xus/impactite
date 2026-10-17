use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};
use rusqlite::params;
use serde_json::json;

use crate::app::App;
use crate::site::shares::Reader;
use crate::web::{Reply, Web};

pub fn routes() -> Router<Arc<Web>> {
	Router::new().route("/api/app/sites/{domain}/titles", get(list))
}

async fn list(State(web): State<Arc<Web>>, reader: Reader, Path(domain): Path<String>) -> Reply {
	let site = web.readable(&reader, &domain)?;
	Ok(Json(json!(web.app.titles(site.id)?)))
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
