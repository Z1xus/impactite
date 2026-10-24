pub mod counter;
pub mod funnels;
pub mod goals;
pub mod google;
pub mod notes;
pub mod reports;
pub mod segments;
pub mod shares;
pub mod shields;
pub mod titles;

use std::fs;
use std::ops::RangeInclusive;
use std::sync::Arc;

use anyhow::bail;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::{delete, get, patch, post};
use axum::{Json, Router};
use crater::Dim;
use jiff::civil::Date;
use jiff::tz::TimeZone;
use rusqlite::{OptionalExtension, params};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::app::{App, Site, token};
use crate::auth::accounts::Viewer;
use crate::auth::teams::{self, manager};
use crate::import;
use crate::site::shares::Reader;
use crate::stats::day_number;
use crate::web::{Error, Reply, Web, blocking};
use crate::{heatmaps, replays};

const UPLOAD: usize = 1 << 30;

pub fn routes() -> Router<Arc<Web>> {
	Router::new()
		.route("/api/app/sites", post(add_site))
		.route("/api/app/live/{domain}", get(live))
		.route("/api/app/sites/{domain}", patch(update).delete(destroy))
		.route(
			"/api/app/sites/{domain}/imports",
			get(list).post(upload).layer(DefaultBodyLimit::max(UPLOAD)),
		)
		.route("/api/app/sites/{domain}/imports/{id}", delete(remove))
		.route("/api/app/sites/{domain}/reset", post(reset))
		.route("/api/app/sites/{domain}/strays", get(strays))
		.route("/api/app/sites/{domain}/purge", post(purge))
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
) -> Reply<StatusCode> {
	let site = web.editable(viewer.user, &domain)?;
	// everything is checked before anything changes
	if let Some(timezone) = &body.timezone
		&& TimeZone::get(timezone).is_err()
	{
		return Err(Error::bad("Use a valid timezone."));
	}
	let renamed = match &body.domain {
		Some(raw) => {
			Some(clean_domain(raw).ok_or_else(|| Error::bad("Use a domain like example.com."))?)
		}
		None => None,
	};
	if body.retention.is_some() && !manages(&web, viewer.user, &site) {
		return Err(Error::forbidden(
			"Only owners and admins can change how long data is kept.",
		));
	}
	if let Some(team) = body.team
		&& (!manages(&web, viewer.user, &site) || !manager(&web, viewer.user, team))
	{
		return Err(Error::forbidden(
			"You need to manage both teams to move a site.",
		));
	}

	if let Some(timezone) = &body.timezone {
		web.app.set_timezone(site.id, timezone)?;
	}
	if let Some(days) = body.retention {
		web.app.set_retention(site.id, days)?;
	}
	if let Some(skip) = &body.skip {
		const KNOWN: [&str; 10] = [
			"referrer", "agent", "device", "location", "region", "language", "session", "vitals",
			"heatmaps", "replays",
		];
		let skip: Vec<&str> = KNOWN
			.into_iter()
			.filter(|name| skip.iter().any(|item| item == name))
			.collect();
		web.app.set_skip(site.id, &skip.join(","))?;
	}
	if let Some(counter) = body.counter {
		web.app.set_counter(site.id, counter)?;
	}
	if let Some(domain) = renamed.filter(|domain| *domain != site.domain) {
		web.app.rename_site(site.id, &domain).map_err(taken)?;
	}
	if let Some(team) = body.team {
		web.app.move_site(site.id, team)?;
	}
	web.ingest.forget(&site.domain);
	web.uptime.sync();
	Ok(StatusCode::NO_CONTENT)
}

// a unique domain that already exists answers as a conflict
fn taken(err: rusqlite::Error) -> Error {
	match err {
		rusqlite::Error::SqliteFailure(failure, _)
			if failure.code == rusqlite::ErrorCode::ConstraintViolation =>
		{
			Error::conflict("This domain is already tracked.")
		}
		err => err.into(),
	}
}

async fn reset(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path(domain): Path<String>,
) -> Reply<StatusCode> {
	let site = managed(
		&web,
		viewer.user,
		&domain,
		"Only owners and admins can reset stats.",
	)?;
	let db = web.stats.db.clone();
	blocking(move || db.reset_site(site.id)).await?;
	let (app, data) = (web.app.clone(), web.data.clone());
	blocking(move || replays::clear(&app, &data, site.id)).await?;
	web.app.clear_imports(site.id)?;
	Ok(StatusCode::NO_CONTENT)
}

