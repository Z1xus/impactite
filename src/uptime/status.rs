use std::collections::{HashMap, HashSet};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::{Arc, LazyLock, Mutex};

use axum::extract::{ConnectInfo, Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, patch};
use axum::{Json, Router};
use futures_util::StreamExt;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::app::{App, lock, now, same, token};
use crate::auth::accounts::{Viewer, hash};
use crate::limit::Limit;
use crate::site::shares::matches;
use crate::uptime::api::{DAY, STREAM_LIFE, editor, live_of};
use crate::uptime::calc::{round, strip, uptime};
use crate::uptime::store::Scope;
use crate::uptime::{Monitor, State as Health};
use crate::web::{Error, Reply, Web, blocking};

const MONITORS: usize = 50;
static LINKS: LazyLock<Limit> = LazyLock::new(|| Limit::new(10, 10));
static CLIENTS: LazyLock<Limit> = LazyLock::new(|| Limit::new(30, 10));
static READS: LazyLock<Limit> = LazyLock::new(|| Limit::new(300, 1));
static STREAMS: LazyLock<Mutex<HashMap<String, u32>>> = LazyLock::new(Mutex::default);
const STREAMS_PER_CLIENT: u32 = 20;

fn busy(web: &Web, peer: SocketAddr, headers: &HeaderMap) -> bool {
	let client = web.client(peer, headers);
	let busy = !READS.open(&client);
	READS.strike(&client);
	busy
}

struct Stream(String);

impl Stream {
	fn open(client: String) -> Option<Self> {
		let mut streams = lock(&STREAMS);
		let count = streams.entry(client.clone()).or_default();
		if *count >= STREAMS_PER_CLIENT {
			return None;
		}
		*count += 1;
		Some(Self(client))
	}
}

impl Drop for Stream {
	fn drop(&mut self) {
		let mut streams = lock(&STREAMS);
		if let Some(count) = streams.get_mut(&self.0) {
			*count -= 1;
			if *count == 0 {
				streams.remove(&self.0);
			}
		}
	}
}

pub fn routes() -> Router<Arc<Web>> {
	Router::new()
		.route("/api/app/uptime/pages", get(list).post(add))
		.route("/api/app/uptime/pages/{id}", patch(change).delete(remove))
		.route("/api/app/status/{slug}", get(show).post(unlock))
		.route("/api/app/status/{slug}/live", get(live))
		.route("/status/{slug}", get(view))
		.route("/status/{slug}/{file}", get(badge))
}

pub struct Page {
	id: i64,
	team: i64,
	slug: String,
	name: String,
	password: String,
	monitors: Vec<i64>,
	look: Look,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
struct Look {
	theme: String,
	style: String,
	accent: String,
	brand: bool,
	home: String,
}

impl Default for Look {
	fn default() -> Self {
		Self {
			theme: "auto".to_owned(),
			style: "pixel".to_owned(),
			accent: "cyan".to_owned(),
			brand: true,
			home: String::new(),
		}
	}
}

const ACCENTS: [&str; 5] = ["cyan", "green", "blue", "violet", "mono"];

// the page's own look goes on before anything paints
const WEAR: &str = r#"<script>(()=>{const l=window.opened&&window.opened.look,d=document.documentElement.dataset;if(!l)return;const light=l.theme==="light"||(l.theme==="auto"&&matchMedia("(prefers-color-scheme: light)").matches);if(light)d.theme="light";else delete d.theme;d.style=l.style;d.accent=l.accent})()</script>"#;

fn looked(look: Look) -> Reply<Look> {
	let home = look.home.trim().to_owned();
	let fine = ["auto", "dark", "light"].contains(&look.theme.as_str())
		&& ["pixel", "plain"].contains(&look.style.as_str())
		&& ACCENTS.contains(&look.accent.as_str())
		&& (home.is_empty()
			|| (home.len() <= 300
				&& url::Url::parse(&home).is_ok_and(|url| url.scheme() == "https")));
	if !fine {
		return Err(Error::bad("Enter an https link for your website."));
	}
	Ok(Look { home, ..look })
}

impl Page {
	// changes with the password, so a new password locks out old tokens
	fn grant(&self) -> String {
		if self.password.is_empty() {
			String::new()
		} else {
			blake3::hash(format!("status:{}:{}", self.slug, self.password).as_bytes()).to_hex()
				[..32]
				.to_owned()
		}
	}

