use std::fs;
use std::ops::RangeInclusive;
use std::sync::Arc;

use anyhow::bail;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, patch};
use axum::{Json, Router};
use crater::Dim;
use jiff::civil::Date;
use jiff::tz::TimeZone;
use rusqlite::params;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::accounts::{self, Viewer, Web, fail};
use crate::app::{App, Site, token};
use crate::import;
use crate::stats::day_number;

const UPLOAD: usize = 1 << 30;

pub fn routes() -> Router<Arc<Web>> {
	Router::new()
		.route("/api/app/sites/{domain}", patch(update).delete(destroy))
		.route(
			"/api/app/sites/{domain}/imports",
			get(list).post(upload).layer(DefaultBodyLimit::max(UPLOAD)),
		)
		.route("/api/app/sites/{domain}/imports/{id}", delete(remove))
		.route("/api/app/sites/{domain}/reset", axum::routing::post(reset))
		.route("/api/app/sites/{domain}/purge", axum::routing::post(purge))
		.route(
			"/api/app/sites/{domain}/guests",
			get(guests).post(invite_guest),
		)
		.route(
			"/api/app/sites/{domain}/guests/{user}",
			delete(remove_guest),
		)
		.route(
			"/api/app/sites/{domain}/guests/invites/{id}",
			delete(revoke_guest),
		)
}

