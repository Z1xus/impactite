use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use rusqlite::{OptionalExtension, params};
use serde_json::Value;

use crate::app::App;
use crate::auth::accounts::Viewer;
use crate::web::{Web, fail, internal};

pub fn routes() -> Router<Arc<Web>> {
	Router::new().route("/api/app/prefs/{key}", get(read).put(write))
}

async fn read(State(web): State<Arc<Web>>, viewer: Viewer, Path(key): Path<String>) -> Response {
	match web.app.pref(viewer.user, &key) {
		Ok(value) => Json(value.unwrap_or(Value::Null)).into_response(),
		Err(err) => internal(err),
	}
}

async fn write(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path(key): Path<String>,
	Json(value): Json<Value>,
) -> Response {
	let text = value.to_string();
	if key.len() > 200 || text.len() > 16_384 {
		return fail(
			StatusCode::PAYLOAD_TOO_LARGE,
			"This preference is too large.",
		);
	}
	match web.app.set_pref(viewer.user, &key, &text) {
		Ok(()) => StatusCode::NO_CONTENT.into_response(),
		Err(err) => internal(err),
	}
}

impl App {
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
