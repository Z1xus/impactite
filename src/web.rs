use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use axum::Json;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde_json::{Value, json};

use crate::app::{App, Site};
use crate::auth::passkeys::Purpose;
use crate::auth::webauthn::Relying;
use crate::ingest::Ingest;
use crate::mail::Mail;
use crate::site::shares::Reader;
use crate::stats::Stats;

pub struct Web {
	pub app: Arc<App>,
	pub stats: Arc<Stats>,
	pub ingest: Arc<Ingest>,
	pub mail: Arc<Mail>,
	pub http: reqwest::Client,
	pub hooks: reqwest::Client,
	pub data: PathBuf,
	pub url: String,
	pub rp: Relying,
	pub setup: Mutex<Option<String>>,
	pub ceremonies: Mutex<HashMap<String, (Purpose, i64)>>,
	pub alias: Option<String>,
	pub updates: Mutex<crate::update::Status>,
	pub sso: Option<crate::auth::sso::Provider>,
	pub google: Option<crate::site::google::Client>,
}

// what a handler returns when it can't answer, sent as `{ "error": message }`
#[derive(Debug)]
pub struct Error(StatusCode, String);

pub type Reply<T = Json<Value>> = Result<T, Error>;

impl Error {
	pub fn new(status: StatusCode, message: impl Into<String>) -> Self {
		Self(status, message.into())
	}

	pub fn bad(message: impl Into<String>) -> Self {
		Self::new(StatusCode::BAD_REQUEST, message)
	}

	pub fn forbidden(message: impl Into<String>) -> Self {
		Self::new(StatusCode::FORBIDDEN, message)
	}

	pub fn not_found(message: impl Into<String>) -> Self {
		Self::new(StatusCode::NOT_FOUND, message)
	}

	pub fn conflict(message: impl Into<String>) -> Self {
		Self::new(StatusCode::CONFLICT, message)
	}

	pub fn too_many() -> Self {
		Self::new(
			StatusCode::TOO_MANY_REQUESTS,
			"Too many attempts. Wait a few minutes.",
		)
	}
}

impl Error {
	pub fn broken() -> Self {
		Self::new(StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong.")
	}
}

impl From<(StatusCode, &'static str)> for Error {
	fn from((status, message): (StatusCode, &'static str)) -> Self {
		Self::new(status, message)
	}
}

impl From<std::io::Error> for Error {
	fn from(err: std::io::Error) -> Self {
		tracing::error!(%err, "storage");
		Self::broken()
	}
}

impl From<tokio::task::JoinError> for Error {
	fn from(err: tokio::task::JoinError) -> Self {
		tracing::error!(%err, "task");
		Self::broken()
	}
}

pub async fn blocking<T: Send + 'static>(
	work: impl FnOnce() -> std::io::Result<T> + Send + 'static,
) -> Reply<T> {
	Ok(tokio::task::spawn_blocking(work).await??)
}

impl From<rusqlite::Error> for Error {
	fn from(err: rusqlite::Error) -> Self {
		tracing::error!(%err, "accounts");
		Self::broken()
	}
}

impl IntoResponse for Error {
	fn into_response(self) -> Response {
		fail(self.0, &self.1)
	}
}

impl Web {
	// the address a rate limit counts, a whole ipv6 network is one visitor
	pub fn client(&self, peer: SocketAddr, headers: &HeaderMap) -> String {
		match self.ingest.proxies.visitor(peer, headers).ip {
			IpAddr::V6(ip) => format!("{:x}", u128::from(ip) >> 64),
			ip => ip.to_string(),
		}
	}

	pub fn editable(&self, user: i64, domain: &str) -> Reply<Site> {
		self.app
			.editable(user, domain)
			.ok_or_else(|| Error::forbidden("You can't change this site."))
	}

	pub fn readable(&self, reader: &Reader, domain: &str) -> Reply<Site> {
		self.app
			.site(domain)?
			.filter(|_| reader.reads(self, domain))
			.ok_or_else(|| Error::not_found("Site not found."))
	}
}

pub fn fail(status: StatusCode, message: &str) -> Response {
	(status, Json(json!({ "error": message }))).into_response()
}