	fn token(&self) -> String {
		format!("{}.{}", self.slug, self.grant())
	}

	fn opens(&self, given: Option<&str>) -> bool {
		self.password.is_empty() || given.is_some_and(|given| same(&self.token(), given))
	}
}

#[derive(Deserialize)]
struct Shared {
	share: Option<String>,
}

// event streams cannot send headers, so the token may come in the query
fn given(headers: &HeaderMap, shared: &Shared) -> Option<String> {
	headers
		.get("x-share")
		.and_then(|value| value.to_str().ok())
		.map(str::to_owned)
		.or_else(|| shared.share.clone())
}

fn locked() -> Response {
	(
		StatusCode::UNAUTHORIZED,
		Json(json!({ "error": "This page is locked.", "locked": true })),
	)
		.into_response()
}

fn find(web: &Web, slug: &str) -> Reply<Page> {
	web.app
		.status_page(slug)?
		.ok_or_else(|| Error::not_found("This page doesn't exist."))
}

async fn payload(web: &Arc<Web>, page: &Page) -> Reply<Value> {
	let at = now();
	let monitors = web.app.page_monitors(page.id)?;
	let mut spans = web.app.spans(&Scope::Page(page.id), at - 90 * DAY)?;
	let ids = page.monitors.clone();
	let reader = web.clone();
	let mut digests = blocking(move || {
		Ok(ids
			.into_iter()
			.map(|id| (id, reader.uptime.digest(id, at)))
			.collect::<HashMap<_, _>>())
	})
	.await?;
	let shown: Vec<&Monitor> = page
		.monitors
		.iter()
		.filter_map(|id| monitors.iter().find(|monitor| monitor.id == *id))
		.collect();
	let items: Vec<Value> = shown
		.iter()
		.map(|monitor| {
			let spans = spans.remove(&monitor.id).unwrap_or_default();
			let live = live_of(monitor, web.uptime.live(monitor.id));
			let digest = digests.remove(&monitor.id).unwrap_or_default();
			let watched = digest.watched_from(monitor.created_at);
			json!({
				"id": monitor.id,
				"name": monitor.name,
				"state": live.state,
				"since": live.since,
				"uptime": uptime(&spans, watched, at - 90 * DAY, at).map(round),
				"strip": strip(&spans, watched, at, &digest.slow_days),
			})
		})
		.collect();
	let found = web.app.page_incidents(page.id, at - 30 * DAY)?;
	let mut notes = web
		.app
		.public_notes(&found.iter().map(|item| item.0).collect::<Vec<_>>())?;
	let incidents: Vec<Value> = found
		.into_iter()
		.map(|(id, monitor, started, ended, cause)| {
			json!({
				"id": id,
				"monitor": monitor,
				"started_at": started,
				"ended_at": ended,
				"cause": cause,
				"notes": notes.remove(&id).unwrap_or_default(),
			})
		})
		.collect();
	let maintenance: Vec<Value> = web
		.app
		.maintenance(&Scope::Page(page.id), at)?
		.into_iter()
		.filter(|window| {
			window["starts_at"]
				.as_i64()
				.is_some_and(|start| start < at + 7 * DAY)
		})
		.map(|mut window| {
			let ours: Vec<Value> = window["monitors"]
				.as_array()
				.into_iter()
				.flatten()
				.filter(|id| id.as_i64().is_some_and(|id| page.monitors.contains(&id)))
				.cloned()
				.collect();
			window["monitors"] = json!(ours);
			window
		})
		.collect();
	Ok(json!({
		"name": page.name,
		"look": page.look,
		"now": at,
		"monitors": items,
		"incidents": incidents,
		"maintenance": maintenance,
	}))
}

async fn show(
	State(web): State<Arc<Web>>,
	ConnectInfo(peer): ConnectInfo<SocketAddr>,
	Path(slug): Path<String>,
	headers: HeaderMap,
	Query(shared): Query<Shared>,
) -> Reply<Response> {
	if busy(&web, peer, &headers) {
		return Err(Error::too_many());
	}
	let page = find(&web, &slug)?;
	if !page.opens(given(&headers, &shared).as_deref()) {
		return Ok(locked());
	}
	Ok(Json(payload(&web, &page).await?).into_response())
}

#[derive(Deserialize)]
struct Unlock {
	#[serde(default)]
	password: String,
}

async fn unlock(
	State(web): State<Arc<Web>>,
	ConnectInfo(peer): ConnectInfo<SocketAddr>,
	headers: HeaderMap,
	Path(slug): Path<String>,
	Json(body): Json<Unlock>,
) -> Reply<Response> {
	let page = find(&web, &slug)?;
	if !page.password.is_empty() {
		let client = web.client(peer, &headers);
		if !LINKS.open(&slug) || !CLIENTS.open(&client) {
			return Err(Error::too_many());
		}
		if !matches(&page.password, &body.password).await? {
			LINKS.strike(&slug);
			CLIENTS.strike(&client);
			return Ok((
				StatusCode::UNAUTHORIZED,
				Json(json!({ "error": "Wrong password.", "locked": true })),
			)
				.into_response());
		}
	}
	let mut reply = payload(&web, &page).await?;
	reply["token"] = json!(page.token());
	Ok(Json(reply).into_response())
}

async fn live(
	State(web): State<Arc<Web>>,
	ConnectInfo(peer): ConnectInfo<SocketAddr>,
	Path(slug): Path<String>,
	headers: HeaderMap,
	Query(shared): Query<Shared>,
) -> Reply<Response> {
	if busy(&web, peer, &headers) {
		return Err(Error::too_many());
	}
	let page = find(&web, &slug)?;
	if !page.opens(given(&headers, &shared).as_deref()) {
		return Ok(locked());
	}
	let held = Stream::open(web.client(peer, &headers)).ok_or_else(Error::too_many)?;
	let receiver = web.uptime.subscribe();
	let states: HashMap<i64, Health> = web
		.app
		.page_monitors(page.id)?
		.iter()
		.map(|monitor| {
			(
				monitor.id,
				live_of(monitor, web.uptime.live(monitor.id)).state,
			)
		})
		.collect();
	let snapshot: Vec<Value> = states
		.iter()
		.map(|(id, state)| json!({ "id": id, "state": state }))
		.collect();
	let first = futures_util::stream::once(async move {
		Ok::<_, Infallible>(
			Event::default()
				.event("change")
				.json_data(snapshot)
				.unwrap_or_default(),
		)
	});
	let rest = futures_util::stream::unfold(
		(receiver, states, held),
		move |(mut receiver, mut states, held)| async move {
			loop {
				match receiver.recv().await {
					Ok(event)
						if states
							.get(&event.id)
							.is_some_and(|was| *was != event.live.state) =>
					{
						states.insert(event.id, event.live.state);
						let data = Event::default()
							.event("change")
							.json_data([json!({ "id": event.id, "state": event.live.state })])
							.unwrap_or_default();
						return Some((Ok::<_, Infallible>(data), (receiver, states, held)));
					}
					Ok(_) => {}
					// a client that fell behind starts over from a snapshot
					Err(_) => return None,
				}
			}
		},
	);
	Ok(Sse::new(
		first
			.chain(rest)
			.take_until(tokio::time::sleep(STREAM_LIFE)),
	)
	.keep_alive(KeepAlive::default())
	.into_response())
}

async fn view(
	State(web): State<Arc<Web>>,
	ConnectInfo(peer): ConnectInfo<SocketAddr>,
	Path(slug): Path<String>,
	headers: HeaderMap,
) -> Response {
	if busy(&web, peer, &headers) {
		return Error::too_many().into_response();
	}
	let opened = match web.app.status_page(&slug) {
		Ok(Some(page)) if page.password.is_empty() => payload(&web, &page).await.ok(),
		Ok(Some(_)) => Some(json!({ "locked": true })),
		_ => None,
	};
	crate::assets::page(&web, &headers, Some((WEAR, opened)))
}

fn word(state: Health) -> &'static str {
	match state {
		Health::Up => "up",
		Health::Slow => "slow",
		Health::Down => "down",
		Health::Paused => "paused",
		Health::Maintenance => "maintenance",
		Health::Pending => "no data",
	}
}

