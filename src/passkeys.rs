use std::sync::{Arc, PoisonError};

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64;
use rusqlite::{OptionalExtension, params};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::accounts::{Viewer, Web, fail, signed_in};
use crate::app::App;
use crate::ingest::ua;
use crate::webauthn::{self, Assertion, Key, Registration};

const CEREMONY_SECS: i64 = 300;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Purpose {
	Setup,
	Invite(String),
	Add(i64),
	Login,
}

#[derive(Deserialize)]
pub struct Proof {
	pub challenge: String,
	pub credential: Registration,
}

pub fn routes() -> Router<Arc<Web>> {
	Router::new()
		.route("/api/app/passkeys", get(list).post(add))
		.route("/api/app/passkeys/new", post(new))
		.route("/api/app/passkeys/signup", post(signup))
		.route("/api/app/passkeys/login/begin", post(login_begin))
		.route("/api/app/passkeys/login", post(login))
		.route("/api/app/passkeys/{id}", delete(remove))
}

impl Web {
	fn begin(&self, purpose: Purpose) -> Option<String> {
		let challenge = webauthn::challenge()?;
		let now = now();
		let mut ceremonies = self
			.ceremonies
			.lock()
			.unwrap_or_else(PoisonError::into_inner);
		ceremonies.retain(|_, (_, expires)| *expires > now);
		ceremonies.insert(challenge.clone(), (purpose, now + CEREMONY_SECS));
		Some(challenge)
	}

	fn take(&self, challenge: &str) -> Option<Purpose> {
		let mut ceremonies = self
			.ceremonies
			.lock()
			.unwrap_or_else(PoisonError::into_inner);
		ceremonies
			.remove(challenge)
			.filter(|(_, expires)| *expires > now())
			.map(|(purpose, _)| purpose)
	}
}

