use std::sync::{Arc, Mutex, PoisonError};

use argon2::{Argon2, PasswordHasher, PasswordVerifier};
use axum::extract::{FromRequestParts, Path, State};
use axum::http::request::Parts;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use rusqlite::{OptionalExtension, params};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::app::{App, key_hash, token};
use crate::stats::Stats;

const COOKIE: &str = "impactite_session";
const SESSION_SECS: i64 = 30 * 86_400;
const INVITE_SECS: i64 = 7 * 86_400;
const ROLES: &[&str] = &["owner", "admin", "editor", "viewer"];

pub struct Web {
	pub app: Arc<App>,
	pub stats: Arc<Stats>,
	pub setup: Mutex<Option<String>>,
}

pub struct Viewer {
	pub user: i64,
}

impl FromRequestParts<Arc<Web>> for Viewer {
	type Rejection = Response;

	async fn from_request_parts(parts: &mut Parts, web: &Arc<Web>) -> Result<Self, Response> {
		cookie(&parts.headers)
			.and_then(|token| web.app.session_user(&token).ok().flatten())
			.map(|user| Viewer { user })
			.ok_or_else(|| fail(StatusCode::UNAUTHORIZED, "Sign in to continue."))
	}
}

pub fn routes() -> Router<Arc<Web>> {
	Router::new()
		.route("/api/app/state", get(state))
		.route("/api/app/setup", post(setup))
		.route("/api/app/login", post(login))
		.route("/api/app/logout", post(logout))
		.route("/api/app/teams", post(create_team))
		.route("/api/app/teams/{team}/invites", post(invite))
		.route(
			"/api/app/teams/{team}/members/{user}",
			delete(remove_member),
		)
		.route("/api/app/invites/accept", post(accept))
		.route("/api/app/sites", post(add_site))
}

#[derive(Deserialize)]
struct Credentials {
	email: String,
	password: String,
}

#[derive(Deserialize)]
struct Setup {
	token: String,
	name: String,
	email: String,
	password: String,
}

async fn state(State(web): State<Arc<Web>>, headers: HeaderMap) -> Response {
	let user = cookie(&headers).and_then(|token| web.app.session_user(&token).ok().flatten());
	let setup = web.app.users_exist().map(|exists| !exists).unwrap_or(false);
	let me = user.and_then(|user| web.app.me(user).ok());
	Json(json!({ "setup": setup, "me": me })).into_response()
}

async fn setup(
	State(web): State<Arc<Web>>,
	headers: HeaderMap,
	Json(body): Json<Setup>,
) -> Response {
	let expected = web
		.setup
		.lock()
		.unwrap_or_else(PoisonError::into_inner)
		.clone();
	if expected.as_deref() != Some(body.token.as_str()) || web.app.users_exist().unwrap_or(true) {
		return fail(StatusCode::FORBIDDEN, "This setup link is no longer valid.");
	}
	if let Err(message) = valid_account(&body.name, &body.email, &body.password) {
		return fail(StatusCode::BAD_REQUEST, message);
	}
	let created = web
		.app
		.create_user(&body.name, &body.email, &body.password)
		.and_then(|user| {
			let team = web.app.personal_team(user)?;
			web.app.adopt_sites(team)?;
			Ok(user)
		});
	match created {
		Ok(user) => {
			*web.setup.lock().unwrap_or_else(PoisonError::into_inner) = None;
			signed_in(&web, user, &headers)
		}
		Err(err) => internal(err),
	}
}

async fn login(
	State(web): State<Arc<Web>>,
	headers: HeaderMap,
	Json(body): Json<Credentials>,
) -> Response {
	match web.app.verify(&body.email, &body.password) {
		Ok(Some(user)) => signed_in(&web, user, &headers),
		Ok(None) => fail(StatusCode::UNAUTHORIZED, "Wrong email or password."),
		Err(err) => internal(err),
	}
}

