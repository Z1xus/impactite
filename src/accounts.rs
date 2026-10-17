use std::sync::Arc;

use argon2::{Argon2, PasswordHasher, PasswordVerifier};
use axum::extract::{FromRequestParts, State};
use axum::http::request::Parts;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use rusqlite::{OptionalExtension, params};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::app::{App, key_hash, lock, now, token};
use crate::passkeys::{Failure, Proof, Purpose, device, registered};
use crate::web::{Web, fail, internal};

const COOKIE: &str = "impactite_session";
const SESSION_SECS: i64 = 30 * 86_400;

pub struct Viewer {
	pub user: i64,
	pub session: String,
}

impl FromRequestParts<Arc<Web>> for Viewer {
	type Rejection = Response;

	async fn from_request_parts(parts: &mut Parts, web: &Arc<Web>) -> Result<Self, Response> {
		let session = cookie(&parts.headers).map(|token| key_hash(&token));
		let found = session
			.as_ref()
			.and_then(|hash| web.app.session(hash).ok().flatten());
		let (Some(session), Some((user, seen))) = (session, found) else {
			return Err(fail(StatusCode::UNAUTHORIZED, "Sign in to continue."));
		};
		if now() - seen > 3600 {
			let _ = web
				.app
				.touch_session(&session, &device(&parts.headers, "Unknown device"));
		}
		Ok(Viewer { user, session })
	}
}

pub fn routes() -> Router<Arc<Web>> {
	Router::new()
		.route("/api/app/state", get(state))
		.route("/api/app/setup", post(setup))
		.route("/api/app/login", post(login))
		.route("/api/app/logout", post(logout))
}

#[derive(Deserialize)]
struct Credentials {
	email: String,
	password: String,
	code: Option<String>,
}

#[derive(Deserialize)]
struct Setup {
	token: String,
	name: String,
	email: String,
	password: Option<String>,
	passkey: Option<Proof>,
}

async fn state(State(web): State<Arc<Web>>, headers: HeaderMap) -> Response {
	let user = cookie(&headers).and_then(|token| web.app.session_user(&token).ok().flatten());
	let setup = web.app.users_exist().map(|exists| !exists).unwrap_or(false);
	let me = user.and_then(|user| web.app.me(user).ok());
	let update = me.as_ref().filter(|me| me["admin"] == true).and_then(|_| {
		lock(&web.updates)
			.latest
			.as_ref()
			.map(|latest| latest["version"].clone())
	});
	Json(
		json!({ "setup": setup, "signup": web.app.signup_open(), "me": me, "alias": web.alias,
		"sso": web.sso.as_ref().map(|provider| &provider.name), "update": update }),
	)
	.into_response()
}

async fn setup(
	State(web): State<Arc<Web>>,
	headers: HeaderMap,
	Json(body): Json<Setup>,
) -> Response {
	let expected = lock(&web.setup).clone();
	if expected.as_deref() != Some(body.token.as_str()) || web.app.users_exist().unwrap_or(true) {
		return fail(StatusCode::FORBIDDEN, "This setup link is no longer valid.");
	}
	let user = match new_account(
		&web,
		&headers,
		(&body.name, &body.email),
		body.password.as_deref(),
		body.passkey.as_ref(),
		&Purpose::Setup,
	) {
		Ok(user) => user,
		Err((status, message)) => return fail(status, message),
	};
	let adopted = web
		.app
		.personal_team(user)
		.and_then(|team| web.app.adopt_sites(team));
	if let Err(err) = adopted {
		return internal(err);
	}
	*lock(&web.setup) = None;
	signed_in(&web, user, None)
}

