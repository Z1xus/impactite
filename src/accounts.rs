use std::sync::{Arc, Mutex};

use argon2::{Argon2, PasswordHasher, PasswordVerifier};
use axum::extract::{FromRequestParts, Path, State};
use axum::http::request::Parts;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use rusqlite::{OptionalExtension, params};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::app::{App, Site, key_hash, lock, token};
use crate::ingest::Ingest;
use crate::mail::Letter;
use crate::mail::Mail;
use crate::passkeys::{Failure, Proof, Purpose, device, registered};
use crate::shares::Reader;
use crate::stats::Stats;
use crate::webauthn::Relying;

const COOKIE: &str = "impactite_session";
const SESSION_SECS: i64 = 30 * 86_400;
const INVITE_SECS: i64 = 7 * 86_400;
pub const ROLES: &[&str] = &["owner", "admin", "editor", "viewer"];

pub struct Web {
	pub app: Arc<App>,
	pub stats: Arc<Stats>,
	pub ingest: Arc<Ingest>,
	pub mail: Arc<Mail>,
	pub http: reqwest::Client,
	pub data: std::path::PathBuf,
	pub url: String,
	pub rp: Relying,
	pub setup: Mutex<Option<String>>,
	pub ceremonies: Mutex<std::collections::HashMap<String, (Purpose, i64)>>,
	pub alias: Option<String>,
	pub latest: Mutex<Option<Value>>,
	pub sso: Option<crate::sso::Provider>,
	pub google: Option<crate::google::Client>,
}

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
		.route("/api/app/teams", post(create_team))
		.route("/api/app/teams/{team}/invites", post(invite))
		.route(
			"/api/app/teams/{team}/members/{user}",
			delete(remove_member),
		)
		.route("/api/app/invites/accept", post(accept))
		.route("/api/app/invites/{token}", get(invitation))
		.route("/api/app/sites", post(add_site))
		.route("/api/app/live/{domain}", get(live))
}

