use std::net::SocketAddr;
use std::sync::{Arc, LazyLock};

use argon2::{Argon2, PasswordVerifier};
use axum::extract::{ConnectInfo, Path, State};
use axum::http::HeaderMap;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, patch, post, put};
use axum::{Json, Router};
use rusqlite::{OptionalExtension, params};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::app::now;
use crate::app::{App, key_hash, token};
use crate::auth::accounts::{Viewer, hash, signed_in};
use crate::limit::Limit;
use crate::mail::Letter;
use crate::site;
use crate::web::{Error, Reply, Web};

const LINK_SECS: i64 = 15 * 60;

static LINKS: LazyLock<Limit> = LazyLock::new(|| Limit::new(5, 10));
static CLIENTS: LazyLock<Limit> = LazyLock::new(|| Limit::new(20, 10));

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

enum Column {
	Name,
	Email,
	Password,
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

async fn update(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Json(body): Json<Update>,
) -> Reply<Response> {
	let name = body.name.as_deref().map(str::trim);
	if name.is_some_and(|name| name.is_empty() || name.len() > 80) {
		return Err(Error::bad("Tell us your name."));
	}
	let email = body.email.as_deref().map(str::trim);
	if email.is_some_and(|email| !email.contains('@')) {
		return Err(Error::bad("Use a valid email."));
	}
	if let Some(name) = name {
		web.app.set_user(viewer.user, Column::Name, name)?;
	}
	let Some(email) = email else {
		return Ok(StatusCode::NO_CONTENT.into_response());
	};
	let current = web.app.user_email(viewer.user)?;
	if email.eq_ignore_ascii_case(&current) {
		return Ok(StatusCode::NO_CONTENT.into_response());
	}
	if web.app.user_by_email(email)?.is_some() {
		return Err(Error::conflict("This email is already used."));
	}
	// with mail the new address confirms itself first, the old one hears about it
	if web.mail.ready() {
		confirm_email(&web, viewer.user, &current, email)
			.await
			.map_err(|err| {
				tracing::warn!(%err, "email change mail");
				Error::new(StatusCode::BAD_GATEWAY, "The mail couldn't be sent.")
			})?;
		return Ok(Json(json!({ "pending": email })).into_response());
	}
	web.app
		.set_user(viewer.user, Column::Email, email)
		.map_err(used)?;
	Ok(StatusCode::NO_CONTENT.into_response())
}

// a unique email that already exists answers as a conflict
fn used(err: rusqlite::Error) -> Error {
	match err {
		rusqlite::Error::SqliteFailure(failure, _)
			if failure.code == rusqlite::ErrorCode::ConstraintViolation =>
		{
			Error::conflict("This email is already used.")
		}
		err => err.into(),
	}
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

async fn accept_email(State(web): State<Arc<Web>>, Json(body): Json<Token>) -> Reply {
	let (user, email) = web
		.app
		.take_email_change(&body.token)?
		.ok_or_else(|| Error::bad("This link expired or was already used."))?;
	web.app
		.set_user(user, Column::Email, &email)
		.map_err(used)?;
	Ok(Json(json!({ "email": email })))
}

async fn password(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Json(body): Json<Password>,
) -> Reply<StatusCode> {
	let stored = web.app.password_hash(viewer.user)?;
	let matches = body.current.as_deref().is_some_and(|current| {
		argon2::PasswordHash::new(&stored).is_ok_and(|parsed| {
			Argon2::default()
				.verify_password(current.as_bytes(), &parsed)
				.is_ok()
		})
	});
	if !stored.is_empty() && !matches {
		return Err(Error::forbidden("The current password is wrong."));
	}
	if body.password.chars().count() < 10 {
		return Err(Error::bad("Use at least 10 characters for the password."));
	}
	// a new password signs out every other device
	web.app
		.set_user(viewer.user, Column::Password, &hash(&body.password)?)?;
	web.app.end_other_sessions(viewer.user, &viewer.session)?;
	Ok(StatusCode::NO_CONTENT)
}

async fn sessions(State(web): State<Arc<Web>>, viewer: Viewer) -> Reply {
	Ok(Json(
		json!({ "sessions": web.app.sessions(viewer.user, &viewer.session)? }),
	))
}

async fn end_session(State(web): State<Arc<Web>>, viewer: Viewer, Path(id): Path<i64>) -> Reply {
	web.app.end_session_id(viewer.user, id)?;
	sessions(State(web), viewer).await
}

async fn end_others(State(web): State<Arc<Web>>, viewer: Viewer) -> Reply {
	web.app.end_other_sessions(viewer.user, &viewer.session)?;
	sessions(State(web), viewer).await
}

async fn destroy(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Json(body): Json<Confirm>,
) -> Reply<StatusCode> {
	let email = web.app.user_email(viewer.user)?;
	if !body.email.trim().eq_ignore_ascii_case(&email) {
		return Err(Error::bad("Type your email to confirm."));
	}
	remove_account(&web, viewer.user).await?;
	Ok(StatusCode::NO_CONTENT)
}

pub async fn remove_account(web: &Web, user: i64) -> Reply<()> {
	let owned = web.app.owned_teams(user)?;
	if let Some((_, name, _)) = owned.iter().find(|(_, _, others)| *others > 0) {
		return Err(Error::new(
			StatusCode::CONFLICT,
			format!("Hand over {name} to another member first."),
		));
	}
	for (team, _, _) in owned {
		for site in web.app.team_sites(team)? {
			site::drop_site(web, site).await?;
		}
		web.app.delete_team(team)?;
	}
	web.app.delete_user(user)?;
	Ok(())
}

async fn send_link(
	State(web): State<Arc<Web>>,
	ConnectInfo(peer): ConnectInfo<SocketAddr>,
	headers: HeaderMap,
	Json(body): Json<Confirm>,
) -> Reply<StatusCode> {
	if !web.mail.ready() {
		return Err(Error::bad("Mail is not set up on this server."));
	}
	let client = web.client(peer, &headers);
	let account = body.email.trim().to_lowercase();
	if !LINKS.open(&account) || !CLIENTS.open(&client) {
		return Err(Error::too_many());
	}
	LINKS.strike(&account);
	CLIENTS.strike(&client);
	// answer the same way for unknown emails so accounts stay private
	let Some(user) = web.app.user_by_email(&body.email)? else {
		return Ok(StatusCode::NO_CONTENT);
	};
	let token = web.app.create_login(user)?;
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
	web.mail
		.send(
			&body.email,
			"Sign in to impactite",
			&letter.text(),
			&letter.html(),
		)
		.await
		.map_err(|err| {
			tracing::warn!(%err, "login mail");
			Error::new(StatusCode::BAD_GATEWAY, "The mail could not be sent.")
		})?;
	Ok(StatusCode::NO_CONTENT)
}

async fn accept_link(State(web): State<Arc<Web>>, Json(body): Json<Token>) -> Reply<Response> {
	let user = web
		.app
		.take_login(&body.token)?
		.ok_or_else(|| Error::not_found("This link expired or was already used."))?;
	if web.app.totp(user)?.is_some() {
		return Err(Error::forbidden(
			"Two-factor is on. Sign in with your password and code.",
		));
	}
	signed_in(&web, user, None)
}

impl App {
	fn set_user(&self, user: i64, column: Column, value: &str) -> rusqlite::Result<()> {
		let sql = match column {
			Column::Name => "update users set name = ?2 where id = ?1",
			Column::Email => "update users set email = ?2 where id = ?1",
			Column::Password => "update users set password = ?2 where id = ?1",
		};
		self.conn().execute(sql, params![user, value])?;
		Ok(())
	}

	// sessions, invites and links nobody used before they ran out
	pub fn purge_expired(&self) -> rusqlite::Result<()> {
		let conn = self.conn();
		for table in ["sessions", "invites", "logins", "email_changes"] {
			conn.execute(
				&format!("delete from {table} where expires_at <= ?1"),
				[now()],
			)?;
		}
		Ok(())
	}

	fn password_hash(&self, user: i64) -> rusqlite::Result<String> {
		self.conn()
			.query_row("select password from users where id = ?1", [user], |row| {
				row.get(0)
			})
	}

	pub(crate) fn user_email(&self, user: i64) -> rusqlite::Result<String> {
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