async fn purge(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path(domain): Path<String>,
	Json(body): Json<Purge>,
) -> Reply<StatusCode> {
	let site = managed(
		&web,
		viewer.user,
		&domain,
		"Only owners and admins can purge pages.",
	)?;
	let page = body.page.trim().to_owned();
	if !page.starts_with('/') {
		return Err(Error::bad("Use a path like /old-page."));
	}
	let db = web.stats.db.clone();
	blocking(move || db.purge(site.id, Dim::Page, &page)).await?;
	Ok(StatusCode::NO_CONTENT)
}

async fn guests(State(web): State<Arc<Web>>, viewer: Viewer, Path(domain): Path<String>) -> Reply {
	let site = managed(&web, viewer.user, &domain, CHANGE)?;
	respond_guests(&web, &site)
}

async fn invite_guest(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path(domain): Path<String>,
	Json(body): Json<Guest>,
) -> Reply {
	let site = managed(&web, viewer.user, &domain, CHANGE)?;
	if !matches!(body.role.as_str(), "viewer" | "editor") || !body.email.contains('@') {
		return Err(Error::bad("Use a valid email and role."));
	}
	let team = web.app.site_team(site.id)?;
	let token = web
		.app
		.invite(team, Some(site.id), &body.email, &body.role)?;
	teams::send_invite(&web, viewer.user, &site.domain, &body.email, &token).await
}

async fn remove_guest(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path((domain, user)): Path<(String, i64)>,
) -> Reply {
	let site = managed(&web, viewer.user, &domain, CHANGE)?;
	web.app.remove_guest(site.id, user)?;
	respond_guests(&web, &site)
}

async fn revoke_guest(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path((domain, id)): Path<(String, i64)>,
) -> Reply {
	let site = managed(&web, viewer.user, &domain, CHANGE)?;
	web.app.revoke_guest_invite(site.id, id)?;
	respond_guests(&web, &site)
}

fn respond_guests(web: &Web, site: &Site) -> Reply {
	Ok(Json(web.app.guests(site.id)?))
}

const CHANGE: &str = "You can't change this site.";

fn manages(web: &Web, user: i64, site: &Site) -> bool {
	web.app
		.site_team(site.id)
		.is_ok_and(|team| manager(web, user, team))
}

fn managed(web: &Web, user: i64, domain: &str, message: &str) -> Reply<Site> {
	web.app
		.site(domain)?
		.filter(|site| manages(web, user, site))
		.ok_or_else(|| Error::forbidden(message))
}

async fn strays(State(web): State<Arc<Web>>, viewer: Viewer, Path(domain): Path<String>) -> Reply {
	let site = web.editable(viewer.user, &domain)?;
	Ok(Json(json!({ "domains": web.ingest.strays(&site.domain) })))
}

async fn destroy(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path(domain): Path<String>,
) -> Reply<StatusCode> {
	let site = managed(
		&web,
		viewer.user,
		&domain,
		"Only owners and admins can delete sites.",
	)?;
	drop_site(&web, site).await?;
	Ok(StatusCode::NO_CONTENT)
}

pub async fn drop_site(web: &Web, site: Site) -> Reply<()> {
	let db = web.stats.db.clone();
	blocking(move || db.drop_site(site.id)).await?;
	let (app, data) = (web.app.clone(), web.data.clone());
	blocking(move || replays::clear(&app, &data, site.id)).await?;
	let data = web.data.clone();
	blocking(move || heatmaps::clear(&data, site.id)).await?;
	web.ingest.forget(&site.domain);
	web.app.delete_site(site.id)?;
	web.uptime.sync();
	Ok(())
}

async fn list(State(web): State<Arc<Web>>, viewer: Viewer, Path(domain): Path<String>) -> Reply {
	respond(&web, &web.editable(viewer.user, &domain)?)
}

