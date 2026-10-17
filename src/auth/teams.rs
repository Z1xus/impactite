use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, patch, post};
use axum::{Json, Router};
use rusqlite::{OptionalExtension, params};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::app::{App, Site, key_hash, now, token};
use crate::auth::accounts::{Viewer, cookie, new_account, signed_in};
use crate::auth::passkeys::{Proof, Purpose};
use crate::mail::Letter;
use crate::site;
use crate::web::{Web, broken, fail, internal};

pub const ROLES: &[&str] = &["owner", "admin", "editor", "viewer"];
const INVITE_SECS: i64 = 7 * 86_400;

pub fn routes() -> Router<Arc<Web>> {
	Router::new()
		.route("/api/app/teams", post(create_team))
		.route("/api/app/teams/{team}/invites", post(invite))
		.route(
			"/api/app/teams/{team}/members/{user}",
			delete(remove_member),
		)
		.route("/api/app/invites/accept", post(accept))
		.route("/api/app/invites/{token}", get(invitation))
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

pub(crate) fn manager(web: &Web, user: i64, team: i64) -> bool {
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
		if !site::drop_site(&web, site).await {
			return broken();
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
	if !manager(&web, viewer.user, team) {
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
	let allowed = viewer.user == user || manager(&web, viewer.user, team);
	if !allowed || web.app.role(user, team).ok().flatten().as_deref() == Some("owner") {
		return fail(StatusCode::FORBIDDEN, "You can't remove this member.");
	}
	match web.app.remove_member(team, user) {
		Ok(()) => StatusCode::NO_CONTENT.into_response(),
		Err(err) => internal(err),
	}
}

pub struct Invitation {
	pub team: i64,
	pub site: Option<i64>,
	pub email: String,
	pub role: String,
	pub place: String,
}

impl App {
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
}
