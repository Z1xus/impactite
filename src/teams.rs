use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, patch, post};
use axum::{Json, Router};
use rusqlite::params;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::accounts::{ROLES, Viewer, Web, fail, internal};
use crate::app::{App, Site};
use crate::sites;

pub fn routes() -> Router<Arc<Web>> {
	Router::new()
		.route(
			"/api/app/teams/{team}",
			get(show).patch(rename).delete(destroy),
		)
		.route("/api/app/teams/{team}/members/{user}/role", patch(set_role))
		.route("/api/app/teams/{team}/owner", post(transfer))
		.route("/api/app/teams/{team}/invites/{id}", delete(revoke))
}

#[derive(Deserialize)]
struct Name {
	name: String,
}

#[derive(Deserialize)]
struct Role {
	role: String,
}

#[derive(Deserialize)]
struct Owner {
	user: i64,
}

fn role(web: &Web, user: i64, team: i64) -> Option<String> {
	web.app.role(user, team).ok().flatten()
}

fn manager(web: &Web, user: i64, team: i64) -> bool {
	matches!(role(web, user, team).as_deref(), Some("owner" | "admin"))
}

async fn show(State(web): State<Arc<Web>>, viewer: Viewer, Path(team): Path<i64>) -> Response {
	if role(&web, viewer.user, team).is_none() {
		return fail(StatusCode::NOT_FOUND, "Team not found.");
	}
	match web.app.team(team, manager(&web, viewer.user, team)) {
		Ok(team) => Json(team).into_response(),
		Err(err) => internal(err),
	}
}

async fn rename(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path(team): Path<i64>,
	Json(body): Json<Name>,
) -> Response {
	let name = body.name.trim();
	if !manager(&web, viewer.user, team) {
		return fail(
			StatusCode::FORBIDDEN,
			"Only owners and admins can rename the team.",
		);
	}
	if name.is_empty() || name.len() > 80 {
		return fail(StatusCode::BAD_REQUEST, "Give the team a name.");
	}
	match web.app.rename_team(team, name) {
		Ok(()) => StatusCode::NO_CONTENT.into_response(),
		Err(err) => internal(err),
	}
}