#[derive(Deserialize)]
struct Update {
	timezone: Option<String>,
	team: Option<i64>,
	domain: Option<String>,
	retention: Option<u32>,
	counter: Option<bool>,
	skip: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct Guest {
	email: String,
	role: String,
}

#[derive(Deserialize)]
struct Purge {
	page: String,
}

#[derive(Deserialize)]
struct Upload {
	source: String,
}

async fn update(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path(domain): Path<String>,
	Json(body): Json<Update>,
) -> Response {
	let Some(site) = editable(&web, viewer.user, &domain) else {
		return forbidden();
	};
	if let Some(timezone) = &body.timezone {
		if TimeZone::get(timezone).is_err() {
			return fail(StatusCode::BAD_REQUEST, "Use a valid timezone.");
		}
		if web.app.set_timezone(site.id, timezone).is_err() {
			return broken();
		}
		web.ingest.forget(&site.domain);
	}
	if let Some(days) = body.retention
		&& web.app.set_retention(site.id, days).is_err()
	{
		return broken();
	}
	if let Some(skip) = &body.skip {
		const KNOWN: [&str; 7] = [
			"referrer", "agent", "device", "location", "region", "language", "session",
		];
		let skip: Vec<&str> = KNOWN
			.into_iter()
			.filter(|name| skip.iter().any(|item| item == name))
			.collect();
		if web.app.set_skip(site.id, &skip.join(",")).is_err() {
			return broken();
		}
		web.ingest.forget(&site.domain);
	}
	if let Some(counter) = body.counter
		&& web.app.set_counter(site.id, counter).is_err()
	{
		return broken();
	}
	if let Some(raw) = &body.domain {
		let Some(domain) = accounts::clean_domain(raw) else {
			return fail(StatusCode::BAD_REQUEST, "Use a valid domain.");
		};
		if domain != site.domain {
			match web.app.rename_site(site.id, &domain) {
				Err(rusqlite::Error::SqliteFailure(err, _))
					if err.code == rusqlite::ErrorCode::ConstraintViolation =>
				{
					return fail(StatusCode::CONFLICT, "This domain is already tracked.");
				}
				Err(_) => return broken(),
				Ok(()) => web.ingest.forget(&site.domain),
			}
		}
	}
	if let Some(team) = body.team {
		if !manages(&web, viewer.user, &site) || !manager(&web, viewer.user, team) {
			return fail(
				StatusCode::FORBIDDEN,
				"You need to manage both teams to move a site.",
			);
		}
		if web.app.move_site(site.id, team).is_err() {
			return broken();
		}
	}
	StatusCode::NO_CONTENT.into_response()
}

async fn reset(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path(domain): Path<String>,
) -> Response {
	let Some(site) = managed(&web, viewer.user, &domain) else {
		return fail(
			StatusCode::FORBIDDEN,
			"Only owners and admins can reset stats.",
		);
	};
	let db = web.stats.db.clone();
	let id = site.id;
	if !matches!(
		tokio::task::spawn_blocking(move || db.reset_site(id)).await,
		Ok(Ok(()))
	) {
		return broken();
	}
	match web.app.clear_imports(site.id) {
		Ok(()) => StatusCode::NO_CONTENT.into_response(),
		Err(_) => broken(),
	}
}

async fn purge(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path(domain): Path<String>,
	Json(body): Json<Purge>,
) -> Response {
	let Some(site) = managed(&web, viewer.user, &domain) else {
		return fail(
			StatusCode::FORBIDDEN,
			"Only owners and admins can purge pages.",
		);
	};
	let page = body.page.trim().to_owned();
	if !page.starts_with('/') {
		return fail(StatusCode::BAD_REQUEST, "Use a path like /old-page.");
	}
	let db = web.stats.db.clone();
	match tokio::task::spawn_blocking(move || db.purge(site.id, Dim::Page, &page)).await {
		Ok(Ok(())) => StatusCode::NO_CONTENT.into_response(),
		_ => broken(),
	}
}

async fn guests(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path(domain): Path<String>,
) -> Response {
	match managed(&web, viewer.user, &domain) {
		Some(site) => respond_guests(&web, &site),
		None => forbidden(),
	}
}

async fn invite_guest(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path(domain): Path<String>,
	Json(body): Json<Guest>,
) -> Response {
	let Some(site) = managed(&web, viewer.user, &domain) else {
		return forbidden();
	};
	if !matches!(body.role.as_str(), "viewer" | "editor") || !body.email.contains('@') {
		return fail(StatusCode::BAD_REQUEST, "Use a valid email and role.");
	}
	let token = web
		.app
		.site_team(site.id)
		.and_then(|team| web.app.invite(team, Some(site.id), &body.email, &body.role));
	match token {
		Ok(token) => {
			accounts::send_invite(&web, viewer.user, &site.domain, &body.email, &token).await
		}
		Err(_) => broken(),
	}
}

async fn remove_guest(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path((domain, user)): Path<(String, i64)>,
) -> Response {
	let Some(site) = managed(&web, viewer.user, &domain) else {
		return forbidden();
	};
	match web.app.remove_guest(site.id, user) {
		Ok(()) => respond_guests(&web, &site),
		Err(_) => broken(),
	}
}

async fn revoke_guest(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path((domain, id)): Path<(String, i64)>,
) -> Response {
	let Some(site) = managed(&web, viewer.user, &domain) else {
		return forbidden();
	};
	match web.app.revoke_guest_invite(site.id, id) {
		Ok(()) => respond_guests(&web, &site),
		Err(_) => broken(),
	}
}

fn respond_guests(web: &Web, site: &Site) -> Response {
	match web.app.guests(site.id) {
		Ok(guests) => Json(guests).into_response(),
		Err(_) => broken(),
	}
}

fn manager(web: &Web, user: i64, team: i64) -> bool {
	matches!(
		web.app.role(user, team).ok().flatten().as_deref(),
		Some("owner" | "admin")
	)
}

fn manages(web: &Web, user: i64, site: &Site) -> bool {
	web.app
		.site_team(site.id)
		.is_ok_and(|team| manager(web, user, team))
}

fn managed(web: &Web, user: i64, domain: &str) -> Option<Site> {
	let site = web.app.site(domain).ok().flatten()?;
	manages(web, user, &site).then_some(site)
}

async fn destroy(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path(domain): Path<String>,
) -> Response {
	let role = web.app.site_role(viewer.user, &domain).ok().flatten();
	let site = web.app.site(&domain).ok().flatten();
	let (Some("owner" | "admin"), Some(site)) = (role.as_deref(), site) else {
		return fail(
			StatusCode::FORBIDDEN,
			"Only owners and admins can delete sites.",
		);
	};
	match drop_site(&web, site).await {
		true => StatusCode::NO_CONTENT.into_response(),
		false => broken(),
	}
}

pub async fn drop_site(web: &Web, site: Site) -> bool {
	let db = web.stats.db.clone();
	let dropped = tokio::task::spawn_blocking(move || db.drop_site(site.id)).await;
	if !matches!(dropped, Ok(Ok(()))) {
		return false;
	}
	web.ingest.forget(&site.domain);
	web.app.delete_site(site.id).is_ok()
}

async fn list(State(web): State<Arc<Web>>, viewer: Viewer, Path(domain): Path<String>) -> Response {
	match editable(&web, viewer.user, &domain) {
		Some(site) => respond(&web, &site),
		None => forbidden(),
	}
}

async fn upload(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path(domain): Path<String>,
	Query(upload): Query<Upload>,
	body: Bytes,
) -> Response {
	let Some(site) = editable(&web, viewer.user, &domain) else {
		return forbidden();
	};
	let shared = web.clone();
	let done = tokio::task::spawn_blocking(move || -> anyhow::Result<String> {
		let dir = shared.data.join("uploads");
		fs::create_dir_all(&dir)?;
		let extension = match (upload.source.as_str(), body.starts_with(&[0x1f, 0x8b])) {
			("plausible" | "impactite", _) => "zip",
			("goatcounter", true) => "csv.gz",
			("goatcounter", false) => "csv",
			_ => bail!("Choose impactite, Plausible or GoatCounter."),
		};
		let path = dir.join(format!("{}.{extension}", token()?));
		fs::write(&path, &body)?;
		let db = &shared.stats.db;
		let done = match upload.source.as_str() {
			"plausible" => import::plausible_zip(db, &shared.app, &site, &path)
				.map(|days| format!("{days} days")),
			"umami" => import::umami_csv(db, &shared.app, &site, &path)
				.map(|count| format!("{count} events")),
			"impactite" => import::native_zip(db, &shared.app, &site, &path)
				.map(|events| format!("{events} events")),
			_ => import::goatcounter_csv(db, &shared.app, &site, &path)
				.map(|events| format!("{events} events")),
		};
		let _ = fs::remove_file(&path);
		done
	})
	.await;
	match done {
		Ok(Ok(done)) => Json(json!({ "imported": done })).into_response(),
		Ok(Err(err)) => fail(StatusCode::BAD_REQUEST, &err.to_string()),
		Err(_) => broken(),
	}
}

async fn remove(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path((domain, id)): Path<(String, i64)>,
) -> Response {
	let Some(site) = editable(&web, viewer.user, &domain) else {
		return forbidden();
	};
	let Ok(Some((first, last))) = web.app.remove_import(site.id, id) else {
		return fail(StatusCode::NOT_FOUND, "Import not found.");
	};
	match days(&first, &last).map(|days| web.stats.db.drop_imports(site.id, days)) {
		Some(Ok(())) => respond(&web, &site),
		_ => broken(),
	}
}

fn days(first: &str, last: &str) -> Option<RangeInclusive<i32>> {
	let first: Date = first.parse().ok()?;
	let last: Date = last.parse().ok()?;
	Some(day_number(first)..=day_number(last))
}

fn editable(web: &Web, user: i64, domain: &str) -> Option<Site> {
	let role = web.app.site_role(user, domain).ok().flatten()?;
	(role != "viewer").then_some(())?;
	web.app.site(domain).ok().flatten()
}

fn respond(web: &Web, site: &Site) -> Response {
	match web.app.imports(site.id) {
		Ok(imports) => Json(json!({
			"imports": imports
				.iter()
				.map(|(id, source, first, last)| json!({ "id": id, "source": source, "first": first, "last": last }))
				.collect::<Vec<_>>()
		}))
		.into_response(),
		Err(_) => broken(),
	}
}

fn forbidden() -> Response {
	fail(StatusCode::FORBIDDEN, "You can't change this site.")
}

fn broken() -> Response {
	fail(StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong.")
}

impl App {
	pub fn site_team(&self, site: u64) -> rusqlite::Result<i64> {
		self.conn().query_row(
			"select team from sites where id = ?1",
			[site as i64],
			|row| row.get(0),
		)
	}

	pub fn rename_site(&self, site: u64, domain: &str) -> rusqlite::Result<()> {
		self.conn().execute(
			"update sites set domain = ?2 where id = ?1",
			params![site as i64, domain],
		)?;
		Ok(())
	}

	pub fn set_counter(&self, site: u64, counter: bool) -> rusqlite::Result<()> {
		self.conn().execute(
			"update sites set counter = ?2 where id = ?1",
			params![site as i64, counter],
		)?;
		Ok(())
	}

	pub fn set_skip(&self, site: u64, skip: &str) -> rusqlite::Result<()> {
		self.conn().execute(
			"update sites set skip = ?2 where id = ?1",
			params![site as i64, skip],
		)?;
		Ok(())
	}

	pub fn skip(&self, site: u64) -> rusqlite::Result<String> {
		self.conn().query_row(
			"select skip from sites where id = ?1",
			[site as i64],
			|row| row.get(0),
		)
	}

	pub fn counter(&self, site: u64) -> rusqlite::Result<bool> {
		self.conn().query_row(
			"select counter from sites where id = ?1",
			[site as i64],
			|row| row.get(0),
		)
	}

	pub fn set_retention(&self, site: u64, days: u32) -> rusqlite::Result<()> {
		self.conn().execute(
			"update sites set retention = ?2 where id = ?1",
			params![site as i64, days],
		)?;
		Ok(())
	}

	pub fn clear_imports(&self, site: u64) -> rusqlite::Result<()> {
		self.conn()
			.execute("delete from imports where site = ?1", [site as i64])?;
		Ok(())
	}

	pub fn retentions(&self) -> rusqlite::Result<Vec<(Site, i64)>> {
		let conn = self.conn();
		let mut stmt = conn.prepare(
			"select id, domain, timezone, created_at, retention from sites
			where team is not null and retention > 0",
		)?;
		stmt.query_map([], |row| {
			Ok((
				Site {
					id: row.get::<_, i64>(0)? as u64,
					domain: row.get(1)?,
					timezone: row.get(2)?,
					created_at: row.get(3)?,
				},
				row.get(4)?,
			))
		})?
		.collect()
	}

	pub fn move_site(&self, site: u64, team: i64) -> rusqlite::Result<()> {
		self.conn().execute(
			"update sites set team = ?2 where id = ?1",
			params![site as i64, team],
		)?;
		Ok(())
	}

	pub fn guests(&self, site: u64) -> rusqlite::Result<Value> {
		let conn = self.conn();
		let guests = conn
			.prepare(
				"select u.id, u.name, u.email, g.role from guests g join users u on u.id = g.user
				where g.site = ?1 order by u.name",
			)?
			.query_map([site as i64], |row| {
				Ok(json!({
					"id": row.get::<_, i64>(0)?,
					"name": row.get::<_, String>(1)?,
					"email": row.get::<_, String>(2)?,
					"role": row.get::<_, String>(3)?,
				}))
			})?
			.collect::<rusqlite::Result<Vec<_>>>()?;
		let invites = conn
			.prepare(
				"select rowid, email, role from invites where site = ?1 and expires_at > unixepoch() order by rowid",
			)?
			.query_map([site as i64], |row| {
				Ok(json!({
					"id": row.get::<_, i64>(0)?,
					"email": row.get::<_, String>(1)?,
					"role": row.get::<_, String>(2)?,
				}))
			})?
			.collect::<rusqlite::Result<Vec<_>>>()?;
		Ok(json!({ "guests": guests, "invites": invites }))
	}

	pub fn remove_guest(&self, site: u64, user: i64) -> rusqlite::Result<()> {
		self.conn().execute(
			"delete from guests where site = ?1 and user = ?2",
			params![site as i64, user],
		)?;
		Ok(())
	}

	pub fn revoke_guest_invite(&self, site: u64, id: i64) -> rusqlite::Result<()> {
		self.conn().execute(
			"delete from invites where site = ?1 and rowid = ?2",
			params![site as i64, id],
		)?;
		Ok(())
	}
}
