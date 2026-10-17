use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::Response;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64;
use rusqlite::{OptionalExtension, params};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::app::now;
use crate::app::{App, lock};
use crate::auth::accounts::{Viewer, signed_in};
use crate::auth::webauthn::{self, Assertion, Key, Registration};
use crate::ingest::ua;
use crate::web::{Error, Reply, Web};

const CEREMONY_SECS: i64 = 300;
const MAX_CEREMONIES: usize = 10_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Purpose {
	Setup,
	Signup,
	Invite(String),
	Add(i64),
	Login,
	Google(u64),
	Analytics(u64),
	// nonce and pkce verifier of an sso sign in
	Sso(String, String),
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
	pub(crate) fn begin(&self, purpose: Purpose) -> Option<String> {
		let challenge = webauthn::challenge()?;
		let now = now();
		let mut ceremonies = lock(&self.ceremonies);
		ceremonies.retain(|_, (_, expires)| *expires > now);
		// anyone can start a sign-in, so the open ones stay bounded
		if ceremonies.len() >= MAX_CEREMONIES {
			return None;
		}
		ceremonies.insert(challenge.clone(), (purpose, now + CEREMONY_SECS));
		Some(challenge)
	}

	pub(crate) fn take(&self, challenge: &str) -> Option<Purpose> {
		let mut ceremonies = lock(&self.ceremonies);
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

pub fn device(headers: &HeaderMap, fallback: &str) -> String {
	let agent = ua::parse(
		headers
			.get(header::USER_AGENT)
			.and_then(|value| value.to_str().ok())
			.unwrap_or_default(),
	);
	match (agent.browser, agent.os) {
		("", "") => fallback.into(),
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
	#[serde(default)]
	open: bool,
}

async fn signup(State(web): State<Arc<Web>>, Json(body): Json<Signup>) -> Reply {
	let (purpose, email) = if body.open {
		if !web.app.signup_open() {
			return Err(Error::forbidden("Sign-up is closed on this server."));
		}
		(Purpose::Signup, body.email.clone().unwrap_or_default())
	} else if body.invite {
		let invitation = web
			.app
			.open_invite(&body.token)?
			.ok_or_else(|| Error::not_found("This invite expired or was already used."))?;
		(Purpose::Invite(body.token.clone()), invitation.email)
	} else {
		let expected = lock(&web.setup).clone();
		if expected.as_deref() != Some(body.token.as_str()) {
			return Err(Error::forbidden("This setup link is no longer valid."));
		}
		(Purpose::Setup, body.email.clone().unwrap_or_default())
	};
	let mut handle = [0; 16];
	getrandom::fill(&mut handle).map_err(|_| Error::broken())?;
	ceremony(&web, purpose, |challenge| {
		webauthn::creation_options(&web.rp, challenge, &handle, &email, &body.name, &[])
	})
}

async fn new(State(web): State<Arc<Web>>, viewer: Viewer) -> Reply {
	let (email, name) = web.app.user_contact(viewer.user)?;
	let exclude = web.app.passkey_ids(viewer.user)?;
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
) -> Reply {
	let key = registered(&web, &proof, &Purpose::Add(viewer.user))?;
	web.app
		.add_passkey(viewer.user, &key, &device(&headers, "Passkey"))
		.map_err(|_| Error::conflict("This passkey is already saved."))?;
	list(State(web), viewer).await
}

async fn list(State(web): State<Arc<Web>>, viewer: Viewer) -> Reply {
	Ok(Json(json!({ "passkeys": web.app.passkeys(viewer.user)? })))
}

async fn remove(State(web): State<Arc<Web>>, viewer: Viewer, Path(id): Path<String>) -> Reply {
	let id = B64
		.decode(&id)
		.map_err(|_| Error::not_found("Passkey not found."))?;
	let last = web
		.app
		.passkey_ids(viewer.user)
		.is_ok_and(|ids| ids.len() <= 1)
		&& !web.app.has_password(viewer.user).unwrap_or(false);
	if last {
		return Err(Error::bad(
			"Add another passkey before removing your last one.",
		));
	}
	if !web.app.remove_passkey(viewer.user, &id)? {
		return Err(Error::not_found("Passkey not found."));
	}
	list(State(web), viewer).await
}

async fn login_begin(State(web): State<Arc<Web>>) -> Reply {
	ceremony(&web, Purpose::Login, |challenge| {
		webauthn::request_options(&web.rp, challenge)
	})
}

#[derive(Deserialize)]
struct Login {
	challenge: String,
	credential: Assertion,
}

async fn login(State(web): State<Arc<Web>>, Json(body): Json<Login>) -> Reply<Response> {
	let unknown = || {
		Error::new(
			StatusCode::UNAUTHORIZED,
			"This passkey is not registered here.",
		)
	};
	if web.take(&body.challenge) != Some(Purpose::Login) {
		return Err(Error::bad("This sign-in expired. Try again."));
	}
	let id = body.credential.credential().ok_or_else(unknown)?;
	let (user, key) = web.app.passkey(&id)?.ok_or_else(unknown)?;
	let counter =
		webauthn::verify(&web.rp, &body.challenge, &body.credential, &key).map_err(|err| {
			tracing::warn!(err, "passkey login");
			Error::new(
				StatusCode::UNAUTHORIZED,
				"This passkey could not be verified.",
			)
		})?;
	let _ = web.app.touch_passkey(&id, counter);
	signed_in(&web, user, None)
}

fn ceremony(web: &Web, purpose: Purpose, options: impl FnOnce(&str) -> Value) -> Reply {
	let challenge = web.begin(purpose).ok_or_else(Error::broken)?;
	Ok(Json(
		json!({ "challenge": challenge, "options": options(&challenge) }),
	))
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