async fn destroy(State(web): State<Arc<Web>>, viewer: Viewer, Path(team): Path<i64>) -> Response {
	if role(&web, viewer.user, team).as_deref() != Some("owner") {
		return fail(StatusCode::FORBIDDEN, "Only the owner can delete the team.");
	}
	if web.app.is_personal(team).unwrap_or(true) {
		return fail(
			StatusCode::BAD_REQUEST,
			"Your personal workspace can't be deleted.",
		);
	}
	let sites = match web.app.team_sites(team) {
		Ok(sites) => sites,
		Err(err) => return internal(err),
	};
	for site in sites {
		if !sites::drop_site(&web, site).await {
			return fail(StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong.");
		}
	}
	match web.app.delete_team(team) {
		Ok(()) => StatusCode::NO_CONTENT.into_response(),
		Err(err) => internal(err),
	}
}

async fn set_role(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path((team, user)): Path<(i64, i64)>,
	Json(body): Json<Role>,
) -> Response {
	let current = role(&web, user, team);
	if !manager(&web, viewer.user, team) || current.is_none() || current.as_deref() == Some("owner")
	{
		return fail(StatusCode::FORBIDDEN, "You can't change this role.");
	}
	if !ROLES[1..].contains(&body.role.as_str()) {
		return fail(StatusCode::BAD_REQUEST, "Pick admin, editor or viewer.");
	}
	match web.app.set_role(team, user, &body.role) {
		Ok(()) => StatusCode::NO_CONTENT.into_response(),
		Err(err) => internal(err),
	}
}

async fn transfer(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path(team): Path<i64>,
	Json(body): Json<Owner>,
) -> Response {
	if role(&web, viewer.user, team).as_deref() != Some("owner")
		|| role(&web, body.user, team).is_none()
		|| body.user == viewer.user
	{
		return fail(
			StatusCode::FORBIDDEN,
			"Only the owner can hand over the team to a member.",
		);
	}
	if web.app.is_personal(team).unwrap_or(true) {
		return fail(
			StatusCode::BAD_REQUEST,
			"Your personal workspace stays yours.",
		);
	}
	let swapped = web
		.app
		.set_role(team, body.user, "owner")
		.and_then(|()| web.app.set_role(team, viewer.user, "admin"));
	match swapped {
		Ok(()) => StatusCode::NO_CONTENT.into_response(),
		Err(err) => internal(err),
	}
}

async fn revoke(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path((team, id)): Path<(i64, i64)>,
) -> Response {
	if !manager(&web, viewer.user, team) {
		return fail(
			StatusCode::FORBIDDEN,
			"Only owners and admins can revoke invites.",
		);
	}
	match web.app.revoke_invite(team, id) {
		Ok(()) => StatusCode::NO_CONTENT.into_response(),
		Err(err) => internal(err),
	}
}

impl App {
	pub fn team(&self, team: i64, invites: bool) -> rusqlite::Result<Value> {
		let conn = self.conn();
		let (name, personal): (String, bool) = conn.query_row(
			"select name, personal from teams where id = ?1",
			[team],
			|row| Ok((row.get(0)?, row.get(1)?)),
		)?;
		let members = conn
			.prepare(
				"select u.id, u.name, u.email, m.role from members m join users u on u.id = m.user
				where m.team = ?1 order by case m.role when 'owner' then 0 when 'admin' then 1
				when 'editor' then 2 else 3 end, u.name",
			)?
			.query_map([team], |row| {
				Ok(json!({
					"id": row.get::<_, i64>(0)?,
					"name": row.get::<_, String>(1)?,
					"email": row.get::<_, String>(2)?,
					"role": row.get::<_, String>(3)?,
				}))
			})?
			.collect::<rusqlite::Result<Vec<_>>>()?;
		let invites = match invites {
			true => conn
				.prepare(
					"select rowid, email, role, expires_at from invites
					where team = ?1 and site is null and expires_at > unixepoch() order by rowid",
				)?
				.query_map([team], |row| {
					Ok(json!({
						"id": row.get::<_, i64>(0)?,
						"email": row.get::<_, String>(1)?,
						"role": row.get::<_, String>(2)?,
						"expires": row.get::<_, i64>(3)?,
					}))
				})?
				.collect::<rusqlite::Result<Vec<_>>>()?,
			false => Vec::new(),
		};
		Ok(
			json!({ "id": team, "name": name, "personal": personal, "members": members, "invites": invites }),
		)
	}

	pub fn rename_team(&self, team: i64, name: &str) -> rusqlite::Result<()> {
		self.conn().execute(
			"update teams set name = ?2 where id = ?1",
			params![team, name],
		)?;
		Ok(())
	}

	pub fn team_sites(&self, team: i64) -> rusqlite::Result<Vec<Site>> {
		let conn = self.conn();
		let mut stmt =
			conn.prepare("select id, domain, timezone, created_at from sites where team = ?1")?;
		stmt.query_map([team], |row| {
			Ok(Site {
				id: row.get::<_, i64>(0)? as u64,
				domain: row.get(1)?,
				timezone: row.get(2)?,
				created_at: row.get(3)?,
			})
		})?
		.collect()
	}

	pub fn delete_team(&self, team: i64) -> rusqlite::Result<()> {
		self.conn()
			.execute("delete from teams where id = ?1", [team])?;
		Ok(())
	}

	pub fn set_role(&self, team: i64, user: i64, role: &str) -> rusqlite::Result<()> {
		self.conn().execute(
			"update members set role = ?3 where team = ?1 and user = ?2",
			params![team, user, role],
		)?;
		Ok(())
	}

	pub fn revoke_invite(&self, team: i64, id: i64) -> rusqlite::Result<()> {
		self.conn().execute(
			"delete from invites where team = ?1 and site is null and rowid = ?2",
			params![team, id],
		)?;
		Ok(())
	}
}