pub(crate) fn new_account(
	web: &Web,
	headers: &HeaderMap,
	(name, email): (&str, &str),
	password: Option<&str>,
	passkey: Option<&Proof>,
	purpose: &Purpose,
) -> Result<i64, Failure> {
	let broken = |err: rusqlite::Error| match err {
		rusqlite::Error::SqliteFailure(err, _)
			if err.code == rusqlite::ErrorCode::ConstraintViolation =>
		{
			(
				StatusCode::CONFLICT,
				"This email already has an account. Sign in instead.",
			)
		}
		err => {
			tracing::error!(%err, "accounts");
			(StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong.")
		}
	};
	valid_identity(name, email).map_err(|message| (StatusCode::BAD_REQUEST, message))?;
	match (passkey, password) {
		(Some(proof), _) => {
			let key = registered(web, proof, purpose)?;
			let user = web.app.create_user(name, email, None).map_err(broken)?;
			web.app
				.add_passkey(user, &key, &device(headers, "Passkey"))
				.map_err(broken)?;
			Ok(user)
		}
		(None, Some(password)) if password.chars().count() >= 10 => web
			.app
			.create_user(name, email, Some(password))
			.map_err(broken),
		(None, Some(_)) => Err((
			StatusCode::BAD_REQUEST,
			"Use at least 10 characters for the password.",
		)),
		(None, None) => Err((StatusCode::BAD_REQUEST, "Create a passkey to continue.")),
	}
}

async fn login(State(web): State<Arc<Web>>, Json(body): Json<Credentials>) -> Response {
	match web.app.verify(&body.email, &body.password) {
		Ok(Some(user)) => match (web.app.totp(user), &body.code) {
			(Ok(None), _) => signed_in(&web, user, None),
			(Ok(Some(_)), None) => (
				StatusCode::UNAUTHORIZED,
				Json(
					json!({ "error": "Enter the code from your authenticator app.", "totp": true }),
				),
			)
				.into_response(),
			(Ok(Some(_)), Some(code)) => match crate::totp::verify(&web.app, user, code) {
				Ok(()) => signed_in(&web, user, None),
				Err(message) => (
					StatusCode::UNAUTHORIZED,
					Json(json!({ "error": message, "totp": true })),
				)
					.into_response(),
			},
			(Err(err), _) => internal(err),
		},
		Ok(None) => fail(StatusCode::UNAUTHORIZED, "Wrong email or password."),
		Err(err) => internal(err),
	}
}

async fn logout(State(web): State<Arc<Web>>, headers: HeaderMap) -> Response {
	if let Some(token) = cookie(&headers) {
		let _ = web.app.end_session(&token);
	}
	let mut response = StatusCode::NO_CONTENT.into_response();
	set_cookie(&mut response, &web, "", 0);
	response
}

fn valid_identity(name: &str, email: &str) -> Result<(), &'static str> {
	if name.trim().is_empty() {
		return Err("Tell us your name.");
	}
	if !email.contains('@') {
		return Err("Use a valid email.");
	}
	Ok(())
}

pub(crate) fn signed_in(web: &Web, user: i64, team: Option<i64>) -> Response {
	let (token, me) = match (web.app.start_session(user), web.app.me(user)) {
		(Ok(token), Ok(me)) => (token, me),
		(Err(err), _) | (_, Err(err)) => return internal(err),
	};
	let mut response = Json(json!({ "me": me, "team": team })).into_response();
	set_cookie(&mut response, web, &token, SESSION_SECS);
	response
}

pub(crate) fn cookie(headers: &HeaderMap) -> Option<String> {
	headers
		.get_all(header::COOKIE)
		.iter()
		.filter_map(|value| value.to_str().ok())
		.flat_map(|value| value.split(';'))
		.filter_map(|pair| pair.trim().split_once('='))
		.find(|(name, _)| *name == COOKIE)
		.map(|(_, value)| value.to_owned())
		.filter(|value| !value.is_empty())
}

fn set_cookie(response: &mut Response, web: &Web, value: &str, max_age: i64) {
	let secure = web.url.starts_with("https://");
	let cookie = format!(
		"{COOKIE}={value}; Path=/; Max-Age={max_age}; HttpOnly; SameSite=Lax{}",
		if secure { "; Secure" } else { "" }
	);
	if let Ok(value) = HeaderValue::from_str(&cookie) {
		response.headers_mut().insert(header::SET_COOKIE, value);
	}
}

#[derive(Deserialize)]
pub struct Signup {
	name: String,
	email: String,
	password: Option<String>,
	passkey: Option<Proof>,
}

pub async fn signup(
	State(web): State<Arc<Web>>,
	headers: HeaderMap,
	Json(body): Json<Signup>,
) -> Response {
	if !web.app.signup_open() {
		return fail(StatusCode::FORBIDDEN, "Sign-up is closed on this server.");
	}
	match new_account(
		&web,
		&headers,
		(&body.name, &body.email),
		body.password.as_deref(),
		body.passkey.as_ref(),
		&Purpose::Signup,
	) {
		Ok(user) => signed_in(&web, user, None),
		Err((status, message)) => fail(status, message),
	}
}

pub(crate) fn hash(password: &str) -> rusqlite::Result<String> {
	Argon2::default()
		.hash_password(password.as_bytes())
		.map(|hash| hash.to_string())
		.map_err(|err| rusqlite::Error::ToSqlConversionFailure(err.to_string().into()))
}

impl App {
	pub fn first_user(&self) -> rusqlite::Result<Option<i64>> {
		self.conn()
			.query_row("select min(id) from users", [], |row| row.get(0))
	}

	pub fn users_exist(&self) -> rusqlite::Result<bool> {
		self.conn()
			.query_row("select exists (select 1 from users)", [], |row| row.get(0))
	}

	pub fn create_user(
		&self,
		name: &str,
		email: &str,
		password: Option<&str>,
	) -> rusqlite::Result<i64> {
		let hash = match password {
			Some(password) => hash(password)?,
			None => String::new(),
		};
		let user = {
			let conn = self.conn();
			conn.execute(
				"insert into users (email, name, password, admin, created_at)
				values (?1, ?2, ?3, not exists (select 1 from users), unixepoch())",
				params![email.trim(), name.trim(), hash],
			)?;
			conn.last_insert_rowid()
		};
		self.create_team(user, "Personal", true)?;
		Ok(user)
	}