fn percent(fraction: f64) -> String {
	let value = (fraction * 10_000.0).floor() / 100.0;
	if value >= 100.0 {
		"100%".to_owned()
	} else {
		format!("{value:.2}%")
	}
}

fn milliseconds(ms: u32) -> String {
	if ms < 1000 {
		format!("{ms} ms")
	} else {
		format!("{:.1} s", f64::from(ms) / 1000.0)
	}
}

// 11 px mono text is about 6.6 px a character
fn svg(label: &str, value: &str, fill: &str, ink: &str) -> String {
	let shorten = |text: &str, max: usize| {
		if text.chars().count() > max {
			format!("{}…", text.chars().take(max - 1).collect::<String>())
		} else {
			text.to_owned()
		}
	};
	let escape = |text: &str| {
		text.replace('&', "&amp;")
			.replace('<', "&lt;")
			.replace('>', "&gt;")
			.replace('"', "&quot;")
	};
	let (label, value) = (shorten(label, 28), shorten(value, 16));
	let (one, two) = (
		label.chars().count() as f64 * 6.6,
		value.chars().count() as f64 * 6.6,
	);
	let (left, right) = (one + 16.0, two + 16.0);
	let width = left + right;
	let (label, value) = (escape(&label), escape(&value));
	format!(
		r##"<svg xmlns="http://www.w3.org/2000/svg" width="{width:.1}" height="20" role="img" aria-label="{label}: {value}" font-family="ui-monospace,monospace" font-size="11" text-anchor="middle"><title>{label}: {value}</title><clipPath id="c"><rect width="{width:.1}" height="20" rx="2"/></clipPath><g clip-path="url(#c)"><rect width="{left:.1}" height="20" fill="#0b0f13"/><rect x="{left:.1}" width="{right:.1}" height="20" fill="{fill}"/></g><rect x="0.5" y="0.5" width="{:.1}" height="19" rx="2" fill="none" stroke="#1d262e"/><text x="{:.1}" y="14" fill="#e6edf3" textLength="{one:.1}" lengthAdjust="spacing">{label}</text><text x="{:.1}" y="14" fill="{ink}" textLength="{two:.1}" lengthAdjust="spacing">{value}</text></svg>"##,
		width - 1.0,
		left / 2.0,
		left + right / 2.0,
	)
}

