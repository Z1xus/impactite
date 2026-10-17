use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

use crate::app::App;
use crate::ingest::Ingest;
use crate::mail::Mail;
use crate::passkeys::Purpose;
use crate::stats::Stats;
use crate::webauthn::Relying;

pub struct Web {
	pub app: Arc<App>,
	pub stats: Arc<Stats>,
	pub ingest: Arc<Ingest>,
	pub mail: Arc<Mail>,
	pub http: reqwest::Client,
	pub data: PathBuf,
	pub url: String,
	pub rp: Relying,
	pub setup: Mutex<Option<String>>,
	pub ceremonies: Mutex<HashMap<String, (Purpose, i64)>>,
	pub alias: Option<String>,
	pub updates: Mutex<crate::update::Status>,
	pub sso: Option<crate::sso::Provider>,
	pub google: Option<crate::google::Client>,
}

pub fn fail(status: StatusCode, message: &str) -> Response {
	(status, Json(json!({ "error": message }))).into_response()
}

pub fn internal(err: rusqlite::Error) -> Response {
	tracing::error!(%err, "accounts");
	broken()
}

pub fn broken() -> Response {
	fail(StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong.")
}