async fn upload(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path(domain): Path<String>,
	Query(upload): Query<Upload>,
	body: Bytes,
) -> Reply {
	let site = web.editable(viewer.user, &domain)?;
	let shared = web.clone();
	let done = tokio::task::spawn_blocking(move || -> anyhow::Result<String> {
		let dir = shared.data.join("uploads");
		fs::create_dir_all(&dir)?;
		let extension = match (upload.source.as_str(), body.starts_with(&[0x1f, 0x8b])) {
			("plausible" | "impactite", _) => "zip",
			("goatcounter", true) => "csv.gz",
			("goatcounter" | "umami", _) => "csv",
			_ => bail!("Choose impactite, Plausible, GoatCounter or Umami."),
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
	.await?
	.map_err(|err| Error::bad(err.to_string()))?;
	Ok(Json(json!({ "imported": done })))
}

async fn remove(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path((domain, id)): Path<(String, i64)>,
) -> Reply {
	let site = web.editable(viewer.user, &domain)?;
	let (first, last) = web
		.app
		.remove_import(site.id, id)?
		.ok_or_else(|| Error::not_found("Import not found."))?;
	let days = days(&first, &last).ok_or_else(Error::broken)?;
	web.stats.db.drop_imports(site.id, days)?;
	respond(&web, &site)
}

fn days(first: &str, last: &str) -> Option<RangeInclusive<i32>> {
	let first: Date = first.parse().ok()?;
	let last: Date = last.parse().ok()?;
	Some(day_number(first)..=day_number(last))
}

fn respond(web: &Web, site: &Site) -> Reply {
	Ok(Json(json!({
		"imports": web
			.app
			.imports(site.id)?
			.iter()
			.map(|(id, source, first, last)| json!({ "id": id, "source": source, "first": first, "last": last }))
			.collect::<Vec<_>>()
	})))
}

impl App {
	pub fn site_team(&self, site: u64) -> rusqlite::Result<i64> {
		self.conn().query_row(
			"select team from sites where id = ?1",
			[site as i64],
			|row| row.get(0),
		)
	}

	// the public link is named after the domain, so it follows the new name
	pub fn rename_site(&self, site: u64, domain: &str) -> rusqlite::Result<()> {
		let mut conn = self.conn();
		let tx = conn.transaction()?;
		tx.execute(
			"update shares set slug = ?2 where site = ?1 and slug = (select domain from sites where id = ?1)",
			params![site as i64, domain],
		)?;
		tx.execute(
			"update monitors set name = ?2, target = 'https://' || ?2 || '/'
			where site = ?1 and target = 'https://' || (select domain from sites where id = ?1) || '/'",
			params![site as i64, domain],
		)?;
		tx.execute(
			"update sites set domain = ?2 where id = ?1",
			params![site as i64, domain],
		)?;
		tx.commit()
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
		let conn = self.conn();
		conn.execute(
			"update sites set team = ?2 where id = ?1",
			params![site as i64, team],
		)?;
		conn.execute(
			"delete from maintenance_monitors where monitor in (select id from monitors where site = ?1)",
			[site as i64],
		)?;
		conn.execute(
			"update monitors set team = ?2 where site = ?1",
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

#[derive(Deserialize)]
struct NewSite {
	team: i64,
	domain: String,
	timezone: String,
}

async fn add_site(State(web): State<Arc<Web>>, viewer: Viewer, Json(body): Json<NewSite>) -> Reply {
	if !matches!(web.app.role(viewer.user, body.team)?.as_deref(), Some(role) if role != "viewer") {
		return Err(Error::forbidden("You can't add sites to this team."));
	}
	let domain =
		clean_domain(&body.domain).ok_or_else(|| Error::bad("Use a domain like example.com."))?;
	if TimeZone::get(&body.timezone).is_err() {
		return Err(Error::bad("Pick a timezone from the list."));
	}
	let site = web
		.app
		.add_site(&domain, &body.timezone, Some(body.team))
		.map_err(taken)?;
	Ok(Json(json!({ "id": site.id, "domain": site.domain })))
}

async fn live(
	State(web): State<Arc<Web>>,
	reader: Reader,
	Path(domain): Path<String>,
) -> Reply<impl IntoResponse> {
	let site = web.readable(&reader, &domain)?;
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
	Ok(Sse::new(ticks).keep_alive(KeepAlive::default()))
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

impl App {
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
}
