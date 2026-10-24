use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, patch, post};
use axum::{Json, Router};
use rusqlite::{OptionalExtension, params};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::app::{App, lock};
use crate::auth::accounts::Viewer;
use crate::auth::profile;
use crate::update::CheckError;
use crate::web::{Error, Reply, Web};

pub fn routes() -> Router<Arc<Web>> {
	Router::new()
		.route("/api/app/admin", get(show).patch(update))
		.route("/api/app/admin/mail", post(test_mail))
		.route("/api/app/admin/update-check", post(check_update))
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

// only instance admins pass, and never to act on themselves when `other` is given
fn admin(web: &Web, user: i64, other: Option<i64>) -> Reply<()> {
	if web.app.is_admin(user) && other != Some(user) {
		Ok(())
	} else {
		Err(Error::forbidden("Only server admins can do this."))
	}
}

async fn show(State(web): State<Arc<Web>>, viewer: Viewer) -> Reply {
	admin(&web, viewer.user, None)?;
	Ok(Json(json!({
		"signup": web.app.signup_open(),
		"mail": web.mail.ready(),
		"users": web.app.users()?,
		"version": crate::update::VERSION,
		"update": lock(&web.updates).clone(),
	})))
}

async fn check_update(State(web): State<Arc<Web>>, viewer: Viewer) -> Reply {
	admin(&web, viewer.user, None)?;
	crate::update::check(&web).await.map_err(|err| match err {
		CheckError::Disabled => Error::forbidden("Update checks are off."),
		CheckError::Busy => Error::conflict("An update check is already running."),
		CheckError::Recent => Error::new(
			StatusCode::TOO_MANY_REQUESTS,
			"Wait a minute before checking again.",
		),
	})?;
	show(State(web), viewer).await
}

async fn update(State(web): State<Arc<Web>>, viewer: Viewer, Json(body): Json<Update>) -> Reply {
	admin(&web, viewer.user, None)?;
	let value = if body.signup { "open" } else { "invite" };
	web.app.set_setting("signup", value)?;
	show(State(web), viewer).await
}

async fn test_mail(State(web): State<Arc<Web>>, viewer: Viewer) -> Reply {
	admin(&web, viewer.user, None)?;
	let email = web.app.user_email(viewer.user)?;
	let lines = ["impactite can send mail from this server.".to_owned()];
	let letter = crate::mail::Letter {
		base: &web.url,
		title: "Mail works",
		lines: &lines,
		table: None,
		button: None,
	};
	web.mail
		.send(
			&email,
			"impactite test mail",
			&letter.text(),
			&letter.html(),
		)
		.await
		.map_err(|err| Error::new(StatusCode::BAD_GATEWAY, err.to_string()))?;
	if !web.mail.ready() {
		return Err(Error::bad(
			"Mail isn't set up. Start the server with --mail.",
		));
	}
	Ok(Json(json!({ "sent": email })))
}

async fn set_admin(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path(user): Path<i64>,
	Json(body): Json<Admin>,
) -> Reply {
	admin(&web, viewer.user, Some(user))?;
	web.app.set_admin(user, body.admin)?;
	show(State(web), viewer).await
}

async fn remove(State(web): State<Arc<Web>>, viewer: Viewer, Path(user): Path<i64>) -> Reply {
	admin(&web, viewer.user, Some(user))?;
	profile::remove_account(&web, user).await?;
	show(State(web), viewer).await
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

	pub(crate) fn setting(&self, key: &str) -> Option<String> {
		self.conn()
			.query_row("select value from settings where key = ?1", [key], |row| {
				row.get(0)
			})
			.optional()
			.ok()
			.flatten()
	}

	pub(crate) fn set_setting(&self, key: &str, value: &str) -> rusqlite::Result<()> {
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
