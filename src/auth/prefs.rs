use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use rusqlite::{OptionalExtension, params};
use serde_json::Value;

use crate::app::App;
use crate::auth::accounts::Viewer;
use crate::web::{Error, Reply, Web};

pub fn routes() -> Router<Arc<Web>> {
	Router::new()
		.route("/api/app/prefs", get(all))
		.route("/api/app/prefs/{key}", get(read).put(write))
}

async fn all(State(web): State<Arc<Web>>, viewer: Viewer) -> Reply {
	Ok(Json(web.app.prefs(viewer.user)?))
}

async fn read(State(web): State<Arc<Web>>, viewer: Viewer, Path(key): Path<String>) -> Reply {
	Ok(Json(
		web.app.pref(viewer.user, &key)?.unwrap_or(Value::Null),
	))
}

async fn write(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path(key): Path<String>,
	Json(value): Json<Value>,
) -> Reply<StatusCode> {
	let text = value.to_string();
	if key.len() > 200 || text.len() > 16_384 {
		return Err(Error::new(
			StatusCode::PAYLOAD_TOO_LARGE,
			"This preference is too large.",
		));
	}
	web.app.set_pref(viewer.user, &key, &text)?;
	Ok(StatusCode::NO_CONTENT)
}

impl App {
	pub fn prefs(&self, user: i64) -> rusqlite::Result<Value> {
		let conn = self.conn();
		let mut statement = conn.prepare("select key, value from prefs where user = ?1")?;
		let rows = statement.query_map(params![user], |row| {
			Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
		})?;
		let mut all = serde_json::Map::new();
		for row in rows {
			let (key, text) = row?;
			if let Ok(value) = serde_json::from_str(&text) {
				all.insert(key, value);
			}
		}
		Ok(Value::Object(all))
	}

	fn pref(&self, user: i64, key: &str) -> rusqlite::Result<Option<Value>> {
		let text: Option<String> = self
			.conn()
			.query_row(
				"select value from prefs where user = ?1 and key = ?2",
				params![user, key],
				|row| row.get(0),
			)
			.optional()?;
		Ok(text.and_then(|text| serde_json::from_str(&text).ok()))
	}

	fn set_pref(&self, user: i64, key: &str, value: &str) -> rusqlite::Result<()> {
		self.conn().execute(
			"insert into prefs (user, key, value) values (?1, ?2, ?3)
			on conflict (user, key) do update set value = ?3",
			params![user, key, value],
		)?;
		Ok(())
	}
}