#[derive(Deserialize)]
struct Show {
	show: Option<String>,
}

async fn badge(
	State(web): State<Arc<Web>>,
	ConnectInfo(peer): ConnectInfo<SocketAddr>,
	headers: HeaderMap,
	Path((slug, file)): Path<(String, String)>,
	Query(query): Query<Show>,
) -> Response {
	if busy(&web, peer, &headers) {
		return Error::too_many().into_response();
	}
	let Some(id) = file
		.strip_suffix(".svg")
		.and_then(|id| id.parse::<i64>().ok())
	else {
		return StatusCode::NOT_FOUND.into_response();
	};
	let found = web
		.app
		.status_page(&slug)
		.ok()
		.flatten()
		.filter(|page| page.password.is_empty() && page.monitors.contains(&id));
	let monitor = found.and_then(|page| {
		web.app
			.monitor(id)
			.ok()
			.flatten()
			.filter(|monitor| monitor.team == page.team)
	});
	let Some(monitor) = monitor else {
		return StatusCode::NOT_FOUND.into_response();
	};
	let at = now();
	let live = live_of(&monitor, web.uptime.live(id));
	let value = match query.show.as_deref().unwrap_or("status") {
		"status" => word(live.state).to_owned(),
		"uptime" => {
			let spans = web
				.app
				.spans(&Scope::Monitor(id), at - 30 * DAY)
				.ok()
				.and_then(|mut spans| spans.remove(&id))
				.unwrap_or_default();
			let reader = web.clone();
			let digest = blocking(move || Ok(reader.uptime.digest(id, at)))
				.await
				.unwrap_or_default();
			uptime(
				&spans,
				digest.watched_from(monitor.created_at),
				at - 30 * DAY,
				at,
			)
			.map_or("n/a".to_owned(), |fraction| percent(round(fraction)))
		}
		"latency" => {
			let reader = web.clone();
			let digest = blocking(move || Ok(reader.uptime.digest(id, at)))
				.await
				.ok();
			digest
				.and_then(|digest| digest.p50)
				.filter(|_| monitor.kind != crate::uptime::Kind::Heartbeat)
				.map_or("n/a".to_owned(), milliseconds)
		}
		_ => return (StatusCode::BAD_REQUEST, "Use status, uptime or latency.").into_response(),
	};
	let (fill, ink) = match live.state {
		Health::Up => ("#38e1f2", "#06090b"),
		Health::Slow => ("#f0b64a", "#06090b"),
		Health::Down => ("#f0645a", "#06090b"),
		Health::Paused | Health::Maintenance => ("#506872", "#e3eef0"),
		Health::Pending => ("#6e818b", "#06090b"),
	};
	let mut response = (
		[(header::CONTENT_TYPE, "image/svg+xml")],
		svg(&monitor.name, &value, fill, ink),
	)
		.into_response();
	let headers = response.headers_mut();
	headers.insert(
		header::ACCESS_CONTROL_ALLOW_ORIGIN,
		HeaderValue::from_static("*"),
	);
	headers.insert(
		header::CACHE_CONTROL,
		HeaderValue::from_static("public, max-age=300"),
	);
	response
}