async fn logout(State(web): State<Arc<Web>>, headers: HeaderMap) -> Response {
	if let Some(token) = cookie(&headers) {
		let _ = web.app.end_session(&token);
	}
	let mut response = StatusCode::NO_CONTENT.into_response();
	set_cookie(&mut response, &headers, "", 0);
	response
}

#[derive(Deserialize)]
struct NewTeam {
	name: String,
}

async fn create_team(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Json(body): Json<NewTeam>,
) -> Response {
	let name = body.name.trim();
	if name.is_empty() || name.len() > 80 {
		return fail(StatusCode::BAD_REQUEST, "Give the team a name.");
	}
	match web.app.create_team(viewer.user, name, false) {
		Ok(team) => Json(json!({ "id": team })).into_response(),
		Err(err) => internal(err),
	}
}

#[derive(Deserialize)]
struct Invite {
	email: String,
	role: String,
}

async fn invite(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path(team): Path<i64>,
	Json(body): Json<Invite>,
) -> Response {
	if !matches!(web.app.role(viewer.user, team), Ok(Some(role)) if role == "owner" || role == "admin")
	{
		return fail(StatusCode::FORBIDDEN, "Only owners and admins can invite.");
	}
	if web.app.is_personal(team).unwrap_or(true) {
		return fail(StatusCode::BAD_REQUEST, "Create a team to invite people.");
	}
	if !ROLES[1..].contains(&body.role.as_str()) || !body.email.contains('@') {
		return fail(StatusCode::BAD_REQUEST, "Use a valid email and role.");
	}
	match web.app.invite(team, &body.email, &body.role) {
		Ok(token) => Json(json!({ "token": token })).into_response(),
		Err(err) => internal(err),
	}
}

#[derive(Deserialize)]
struct Accept {
	token: String,
	name: Option<String>,
	password: Option<String>,
}

async fn accept(
	State(web): State<Arc<Web>>,
	headers: HeaderMap,
	Json(body): Json<Accept>,
) -> Response {
	let Ok(Some((team, email, role))) = web.app.open_invite(&body.token) else {
		return fail(
			StatusCode::NOT_FOUND,
			"This invite expired or was already used.",
		);
	};
	let signed = cookie(&headers).and_then(|token| web.app.session_user(&token).ok().flatten());
	let user = match (signed, &body.name, &body.password) {
		(Some(user), _, _) => user,
		(None, Some(name), Some(password)) => {
			if let Err(message) = valid_account(name, &email, password) {
				return fail(StatusCode::BAD_REQUEST, message);
			}
			match web.app.create_user(name, &email, password) {
				Ok(user) => user,
				Err(err) => return internal(err),
			}
		}
		_ => {
			return fail(
				StatusCode::UNAUTHORIZED,
				"Sign in or create an account to join.",
			);
		}
	};
	if let Err(err) = web.app.join(team, user, &role, &body.token) {
		return internal(err);
	}
	signed_in(&web, user, &headers)
}

async fn remove_member(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path((team, user)): Path<(i64, i64)>,
) -> Response {
	let allowed = viewer.user == user
		|| matches!(web.app.role(viewer.user, team), Ok(Some(role)) if role == "owner" || role == "admin");
	if !allowed || web.app.role(user, team).ok().flatten().as_deref() == Some("owner") {
		return fail(StatusCode::FORBIDDEN, "You can't remove this member.");
	}
	match web.app.remove_member(team, user) {
		Ok(()) => StatusCode::NO_CONTENT.into_response(),
		Err(err) => internal(err),
	}
}

#[derive(Deserialize)]
struct NewSite {
	team: i64,
	domain: String,
	timezone: String,
}

