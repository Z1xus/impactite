use std::sync::Arc;

use argon2::{Argon2, PasswordVerifier};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, patch, post, put};
use axum::{Json, Router};
use rusqlite::{OptionalExtension, params};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::accounts::{Viewer, Web, fail, hash, internal, now, signed_in};
use crate::app::{App, key_hash, token};
use crate::mail::Letter;
use crate::sites;

const LINK_SECS: i64 = 15 * 60;

pub fn routes() -> Router<Arc<Web>> {
	Router::new()
		.route("/api/app/me", patch(update).delete(destroy))
		.route("/api/app/me/password", put(password))
		.route("/api/app/me/email", post(accept_email))
		.route(
			"/api/app/me/sessions",
			axum::routing::get(sessions).delete(end_others),
		)
		.route("/api/app/me/sessions/{id}", delete(end_session))
		.route("/api/app/login/link", post(send_link))
		.route("/api/app/login/link/accept", post(accept_link))
}

#[derive(Deserialize)]
struct Update {
	name: Option<String>,
	email: Option<String>,
}

#[derive(Deserialize)]
struct Password {
	current: Option<String>,
	password: String,
}

#[derive(Deserialize)]
struct Confirm {
	email: String,
}

#[derive(Deserialize)]
struct Token {
	token: String,
}

async fn update(State(web): State<Arc<Web>>, viewer: Viewer, Json(body): Json<Update>) -> Response {
	if let Some(name) = body.name.as_deref().map(str::trim) {
		if name.is_empty() || name.len() > 80 {
			return fail(StatusCode::BAD_REQUEST, "Tell us your name.");
		}
		if let Err(err) = web.app.set_user(viewer.user, "name", name) {
			return internal(err);
		}
	}
	let mut pending = None;
	if let Some(email) = body.email.as_deref().map(str::trim) {
		if !email.contains('@') {
			return fail(StatusCode::BAD_REQUEST, "Use a valid email.");
		}
		let current = match web.app.me(viewer.user) {
			Ok(me) => me["email"].as_str().unwrap_or_default().to_owned(),
			Err(err) => return internal(err),
		};
		if email.eq_ignore_ascii_case(&current) {
			return StatusCode::NO_CONTENT.into_response();
		}
		if matches!(web.app.user_by_email(email), Ok(Some(_))) {
			return fail(StatusCode::CONFLICT, "This email is already used.");
		}
		// with mail the new address confirms itself first, the old one hears about it
		if web.mail.ready() {
			return match confirm_email(&web, viewer.user, &current, email).await {
				Ok(()) => Json(json!({ "pending": email })).into_response(),
				Err(err) => {
					tracing::warn!(%err, "email change mail");
					fail(StatusCode::BAD_GATEWAY, "The mail couldn't be sent.")
				}
			};
		}
		pending = Some(email);
	}
	if let Some(email) = pending {
		match web.app.set_user(viewer.user, "email", email) {
			Err(rusqlite::Error::SqliteFailure(err, _))
				if err.code == rusqlite::ErrorCode::ConstraintViolation =>
			{
				return fail(StatusCode::CONFLICT, "This email is already used.");
			}
			Err(err) => return internal(err),
			Ok(()) => {}
		}
	}
	StatusCode::NO_CONTENT.into_response()
}

async fn confirm_email(web: &Web, user: i64, old: &str, new: &str) -> anyhow::Result<()> {
	let token = web.app.create_email_change(user, new)?;
	let base = web.url.trim_end_matches('/');
	let link = format!("{base}/email/{token}");
	let lines = [
		"Confirm this address to use it for your impactite account. The link works for a day."
			.to_owned(),
	];
	let letter = Letter {
		base,
		title: "Confirm your new email",
		lines: &lines,
		table: None,
		button: Some(("Confirm email", &link)),
	};
	web.mail
		.send(
			new,
			"Confirm your new email",
			&letter.text(),
			&letter.html(),
		)
		.await?;
	let lines = [
		format!(
			"Someone asked to change your account's email to {new}. It changes once that address confirms."
		),
		"If that wasn't you, sign in and check your account.".to_owned(),
	];
	let letter = Letter {
		base,
		title: "Your email is changing",
		lines: &lines,
		table: None,
		button: None,
	};
	web.mail
		.send(
			old,
			"Your impactite email is changing",
			&letter.text(),
			&letter.html(),
		)
		.await
}