async fn live(State(web): State<Arc<Web>>, reader: Reader, Path(domain): Path<String>) -> Response {
	if !reader.reads(&web, &domain) {
		return fail(StatusCode::NOT_FOUND, "Site not found.");
	}
	let Ok(Some(site)) = web.app.site(&domain) else {
		return fail(StatusCode::NOT_FOUND, "Site not found.");
	};
	let ticks = futures_util::stream::unfold(
		(
			web.ingest.clone(),
			tokio::time::interval(std::time::Duration::from_secs(2)),
		),
		move |(ingest, mut interval)| async move {
			interval.tick().await;
			let count = ingest.current_visitors(site.id);
			let event = Event::default().data(count.to_string());
			Some((Ok::<_, std::convert::Infallible>(event), (ingest, interval)))
		},
	);
	Sse::new(ticks)
		.keep_alive(KeepAlive::default())
		.into_response()
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
	Json(
		json!({ "setup": setup, "signup": web.app.signup_open(), "me": me, "alias": web.alias,
		"sso": web.sso.as_ref().map(|provider| &provider.name) }),
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

fn new_account(
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
	let (token, team_name) = match (
		web.app.invite(team, None, &body.email, &body.role),
		web.app.team_name(team),
	) {
		(Ok(token), Ok(team)) => (token, team),
		(Err(err), _) | (_, Err(err)) => return internal(err),
	};
	send_invite(&web, viewer.user, &team_name, &body.email, &token).await
}

pub async fn send_invite(web: &Web, user: i64, place: &str, email: &str, token: &str) -> Response {
	match mail_invite(web, user, place, email, token).await {
		Ok((link, sent)) => Json(json!({ "link": link, "sent": sent })).into_response(),
		Err(err) => internal(err),
	}
}

pub async fn mail_invite(
	web: &Web,
	user: i64,
	place: &str,
	email: &str,
	token: &str,
) -> rusqlite::Result<(String, bool)> {
	let inviter = web.app.user_name(user)?;
	let base = web.url.trim_end_matches('/');
	let link = format!("{base}/invite/{token}");
	let lines = [
		format!("{inviter} invited you to {place} on impactite."),
		"The link works for 7 days.".to_owned(),
	];
	let title = format!("Join {place}");
	let letter = Letter {
		base,
		title: &title,
		lines: &lines,
		table: None,
		button: Some(("Join", &link)),
	};
	let subject = format!("Join {place} on impactite");
	let sent = match web
		.mail
		.send(email, &subject, &letter.text(), &letter.html())
		.await
	{
		Ok(()) => web.mail.ready(),
		Err(err) => {
			tracing::warn!(%err, "invite mail");
			false
		}
	};
	Ok((link, sent))
}

#[derive(Deserialize)]
struct Accept {
	token: String,
	name: Option<String>,
	password: Option<String>,
	passkey: Option<Proof>,
}

async fn invitation(State(web): State<Arc<Web>>, Path(token): Path<String>) -> Response {
	match web.app.open_invite(&token) {
		Ok(Some(invite)) => {
			Json(json!({ "place": invite.place, "email": invite.email })).into_response()
		}
		Ok(None) => fail(
			StatusCode::NOT_FOUND,
			"This invite expired or was already used.",
		),
		Err(err) => internal(err),
	}
}

async fn accept(
	State(web): State<Arc<Web>>,
	headers: HeaderMap,
	Json(body): Json<Accept>,
) -> Response {
	let Ok(Some(Invitation {
		team,
		site,
		email,
		role,
		..
	})) = web.app.open_invite(&body.token)
	else {
		return fail(
			StatusCode::NOT_FOUND,
			"This invite expired or was already used.",
		);
	};
	let signed = cookie(&headers).and_then(|token| web.app.session_user(&token).ok().flatten());
	let user = match (signed, &body.name) {
		(Some(user), _) => user,
		(None, Some(name)) => match new_account(
			&web,
			&headers,
			(name, &email),
			body.password.as_deref(),
			body.passkey.as_ref(),
			&Purpose::Invite(body.token.clone()),
		) {
			Ok(user) => user,
			Err((status, message)) => return fail(status, message),
		},
		_ => {
			return fail(
				StatusCode::UNAUTHORIZED,
				"Sign in or create an account to join.",
			);
		}
	};
	if let Err(err) = web.app.join(team, site, user, &role, &body.token) {
		return internal(err);
	}
	signed_in(&web, user, Some(site.map_or(team, |_| 0)))
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
	let Some(domain) = clean_domain(&body.domain) else {
		return fail(StatusCode::BAD_REQUEST, "Use a domain like example.com.");
	};
	if jiff::tz::TimeZone::get(&body.timezone).is_err() {
		return fail(StatusCode::BAD_REQUEST, "Pick a timezone from the list.");
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

pub struct Invitation {
	pub team: i64,
	pub site: Option<i64>,
	pub email: String,
	pub role: String,
	pub place: String,
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

pub fn clean_domain(raw: &str) -> Option<String> {
	let domain = raw
		.trim()
		.trim_start_matches("https://")
		.trim_start_matches("http://")
		.trim_start_matches("www.")
		.trim_end_matches('/')
		.to_lowercase();
	// unicode names become punycode, the form browsers report
	let Ok(url::Host::Domain(domain)) = url::Host::parse(&domain) else {
		return None;
	};
	// a dot keeps names like `settings` or `team:1` from shadowing app pages
	let label = |label: &str| {
		(1..=63).contains(&label.len())
			&& !label.starts_with('-')
			&& !label.ends_with('-')
			&& label
				.bytes()
				.all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
	};
	((domain.contains('.') || domain == "localhost") && domain.split('.').all(label))
		.then_some(domain)
}

pub(crate) fn hash(password: &str) -> rusqlite::Result<String> {
	Argon2::default()
		.hash_password(password.as_bytes())
		.map(|hash| hash.to_string())
		.map_err(|err| rusqlite::Error::ToSqlConversionFailure(err.to_string().into()))
}

pub(crate) fn now() -> i64 {
	jiff::Timestamp::now().as_second()
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

	pub fn team_name(&self, team: i64) -> rusqlite::Result<String> {
		self.conn()
			.query_row("select name from teams where id = ?1", [team], |row| {
				row.get(0)
			})
	}

	pub fn user_name(&self, user: i64) -> rusqlite::Result<String> {
		self.conn()
			.query_row("select name from users where id = ?1", [user], |row| {
				row.get(0)
			})
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
		self.conn().execute(
			"update sites set team = ?1 where team is null and domain not like '~%'",
			[team],
		)?;
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

	pub fn editable(&self, user: i64, domain: &str) -> Option<Site> {
		let role = self.site_role(user, domain).ok().flatten()?;
		(role != "viewer").then_some(())?;
		self.site(domain).ok().flatten()
	}

	pub fn site_role(&self, user: i64, domain: &str) -> rusqlite::Result<Option<String>> {
		self.conn()
			.query_row(
				"select m.role from sites s join members m on m.team = s.team where m.user = ?1 and s.domain = ?2
				union all select g.role from sites s join guests g on g.site = s.id where g.user = ?1 and s.domain = ?2
				limit 1",
				params![user, domain],
				|row| row.get(0),
			)
			.optional()
	}

	pub fn invite(
		&self,
		team: i64,
		site: Option<u64>,
		email: &str,
		role: &str,
	) -> rusqlite::Result<String> {
		let token = token()?;
		self.conn().execute(
			"insert into invites (hash, team, site, email, role, expires_at) values (?1, ?2, ?3, ?4, ?5, ?6)",
			params![
				key_hash(&token),
				team,
				site.map(|site| site as i64),
				email.trim(),
				role,
				now() + INVITE_SECS
			],
		)?;
		Ok(token)
	}

	pub fn open_invite(&self, token: &str) -> rusqlite::Result<Option<Invitation>> {
		self.conn()
			.query_row(
				"select i.team, i.site, i.email, i.role, coalesce(s.domain, t.name) from invites i
				join teams t on t.id = i.team left join sites s on s.id = i.site
				where i.hash = ?1 and i.expires_at > ?2",
				params![key_hash(token), now()],
				|row| {
					Ok(Invitation {
						team: row.get(0)?,
						site: row.get(1)?,
						email: row.get(2)?,
						role: row.get(3)?,
						place: row.get(4)?,
					})
				},
			)
			.optional()
	}

	pub fn join(
		&self,
		team: i64,
		site: Option<i64>,
		user: i64,
		role: &str,
		token: &str,
	) -> rusqlite::Result<()> {
		let conn = self.conn();
		match site {
			Some(site) => conn.execute(
				"insert into guests (site, user, role) values (?1, ?2, ?3) on conflict do nothing",
				params![site, user, role],
			)?,
			None => conn.execute(
				"insert into members (team, user, role) values (?1, ?2, ?3) on conflict do nothing",
				params![team, user, role],
			)?,
		};
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