async fn add_site(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Json(body): Json<NewSite>,
) -> Response {
	if !matches!(web.app.role(viewer.user, body.team), Ok(Some(role)) if role != "viewer") {
		return fail(StatusCode::FORBIDDEN, "You can't add sites to this team.");
	}
	let domain = body
		.domain
		.trim()
		.trim_start_matches("https://")
		.trim_start_matches("http://")
		.trim_start_matches("www.")
		.trim_end_matches('/')
		.to_lowercase();
	if domain.is_empty() || domain.contains('/') || jiff::tz::TimeZone::get(&body.timezone).is_err()
	{
		return fail(StatusCode::BAD_REQUEST, "Use a valid domain and timezone.");
	}
	match web.app.add_site(&domain, &body.timezone, Some(body.team)) {
		Ok(site) => Json(json!({ "id": site.id, "domain": site.domain })).into_response(),
		Err(rusqlite::Error::SqliteFailure(err, _))
			if err.code == rusqlite::ErrorCode::ConstraintViolation =>
		{
			fail(StatusCode::CONFLICT, "This domain is already tracked.")
		}
		Err(err) => internal(err),
	}
}

fn valid_account(name: &str, email: &str, password: &str) -> Result<(), &'static str> {
	if name.trim().is_empty() {
		return Err("Tell us your name.");
	}
	if !email.contains('@') {
		return Err("Use a valid email.");
	}
	if password.chars().count() < 10 {
		return Err("Use at least 10 characters for the password.");
	}
	Ok(())
}

fn signed_in(web: &Web, user: i64, headers: &HeaderMap) -> Response {
	let (token, me) = match (web.app.start_session(user), web.app.me(user)) {
		(Ok(token), Ok(me)) => (token, me),
		(Err(err), _) | (_, Err(err)) => return internal(err),
	};
	let mut response = Json(json!({ "me": me })).into_response();
	set_cookie(&mut response, headers, &token, SESSION_SECS);
	response
}