async fn accept_email(State(web): State<Arc<Web>>, Json(body): Json<Token>) -> Response {
	let Ok(Some((user, email))) = web.app.take_email_change(&body.token) else {
		return fail(
			StatusCode::BAD_REQUEST,
			"This link expired or was already used.",
		);
	};
	match web.app.set_user(user, "email", &email) {
		Err(rusqlite::Error::SqliteFailure(err, _))
			if err.code == rusqlite::ErrorCode::ConstraintViolation =>
		{
			fail(StatusCode::CONFLICT, "This email is already used.")
		}
		Err(err) => internal(err),
		Ok(()) => Json(json!({ "email": email })).into_response(),
	}
}

async fn password(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Json(body): Json<Password>,
) -> Response {
	let stored = match web.app.password_hash(viewer.user) {
		Ok(stored) => stored,
		Err(err) => return internal(err),
	};
	let matches = body.current.as_deref().is_some_and(|current| {
		argon2::PasswordHash::new(&stored).is_ok_and(|parsed| {
			Argon2::default()
				.verify_password(current.as_bytes(), &parsed)
				.is_ok()
		})
	});
	if !stored.is_empty() && !matches {
		return fail(StatusCode::FORBIDDEN, "The current password is wrong.");
	}
	if body.password.chars().count() < 10 {
		return fail(
			StatusCode::BAD_REQUEST,
			"Use at least 10 characters for the password.",
		);
	}
	match hash(&body.password).and_then(|hash| web.app.set_user(viewer.user, "password", &hash)) {
		Ok(()) => StatusCode::NO_CONTENT.into_response(),
		Err(err) => internal(err),
	}
}

async fn sessions(State(web): State<Arc<Web>>, viewer: Viewer) -> Response {
	match web.app.sessions(viewer.user, &viewer.session) {
		Ok(sessions) => Json(json!({ "sessions": sessions })).into_response(),
		Err(err) => internal(err),
	}
}

async fn end_session(State(web): State<Arc<Web>>, viewer: Viewer, Path(id): Path<i64>) -> Response {
	match web.app.end_session_id(viewer.user, id) {
		Ok(()) => sessions(State(web), viewer).await,
		Err(err) => internal(err),
	}
}

async fn end_others(State(web): State<Arc<Web>>, viewer: Viewer) -> Response {
	match web.app.end_other_sessions(viewer.user, &viewer.session) {
		Ok(()) => sessions(State(web), viewer).await,
		Err(err) => internal(err),
	}
}

async fn destroy(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Json(body): Json<Confirm>,
) -> Response {
	let email = match web.app.user_email(viewer.user) {
		Ok(email) => email,
		Err(err) => return internal(err),
	};
	if !body.email.trim().eq_ignore_ascii_case(&email) {
		return fail(StatusCode::BAD_REQUEST, "Type your email to confirm.");
	}
	match remove_account(&web, viewer.user).await {
		Ok(()) => StatusCode::NO_CONTENT.into_response(),
		Err((status, message)) => fail(status, &message),
	}
}

pub async fn remove_account(web: &Web, user: i64) -> Result<(), (StatusCode, String)> {
	let broken = |err: rusqlite::Error| {
		tracing::error!(%err, "remove account");
		(
			StatusCode::INTERNAL_SERVER_ERROR,
			"Something went wrong.".to_owned(),
		)
	};
	let owned = web.app.owned_teams(user).map_err(broken)?;
	if let Some((_, name, _)) = owned.iter().find(|(_, _, others)| *others > 0) {
		return Err((
			StatusCode::CONFLICT,
			format!("Hand over {name} to another member first."),
		));
	}
	for (team, _, _) in owned {
		for site in web.app.team_sites(team).map_err(broken)? {
			if !sites::drop_site(web, site).await {
				return Err((
					StatusCode::INTERNAL_SERVER_ERROR,
					"Something went wrong.".to_owned(),
				));
			}
		}
		web.app.delete_team(team).map_err(broken)?;
	}
	web.app.delete_user(user).map_err(broken)
}

async fn send_link(State(web): State<Arc<Web>>, Json(body): Json<Confirm>) -> Response {
	if !web.mail.ready() {
		return fail(
			StatusCode::BAD_REQUEST,
			"Mail is not set up on this server.",
		);
	}
	let user = match web.app.user_by_email(&body.email) {
		Ok(user) => user,
		Err(err) => return internal(err),
	};
	// answer the same way for unknown emails so accounts stay private
	let Some(user) = user else {
		return StatusCode::NO_CONTENT.into_response();
	};
	let token = match web.app.create_login(user) {
		Ok(token) => token,
		Err(err) => return internal(err),
	};
	let base = web.url.trim_end_matches('/');
	let link = format!("{base}/login/link/{token}");
	let lines =
		["The link works for 15 minutes. If you didn't ask for it, ignore this mail.".to_owned()];
	let letter = Letter {
		base,
		title: "Sign in to impactite",
		lines: &lines,
		table: None,
		button: Some(("Sign in", &link)),
	};
	if let Err(err) = web
		.mail
		.send(
			&body.email,
			"Sign in to impactite",
			&letter.text(),
			&letter.html(),
		)
		.await
	{
		tracing::warn!(%err, "login mail");
		return fail(StatusCode::BAD_GATEWAY, "The mail could not be sent.");
	}
	StatusCode::NO_CONTENT.into_response()
}