	pub fn verify(&self, email: &str, password: &str) -> rusqlite::Result<Option<i64>> {
		let found: Option<(i64, String)> = self
			.conn()
			.query_row(
				"select id, password from users where email = ?1",
				[email.trim()],
				|row| Ok((row.get(0)?, row.get(1)?)),
			)
			.optional()?;
		Ok(found
			.filter(|(_, hash)| {
				!hash.is_empty()
					&& Argon2::default()
						.verify_password(password.as_bytes(), hash.as_str())
						.is_ok()
			})
			.map(|(user, _)| user))
	}

	pub fn start_session(&self, user: i64) -> rusqlite::Result<String> {
		let token = token()?;
		self.conn().execute(
			"insert into sessions (hash, user, expires_at, created_at) values (?1, ?2, ?3, unixepoch())",
			params![key_hash(&token), user, now() + SESSION_SECS],
		)?;
		Ok(token)
	}

	pub fn session(&self, hash: &str) -> rusqlite::Result<Option<(i64, i64)>> {
		self.conn()
			.query_row(
				"select user, seen_at from sessions where hash = ?1 and expires_at > ?2",
				params![hash, now()],
				|row| Ok((row.get(0)?, row.get(1)?)),
			)
			.optional()
	}

	pub fn touch_session(&self, hash: &str, device: &str) -> rusqlite::Result<()> {
		self.conn().execute(
			"update sessions set seen_at = unixepoch(), device = ?2 where hash = ?1",
			params![hash, device],
		)?;
		Ok(())
	}

	pub fn session_user(&self, token: &str) -> rusqlite::Result<Option<i64>> {
		self.conn()
			.query_row(
				"select user from sessions where hash = ?1 and expires_at > ?2",
				params![key_hash(token), now()],
				|row| row.get(0),
			)
			.optional()
	}

	pub fn end_session(&self, token: &str) -> rusqlite::Result<()> {
		self.conn()
			.execute("delete from sessions where hash = ?1", [key_hash(token)])?;
		Ok(())
	}

	pub fn user_name(&self, user: i64) -> rusqlite::Result<String> {
		self.conn()
			.query_row("select name from users where id = ?1", [user], |row| {
				row.get(0)
			})
	}

	pub fn me(&self, user: i64) -> rusqlite::Result<Value> {
		let conn = self.conn();
		let (name, email, password, admin, totp): (String, String, bool, bool, bool) = conn
			.query_row(
				"select name, email, password != '', admin, totp != '' from users where id = ?1",
				[user],
				|row| {
					Ok((
						row.get(0)?,
						row.get(1)?,
						row.get(2)?,
						row.get(3)?,
						row.get(4)?,
					))
				},
			)?;
		let mut teams_stmt = conn.prepare(
			"select t.id, t.name, t.personal, m.role from teams t join members m on m.team = t.id
			where m.user = ?1 order by t.personal desc, t.name",
		)?;
		let mut sites_stmt = conn.prepare(
			"select id, domain, timezone, retention, counter, skip from sites where team = ?1 order by domain",
		)?;
		let site = |row: &rusqlite::Row, role: String| {
			Ok(json!({
				"id": row.get::<_, i64>(0)?,
				"domain": row.get::<_, String>(1)?,
				"timezone": row.get::<_, String>(2)?,
				"retention": row.get::<_, i64>(3)?,
				"counter": row.get::<_, bool>(4)?,
				"skip": row.get::<_, String>(5)?.split(',').filter(|item| !item.is_empty()).collect::<Vec<_>>(),
				"role": role,
			}))
		};
		let mut teams = Vec::new();
		for team in teams_stmt.query_map([user], |row| {
			Ok((
				row.get::<_, i64>(0)?,
				row.get::<_, String>(1)?,
				row.get::<_, bool>(2)?,
				row.get::<_, String>(3)?,
			))
		})? {
			let (id, name, personal, role) = team?;
			let sites = sites_stmt
				.query_map([id], |row| site(row, role.clone()))?
				.collect::<rusqlite::Result<Vec<_>>>()?;
			teams.push(
				json!({ "id": id, "name": name, "personal": personal, "role": role, "sites": sites }),
			);
		}
		let shared = conn
			.prepare(
				"select s.id, s.domain, s.timezone, s.retention, s.counter, s.skip, g.role from guests g join sites s on s.id = g.site
				where g.user = ?1 and s.team is not null
				and s.team not in (select team from members where user = ?1) order by s.domain",
			)?
			.query_map([user], |row| site(row, row.get(6)?))?
			.collect::<rusqlite::Result<Vec<_>>>()?;
		if !shared.is_empty() {
			teams.push(json!({ "id": 0, "name": "Shared with me", "personal": false, "role": "guest", "sites": shared }));
		}
		Ok(
			json!({ "id": user, "name": name, "email": email, "password": password, "admin": admin, "totp": totp, "teams": teams }),
		)
	}
}