fn cookie(headers: &HeaderMap) -> Option<String> {
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

fn set_cookie(response: &mut Response, headers: &HeaderMap, value: &str, max_age: i64) {
	let secure = headers
		.get("x-forwarded-proto")
		.is_some_and(|proto| proto == "https");
	let cookie = format!(
		"{COOKIE}={value}; Path=/; Max-Age={max_age}; HttpOnly; SameSite=Lax{}",
		if secure { "; Secure" } else { "" }
	);
	if let Ok(value) = HeaderValue::from_str(&cookie) {
		response.headers_mut().insert(header::SET_COOKIE, value);
	}
}

pub fn fail(status: StatusCode, message: &str) -> Response {
	(status, Json(json!({ "error": message }))).into_response()
}

fn internal(err: rusqlite::Error) -> Response {
	tracing::error!(%err, "accounts");
	fail(StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong.")
}

fn now() -> i64 {
	jiff::Timestamp::now().as_second()
}

impl App {
	pub fn users_exist(&self) -> rusqlite::Result<bool> {
		self.conn()
			.query_row("select exists (select 1 from users)", [], |row| row.get(0))
	}

	pub fn create_user(&self, name: &str, email: &str, password: &str) -> rusqlite::Result<i64> {
		let hash = Argon2::default()
			.hash_password(password.as_bytes())
			.map_err(|err| rusqlite::Error::ToSqlConversionFailure(err.to_string().into()))?
			.to_string();
		let user = {
			let conn = self.conn();
			conn.execute(
				"insert into users (email, name, password, created_at) values (?1, ?2, ?3, unixepoch())",
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
				Argon2::default()
					.verify_password(password.as_bytes(), hash.as_str())
					.is_ok()
			})
			.map(|(user, _)| user))
	}

	pub fn start_session(&self, user: i64) -> rusqlite::Result<String> {
		let token = token()?;
		self.conn().execute(
			"insert into sessions (hash, user, expires_at) values (?1, ?2, ?3)",
			params![key_hash(&token), user, now() + SESSION_SECS],
		)?;
		Ok(token)
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

	pub fn create_team(&self, user: i64, name: &str, personal: bool) -> rusqlite::Result<i64> {
		let conn = self.conn();
		conn.execute(
			"insert into teams (name, personal, created_at) values (?1, ?2, unixepoch())",
			params![name, personal],
		)?;
		let team = conn.last_insert_rowid();
		conn.execute(
			"insert into members (team, user, role) values (?1, ?2, 'owner')",
			params![team, user],
		)?;
		Ok(team)
	}

	pub fn personal_team(&self, user: i64) -> rusqlite::Result<i64> {
		self.conn().query_row(
			"select t.id from teams t join members m on m.team = t.id where m.user = ?1 and t.personal = 1",
			[user],
			|row| row.get(0),
		)
	}

	pub fn is_personal(&self, team: i64) -> rusqlite::Result<bool> {
		self.conn()
			.query_row("select personal from teams where id = ?1", [team], |row| {
				row.get(0)
			})
	}

	pub fn adopt_sites(&self, team: i64) -> rusqlite::Result<()> {
		self.conn()
			.execute("update sites set team = ?1 where team is null", [team])?;
		Ok(())
	}

	pub fn role(&self, user: i64, team: i64) -> rusqlite::Result<Option<String>> {
		self.conn()
			.query_row(
				"select role from members where user = ?1 and team = ?2",
				params![user, team],
				|row| row.get(0),
			)
			.optional()
	}

	pub fn site_role(&self, user: i64, domain: &str) -> rusqlite::Result<Option<String>> {
		self.conn()
			.query_row(
				"select m.role from sites s join members m on m.team = s.team where m.user = ?1 and s.domain = ?2",
				params![user, domain],
				|row| row.get(0),
			)
			.optional()
	}

	pub fn invite(&self, team: i64, email: &str, role: &str) -> rusqlite::Result<String> {
		let token = token()?;
		self.conn().execute(
			"insert into invites (hash, team, email, role, expires_at) values (?1, ?2, ?3, ?4, ?5)",
			params![
				key_hash(&token),
				team,
				email.trim(),
				role,
				now() + INVITE_SECS
			],
		)?;
		Ok(token)
	}

	pub fn open_invite(&self, token: &str) -> rusqlite::Result<Option<(i64, String, String)>> {
		self.conn()
			.query_row(
				"select team, email, role from invites where hash = ?1 and expires_at > ?2",
				params![key_hash(token), now()],
				|row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
			)
			.optional()
	}

	pub fn join(&self, team: i64, user: i64, role: &str, token: &str) -> rusqlite::Result<()> {
		let conn = self.conn();
		conn.execute(
			"insert into members (team, user, role) values (?1, ?2, ?3) on conflict do nothing",
			params![team, user, role],
		)?;
		conn.execute("delete from invites where hash = ?1", [key_hash(token)])?;
		Ok(())
	}

	pub fn remove_member(&self, team: i64, user: i64) -> rusqlite::Result<()> {
		self.conn().execute(
			"delete from members where team = ?1 and user = ?2",
			params![team, user],
		)?;
		Ok(())
	}

	pub fn me(&self, user: i64) -> rusqlite::Result<Value> {
		let conn = self.conn();
		let (name, email): (String, String) = conn.query_row(
			"select name, email from users where id = ?1",
			[user],
			|row| Ok((row.get(0)?, row.get(1)?)),
		)?;
		let mut teams_stmt = conn.prepare(
			"select t.id, t.name, t.personal, m.role from teams t join members m on m.team = t.id
			where m.user = ?1 order by t.personal desc, t.name",
		)?;
		let mut sites_stmt =
			conn.prepare("select id, domain, timezone from sites where team = ?1 order by domain")?;
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
				.query_map([id], |row| {
					Ok(json!({
						"id": row.get::<_, i64>(0)?,
						"domain": row.get::<_, String>(1)?,
						"timezone": row.get::<_, String>(2)?,
					}))
				})?
				.collect::<rusqlite::Result<Vec<_>>>()?;
			teams.push(
				json!({ "id": id, "name": name, "personal": personal, "role": role, "sites": sites }),
			);
		}
		Ok(json!({ "id": user, "name": name, "email": email, "teams": teams }))
	}
}