async fn accept_link(State(web): State<Arc<Web>>, Json(body): Json<Token>) -> Response {
	match web.app.take_login(&body.token) {
		Ok(Some(user)) => signed_in(&web, user, None),
		Ok(None) => fail(
			StatusCode::NOT_FOUND,
			"This link expired or was already used.",
		),
		Err(err) => internal(err),
	}
}

impl App {
	fn set_user(&self, user: i64, column: &str, value: &str) -> rusqlite::Result<()> {
		self.conn().execute(
			&format!("update users set {column} = ?2 where id = ?1"),
			params![user, value],
		)?;
		Ok(())
	}

	fn password_hash(&self, user: i64) -> rusqlite::Result<String> {
		self.conn()
			.query_row("select password from users where id = ?1", [user], |row| {
				row.get(0)
			})
	}

	fn user_email(&self, user: i64) -> rusqlite::Result<String> {
		self.conn()
			.query_row("select email from users where id = ?1", [user], |row| {
				row.get(0)
			})
	}

	pub(crate) fn user_by_email(&self, email: &str) -> rusqlite::Result<Option<i64>> {
		self.conn()
			.query_row(
				"select id from users where email = ?1",
				[email.trim()],
				|row| row.get(0),
			)
			.optional()
	}

	fn sessions(&self, user: i64, current: &str) -> rusqlite::Result<Vec<Value>> {
		let conn = self.conn();
		let mut stmt = conn.prepare(
			"select rowid, hash, device, created_at, seen_at from sessions
			where user = ?1 and expires_at > unixepoch() order by seen_at desc",
		)?;
		stmt.query_map([user], |row| {
			Ok(json!({
				"id": row.get::<_, i64>(0)?,
				"current": row.get::<_, String>(1)? == current,
				"device": row.get::<_, String>(2)?,
				"created": row.get::<_, i64>(3)?,
				"seen": row.get::<_, i64>(4)?,
			}))
		})?
		.collect()
	}

	fn end_session_id(&self, user: i64, id: i64) -> rusqlite::Result<()> {
		self.conn().execute(
			"delete from sessions where user = ?1 and rowid = ?2",
			params![user, id],
		)?;
		Ok(())
	}

	fn end_other_sessions(&self, user: i64, current: &str) -> rusqlite::Result<()> {
		self.conn().execute(
			"delete from sessions where user = ?1 and hash != ?2",
			params![user, current],
		)?;
		Ok(())
	}

	fn owned_teams(&self, user: i64) -> rusqlite::Result<Vec<(i64, String, i64)>> {
		let conn = self.conn();
		let mut stmt = conn.prepare(
			"select t.id, t.name, (select count(*) from members o where o.team = t.id and o.user != ?1)
			from teams t join members m on m.team = t.id where m.user = ?1 and m.role = 'owner'",
		)?;
		stmt.query_map([user], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
			.collect()
	}

	fn delete_user(&self, user: i64) -> rusqlite::Result<()> {
		self.conn()
			.execute("delete from users where id = ?1", [user])?;
		Ok(())
	}

	fn create_login(&self, user: i64) -> rusqlite::Result<String> {
		let token = token()?;
		self.conn().execute(
			"insert into logins (hash, user, expires_at) values (?1, ?2, ?3)",
			params![key_hash(&token), user, now() + LINK_SECS],
		)?;
		Ok(token)
	}

	fn create_email_change(&self, user: i64, email: &str) -> rusqlite::Result<String> {
		let token = token()?;
		self.conn().execute(
			"insert into email_changes (hash, user, email, expires_at) values (?1, ?2, ?3, ?4)",
			params![key_hash(&token), user, email, now() + 86_400],
		)?;
		Ok(token)
	}

	fn take_email_change(&self, token: &str) -> rusqlite::Result<Option<(i64, String)>> {
		self.conn()
			.query_row(
				"delete from email_changes where hash = ?1 and expires_at > ?2 returning user, email",
				params![key_hash(token), now()],
				|row| Ok((row.get(0)?, row.get(1)?)),
			)
			.optional()
	}

	fn take_login(&self, token: &str) -> rusqlite::Result<Option<i64>> {
		self.conn()
			.query_row(
				"delete from logins where hash = ?1 and expires_at > ?2 returning user",
				params![key_hash(token), now()],
				|row| row.get(0),
			)
			.optional()
	}
}