#[derive(Deserialize)]
struct Scoped {
	team: i64,
}

#[derive(Deserialize)]
struct Fields {
	team: Option<i64>,
	name: Option<String>,
	slug: Option<String>,
	monitors: Option<Vec<i64>>,
	// absent keeps it, empty removes it
	password: Option<String>,
	look: Option<Look>,
}

fn valid_slug(slug: &str) -> bool {
	(3..=40).contains(&slug.len())
		&& slug
			.bytes()
			.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
		&& !slug.starts_with('-')
		&& !slug.ends_with('-')
}

fn respond(web: &Web, team: i64) -> Reply {
	let known: HashSet<i64> = web
		.app
		.team_monitors(team)?
		.iter()
		.map(|(monitor, _)| monitor.id)
		.collect();
	let pages: Vec<Value> = web
		.app
		.status_pages(team)?
		.into_iter()
		.map(|page| {
			json!({
				"id": page.id,
				"slug": page.slug,
				"name": page.name,
				"locked": !page.password.is_empty(),
				"look": page.look,
				"monitors": page.monitors.iter().filter(|id| known.contains(id)).collect::<Vec<_>>(),
			})
		})
		.collect();
	Ok(Json(json!({ "pages": pages })))
}

async fn list(State(web): State<Arc<Web>>, viewer: Viewer, Query(query): Query<Scoped>) -> Reply {
	editor(&web, viewer.user, query.team)?;
	respond(&web, query.team)
}

fn chosen(web: &Web, team: i64, monitors: Vec<i64>) -> Reply<Vec<i64>> {
	let known: HashSet<i64> = web
		.app
		.team_monitors(team)?
		.iter()
		.map(|(monitor, _)| monitor.id)
		.collect();
	let mut seen = HashSet::new();
	let monitors: Vec<i64> = monitors.into_iter().filter(|id| seen.insert(*id)).collect();
	if monitors.is_empty()
		|| monitors.len() > MONITORS
		|| !monitors.iter().all(|id| known.contains(id))
	{
		return Err(Error::bad("You can choose up to 50 monitors."));
	}
	Ok(monitors)
}

fn titled(name: &str) -> Reply<String> {
	let name = name.trim();
	if name.chars().count() > 80 {
		return Err(Error::bad("This title is too long."));
	}
	Ok(name.to_owned())
}

fn addressed(slug: &str) -> Reply<String> {
	let slug = slug.trim();
	if !valid_slug(slug) {
		return Err(Error::bad("Use 3 to 40 letters, numbers or hyphens."));
	}
	Ok(slug.to_owned())
}

async fn add(State(web): State<Arc<Web>>, viewer: Viewer, Json(body): Json<Fields>) -> Reply {
	let team = body.team.ok_or_else(|| Error::bad("Choose a team."))?;
	editor(&web, viewer.user, team)?;
	let name = titled(body.name.as_deref().unwrap_or_default())?;
	let slug = match body.slug.as_deref().filter(|slug| !slug.trim().is_empty()) {
		Some(slug) => addressed(slug)?,
		None => token()?[..20].to_ascii_lowercase(),
	};
	let monitors = chosen(&web, team, body.monitors.unwrap_or_default())?;
	let password = match body.password.as_deref() {
		Some(password) if !password.is_empty() => hash(password)?,
		_ => String::new(),
	};
	let page = Page {
		id: 0,
		team,
		slug,
		name,
		password,
		monitors,
		look: looked(body.look.unwrap_or_default())?,
	};
	web.app.add_page(&page)?;
	respond(&web, team)
}