pub type Failure = (StatusCode, &'static str);

pub fn registered(web: &Web, proof: &Proof, purpose: &Purpose) -> Result<Key, Failure> {
	if web.take(&proof.challenge).as_ref() != Some(purpose) {
		return Err((
			StatusCode::BAD_REQUEST,
			"This passkey request expired. Try again.",
		));
	}
	webauthn::register(&web.rp, &proof.challenge, &proof.credential).map_err(|err| {
		tracing::warn!(err, "passkey registration");
		(
			StatusCode::BAD_REQUEST,
			"This passkey could not be verified.",
		)
	})
}

pub fn device(headers: &HeaderMap) -> String {
	let agent = ua::parse(
		headers
			.get(header::USER_AGENT)
			.and_then(|value| value.to_str().ok())
			.unwrap_or_default(),
	);
	match (agent.browser, agent.os) {
		("", "") => "Passkey".into(),
		(browser, "") => browser.into(),
		("", os) => os.into(),
		(browser, os) => format!("{browser} on {os}"),
	}
}

#[derive(Deserialize)]
struct Signup {
	token: String,
	name: String,
	email: Option<String>,
	invite: bool,
}

async fn signup(State(web): State<Arc<Web>>, Json(body): Json<Signup>) -> Response {
	let (purpose, email) = if body.invite {
		match web.app.open_invite(&body.token) {
			Ok(Some((_, email, _))) => (Purpose::Invite(body.token.clone()), email),
			_ => {
				return fail(
					StatusCode::NOT_FOUND,
					"This invite expired or was already used.",
				);
			}
		}
	} else {
		let expected = web
			.setup
			.lock()
			.unwrap_or_else(PoisonError::into_inner)
			.clone();
		if expected.as_deref() != Some(body.token.as_str()) {
			return fail(StatusCode::FORBIDDEN, "This setup link is no longer valid.");
		}
		(Purpose::Setup, body.email.clone().unwrap_or_default())
	};
	let mut handle = [0; 16];
	if getrandom::fill(&mut handle).is_err() {
		return fail(StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong.");
	}
	ceremony(&web, purpose, |challenge| {
		webauthn::creation_options(&web.rp, challenge, &handle, &email, &body.name, &[])
	})
}

async fn new(State(web): State<Arc<Web>>, viewer: Viewer) -> Response {
	let (email, name, exclude) = match (
		web.app.user_contact(viewer.user),
		web.app.passkey_ids(viewer.user),
	) {
		(Ok((email, name)), Ok(exclude)) => (email, name, exclude),
		_ => return fail(StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong."),
	};
	let handle = viewer.user.to_be_bytes();
	ceremony(&web, Purpose::Add(viewer.user), |challenge| {
		webauthn::creation_options(&web.rp, challenge, &handle, &email, &name, &exclude)
	})
}

async fn add(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	headers: HeaderMap,
	Json(proof): Json<Proof>,
) -> Response {
	let key = match registered(&web, &proof, &Purpose::Add(viewer.user)) {
		Ok(key) => key,
		Err((status, message)) => return fail(status, message),
	};
	match web.app.add_passkey(viewer.user, &key, &device(&headers)) {
		Ok(()) => list(State(web), viewer).await,
		Err(_) => fail(StatusCode::CONFLICT, "This passkey is already saved."),
	}
}

async fn list(State(web): State<Arc<Web>>, viewer: Viewer) -> Response {
	match web.app.passkeys(viewer.user) {
		Ok(passkeys) => Json(json!({ "passkeys": passkeys })).into_response(),
		Err(_) => fail(StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong."),
	}
}

async fn remove(State(web): State<Arc<Web>>, viewer: Viewer, Path(id): Path<String>) -> Response {
	let Ok(id) = B64.decode(&id) else {
		return fail(StatusCode::NOT_FOUND, "Passkey not found.");
	};
	let last = web
		.app
		.passkey_ids(viewer.user)
		.is_ok_and(|ids| ids.len() <= 1)
		&& !web.app.has_password(viewer.user).unwrap_or(false);
	if last {
		return fail(
			StatusCode::BAD_REQUEST,
			"Add another passkey before removing your last one.",
		);
	}
	match web.app.remove_passkey(viewer.user, &id) {
		Ok(true) => list(State(web), viewer).await,
		Ok(false) => fail(StatusCode::NOT_FOUND, "Passkey not found."),
		Err(_) => fail(StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong."),
	}
}

async fn login_begin(State(web): State<Arc<Web>>) -> Response {
	ceremony(&web, Purpose::Login, |challenge| {
		webauthn::request_options(&web.rp, challenge)
	})
}

#[derive(Deserialize)]
struct Login {
	challenge: String,
	credential: Assertion,
}

async fn login(
	State(web): State<Arc<Web>>,
	headers: HeaderMap,
	Json(body): Json<Login>,
) -> Response {
	let unknown = || {
		fail(
			StatusCode::UNAUTHORIZED,
			"This passkey is not registered here.",
		)
	};
	if web.take(&body.challenge) != Some(Purpose::Login) {
		return fail(StatusCode::BAD_REQUEST, "This sign-in expired. Try again.");
	}
	let Some(id) = body.credential.credential() else {
		return unknown();
	};
	let Ok(Some((user, key))) = web.app.passkey(&id) else {
		return unknown();
	};
	match webauthn::verify(&web.rp, &body.challenge, &body.credential, &key) {
		Ok(counter) => {
			let _ = web.app.touch_passkey(&id, counter);
			signed_in(&web, user, &headers)
		}
		Err(err) => {
			tracing::warn!(err, "passkey login");
			fail(
				StatusCode::UNAUTHORIZED,
				"This passkey could not be verified.",
			)
		}
	}
}

fn ceremony(web: &Web, purpose: Purpose, options: impl FnOnce(&str) -> Value) -> Response {
	match web.begin(purpose) {
		Some(challenge) => {
			Json(json!({ "challenge": challenge, "options": options(&challenge) })).into_response()
		}
		None => fail(StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong."),
	}
}

fn now() -> i64 {
	jiff::Timestamp::now().as_second()
}

impl App {
	pub fn add_passkey(&self, user: i64, key: &Key, name: &str) -> rusqlite::Result<()> {
		self.conn().execute(
			"insert into passkeys (id, user, alg, key, counter, name, created_at) values (?1, ?2, ?3, ?4, ?5, ?6, unixepoch())",
			params![key.id, user, key.alg, key.public, key.counter, name],
		)?;
		Ok(())
	}

	pub fn passkey(&self, id: &[u8]) -> rusqlite::Result<Option<(i64, Key)>> {
		self.conn()
			.query_row(
				"select user, alg, key, counter from passkeys where id = ?1",
				[id],
				|row| {
					Ok((
						row.get(0)?,
						Key {
							id: id.to_vec(),
							alg: row.get(1)?,
							public: row.get(2)?,
							counter: row.get(3)?,
						},
					))
				},
			)
			.optional()
	}

	pub fn touch_passkey(&self, id: &[u8], counter: u32) -> rusqlite::Result<()> {
		self.conn().execute(
			"update passkeys set counter = ?2, used_at = unixepoch() where id = ?1",
			params![id, counter],
		)?;
		Ok(())
	}

	pub fn passkey_ids(&self, user: i64) -> rusqlite::Result<Vec<Vec<u8>>> {
		let conn = self.conn();
		let mut stmt = conn.prepare("select id from passkeys where user = ?1")?;
		stmt.query_map([user], |row| row.get(0))?.collect()
	}

	pub fn passkeys(&self, user: i64) -> rusqlite::Result<Vec<Value>> {
		let conn = self.conn();
		let mut stmt = conn.prepare(
			"select id, name, created_at, used_at from passkeys where user = ?1 order by created_at",
		)?;
		stmt.query_map([user], |row| {
			Ok(json!({
				"id": B64.encode(row.get::<_, Vec<u8>>(0)?),
				"name": row.get::<_, String>(1)?,
				"created_at": row.get::<_, i64>(2)?,
				"used_at": row.get::<_, Option<i64>>(3)?,
			}))
		})?
		.collect()
	}

	pub fn remove_passkey(&self, user: i64, id: &[u8]) -> rusqlite::Result<bool> {
		Ok(self.conn().execute(
			"delete from passkeys where user = ?1 and id = ?2",
			params![user, id],
		)? > 0)
	}

	pub fn has_password(&self, user: i64) -> rusqlite::Result<bool> {
		self.conn().query_row(
			"select password != '' from users where id = ?1",
			[user],
			|row| row.get(0),
		)
	}

	pub fn user_contact(&self, user: i64) -> rusqlite::Result<(String, String)> {
		self.conn().query_row(
			"select email, name from users where id = ?1",
			[user],
			|row| Ok((row.get(0)?, row.get(1)?)),
		)
	}
}
