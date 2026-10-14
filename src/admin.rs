use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, patch, post};
use axum::{Json, Router};
use rusqlite::{OptionalExtension, params};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::accounts::{Viewer, Web, fail, internal};
use crate::app::App;
use crate::profile;

pub fn routes() -> Router<Arc<Web>> {
	Router::new()
		.route("/api/app/admin", get(show).patch(update))
		.route("/api/app/admin/mail", post(test_mail))
		.route(
			"/api/app/admin/users/{user}",
			patch(set_admin).delete(remove),
		)
}

#[derive(Deserialize)]
struct Update {
	signup: bool,
}

#[derive(Deserialize)]
struct Admin {
	admin: bool,
}

fn denied() -> Response {
	fail(StatusCode::FORBIDDEN, "Only instance admins can do this.")
}

async fn show(State(web): State<Arc<Web>>, viewer: Viewer) -> Response {
	if !web.app.is_admin(viewer.user) {
		return denied();
	}
	match web.app.users() {
		Ok(users) => Json(json!({
			"signup": web.app.signup_open(),
			"mail": web.mail.ready(),
			"users": users,
		}))
		.into_response(),
		Err(err) => internal(err),
	}
}

async fn update(State(web): State<Arc<Web>>, viewer: Viewer, Json(body): Json<Update>) -> Response {
	if !web.app.is_admin(viewer.user) {
		return denied();
	}
	let value = if body.signup { "open" } else { "invite" };
	match web.app.set_setting("signup", value) {
		Ok(()) => show(State(web), viewer).await,
		Err(err) => internal(err),
	}
}

async fn test_mail(State(web): State<Arc<Web>>, viewer: Viewer) -> Response {
	if !web.app.is_admin(viewer.user) {
		return denied();
	}
	let email = match web.app.me(viewer.user) {
		Ok(me) => me["email"].as_str().unwrap_or_default().to_owned(),
		Err(err) => return internal(err),
	};
	let text = "Mail from Impactite works.";
	match web
		.mail
		.send(
			&email,
			"Impactite test mail",
			text,
			&format!("<p>{text}</p>"),
		)
		.await
	{
		Ok(()) if web.mail.ready() => Json(json!({ "sent": email })).into_response(),
		Ok(()) => fail(
			StatusCode::BAD_REQUEST,
			"Mail is not set up. Start the server with --mail.",
		),
		Err(err) => fail(StatusCode::BAD_GATEWAY, &err.to_string()),
	}
}

async fn set_admin(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path(user): Path<i64>,
	Json(body): Json<Admin>,
) -> Response {
	if !web.app.is_admin(viewer.user) || user == viewer.user {
		return denied();
	}
	match web.app.set_admin(user, body.admin) {
		Ok(()) => show(State(web), viewer).await,
		Err(err) => internal(err),
	}
}

async fn remove(State(web): State<Arc<Web>>, viewer: Viewer, Path(user): Path<i64>) -> Response {
	if !web.app.is_admin(viewer.user) || user == viewer.user {
		return denied();
	}
	match profile::remove_account(&web, user).await {
		Ok(()) => show(State(web), viewer).await,
		Err((status, message)) => fail(status, &message),
	}
}

impl App {
	pub fn is_admin(&self, user: i64) -> bool {
		self.conn()
			.query_row("select admin from users where id = ?1", [user], |row| {
				row.get(0)
			})
			.unwrap_or(false)
	}

	pub fn signup_open(&self) -> bool {
		self.setting("signup").as_deref() == Some("open")
	}

	fn setting(&self, key: &str) -> Option<String> {
		self.conn()
			.query_row("select value from settings where key = ?1", [key], |row| {
				row.get(0)
			})
			.optional()
			.ok()
			.flatten()
	}

	fn set_setting(&self, key: &str, value: &str) -> rusqlite::Result<()> {
		self.conn().execute(
			"insert into settings (key, value) values (?1, ?2) on conflict (key) do update set value = ?2",
			params![key, value],
		)?;
		Ok(())
	}

	fn set_admin(&self, user: i64, admin: bool) -> rusqlite::Result<()> {
		self.conn().execute(
			"update users set admin = ?2 where id = ?1",
			params![user, admin],
		)?;
		Ok(())
	}

	fn users(&self) -> rusqlite::Result<Vec<Value>> {
		let conn = self.conn();
		let mut stmt = conn.prepare(
			"select u.id, u.name, u.email, u.admin, u.created_at,
			(select count(*) from sites s join members m on m.team = s.team where m.user = u.id)
			from users u order by u.id",
		)?;
		stmt.query_map([], |row| {
			Ok(json!({
				"id": row.get::<_, i64>(0)?,
				"name": row.get::<_, String>(1)?,
				"email": row.get::<_, String>(2)?,
				"admin": row.get::<_, bool>(3)?,
				"created": row.get::<_, i64>(4)?,
				"sites": row.get::<_, i64>(5)?,
			}))
		})?
		.collect()
	}
}