async fn change(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path(id): Path<i64>,
	Query(query): Query<Scoped>,
	Json(body): Json<Fields>,
) -> Reply {
	editor(&web, viewer.user, query.team)?;
	let mut page = web
		.app
		.status_pages(query.team)?
		.into_iter()
		.find(|page| page.id == id)
		.ok_or_else(|| Error::not_found("Page not found."))?;
	if let Some(name) = &body.name {
		page.name = titled(name)?;
	}
	if let Some(slug) = &body.slug {
		page.slug = addressed(slug)?;
	}
	if let Some(monitors) = body.monitors {
		page.monitors = chosen(&web, query.team, monitors)?;
	}
	if let Some(look) = body.look {
		page.look = looked(look)?;
	}
	if let Some(password) = &body.password {
		page.password = if password.is_empty() {
			String::new()
		} else {
			hash(password)?
		};
	}
	web.app.save_page(&page)?;
	respond(&web, query.team)
}

async fn remove(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path(id): Path<i64>,
	Query(query): Query<Scoped>,
) -> Reply {
	editor(&web, viewer.user, query.team)?;
	web.app.remove_page(query.team, id)?;
	respond(&web, query.team)
}

fn page(row: &rusqlite::Row) -> rusqlite::Result<Page> {
	let monitors: String = row.get(5)?;
	let look: String = row.get(6)?;
	Ok(Page {
		id: row.get(0)?,
		team: row.get(1)?,
		slug: row.get(2)?,
		name: row.get(3)?,
		password: row.get(4)?,
		monitors: serde_json::from_str(&monitors).unwrap_or_default(),
		look: serde_json::from_str(&look).unwrap_or_default(),
	})
}

const COLUMNS: &str = "id, team, slug, name, password, monitors, look";

fn taken(err: rusqlite::Error) -> Error {
	match err {
		rusqlite::Error::SqliteFailure(err, _)
			if err.code == rusqlite::ErrorCode::ConstraintViolation =>
		{
			Error::conflict("This address is already in use.")
		}
		err => err.into(),
	}
}

impl App {
	fn status_page(&self, slug: &str) -> rusqlite::Result<Option<Page>> {
		self.conn()
			.query_row(
				&format!("select {COLUMNS} from status_pages where slug = ?1"),
				[slug],
				page,
			)
			.optional()
	}

	fn status_pages(&self, team: i64) -> rusqlite::Result<Vec<Page>> {
		let conn = self.conn();
		let mut stmt = conn.prepare(&format!(
			"select {COLUMNS} from status_pages where team = ?1 order by id"
		))?;
		stmt.query_map([team], page)?.collect()
	}

	fn add_page(&self, page: &Page) -> Reply<()> {
		self.conn()
			.execute(
				"insert into status_pages (team, slug, name, password, monitors, look, created_at)
				values (?1, ?2, ?3, ?4, ?5, ?6, unixepoch())",
				params![
					page.team,
					page.slug,
					page.name,
					page.password,
					json!(page.monitors).to_string(),
					json!(page.look).to_string()
				],
			)
			.map_err(taken)?;
		Ok(())
	}

	fn save_page(&self, page: &Page) -> Reply<()> {
		self.conn()
			.execute(
				"update status_pages set slug = ?2, name = ?3, password = ?4, monitors = ?5, look = ?6 where id = ?1",
				params![
					page.id,
					page.slug,
					page.name,
					page.password,
					json!(page.monitors).to_string(),
					json!(page.look).to_string()
				],
			)
			.map_err(taken)?;
		Ok(())
	}

	fn remove_page(&self, team: i64, id: i64) -> rusqlite::Result<()> {
		self.conn().execute(
			"delete from status_pages where id = ?1 and team = ?2",
			params![id, team],
		)?;
		Ok(())
	}
}
