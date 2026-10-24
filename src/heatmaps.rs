use std::fs;
use std::io::{ErrorKind, Write};
use std::net::SocketAddr;
use std::path::{Path as FilePath, PathBuf};
use std::sync::{Arc, LazyLock};

use axum::body::Bytes;
use axum::extract::{ConnectInfo, Path, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use flate2::Compression;
use flate2::write::GzEncoder;
use rusqlite::{OptionalExtension, params};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::app::{App, now};
use crate::auth::accounts::Viewer;
use crate::ingest::{ua, unpack};
use crate::limit::Limit;
use crate::stats::run;
use crate::web::{Error, Reply, Web, blocking};

pub const EVENT: &str = "~click";
const STALE: i64 = 7 * 86_400;
// per kind of screen
const PAGES: i64 = 3000;

static CLIENTS: LazyLock<Limit> = LazyLock::new(|| Limit::new(10, 10));

pub fn tracker() -> Router<Arc<Web>> {
	Router::new().route("/api/snapshot", get(wanted).post(upload))
}

pub fn routes() -> Router<Arc<Web>> {
	Router::new()
		.route("/api/app/heatmap", post(clicks))
		.route("/api/app/sites/{domain}/snapshot", get(snapshot))
}

pub fn class(width: f64) -> &'static str {
	if width >= 1024.0 {
		"desktop"
	} else if width >= 768.0 {
		"tablet"
	} else {
		"mobile"
	}
}

// where a click landed: pixels from the middle of the page, so centered layouts line up
// across screen sizes, and pixels from the top, both rounded to keep the values few
pub fn props(raw: &Value) -> Vec<(String, String)> {
	let number = |key: &str| raw[key].as_f64().filter(|value| value.is_finite());
	let (Some(x), Some(y), Some(width)) = (number("x"), number("y"), number("w")) else {
		return Vec::new();
	};
	let x = ((x / 4.0).round() * 4.0).clamp(-3000.0, 3000.0);
	let y = ((y / 8.0).round() * 8.0).clamp(0.0, 50_000.0);
	vec![
		("~xy".into(), format!("{x}:{y}")),
		("~w".into(), class(width).into()),
	]
}

fn dir(data: &FilePath, site: u64) -> PathBuf {
	data.join("heatmaps").join(site.to_string())
}

fn file(web: &Web, site: u64, page: &str, class: &str) -> PathBuf {
	let name = blake3::hash(format!("{page}\0{class}").as_bytes()).to_hex();
	dir(&web.data, site).join(format!("{}.json.gz", &name[..32]))
}

pub fn clear(app: &App, data: &FilePath, site: u64) -> std::io::Result<()> {
	app.conn()
		.execute("delete from snapshots where site = ?1", [site as i64])
		.map_err(std::io::Error::other)?;
	match fs::remove_dir_all(dir(data, site)) {
		Err(err) if err.kind() != ErrorKind::NotFound => Err(err),
		_ => Ok(()),
	}
}

#[derive(Deserialize)]
struct Wanted {
	d: String,
	p: String,
	w: String,
}

// a page gets a fresh snapshot once a week, for each kind of screen
fn needs(web: &Web, domain: &str, page: &str, class: &str) -> Option<u64> {
	let site = web.ingest.site(domain)?;
	if !site.wants("heatmaps") || !["desktop", "tablet", "mobile"].contains(&class) {
		return None;
	}
	let updated = web.app.snapshot_age(site.site.id, page, class).ok()?;
	updated
		.is_none_or(|at| now() - at > STALE)
		.then_some(site.site.id)
}

async fn wanted(State(web): State<Arc<Web>>, Query(ask): Query<Wanted>) -> Response {
	match needs(&web, &ask.d, &ask.p, &ask.w) {
		Some(_) => "1".into_response(),
		None => StatusCode::NO_CONTENT.into_response(),
	}
}

#[derive(Deserialize)]
struct Snapshot {
	d: String,
	p: String,
	w: String,
	width: u32,
	height: u32,
	node: Value,
}

async fn upload(
	State(web): State<Arc<Web>>,
	ConnectInfo(peer): ConnectInfo<SocketAddr>,
	headers: HeaderMap,
	body: Bytes,
) -> StatusCode {
	let client = web.ingest.proxies.visitor(peer, &headers).ip.to_string();
	if !CLIENTS.open(&client) {
		return StatusCode::TOO_MANY_REQUESTS;
	}
	CLIENTS.strike(&client);
	let agent = headers
		.get(header::USER_AGENT)
		.and_then(|value| value.to_str().ok())
		.unwrap_or_default();
	let Some(snapshot) = unpack(&body, 16 << 20)
		.filter(|_| !ua::is_bot(agent))
		.and_then(|raw| serde_json::from_slice::<Snapshot>(&raw).ok())
	else {
		return StatusCode::BAD_REQUEST;
	};
	let Some(site) = needs(&web, &snapshot.d, &snapshot.p, &snapshot.w) else {
		return StatusCode::NO_CONTENT;
	};
	if !worth(&web, site, &snapshot) {
		return StatusCode::NO_CONTENT;
	}
	let path = file(&web, site, &snapshot.p, &snapshot.w);
	let stored =
		json!({ "width": snapshot.width, "height": snapshot.height, "node": snapshot.node });
	let saved = blocking(move || {
		fs::create_dir_all(path.parent().unwrap_or(&path))?;
		let mut gz = GzEncoder::new(Vec::new(), Compression::default());
		gz.write_all(stored.to_string().as_bytes())?;
		let next = path.with_extension("tmp");
		fs::write(&next, gz.finish()?)?;
		fs::rename(next, path)
	})
	.await;
	if saved.is_err() {
		return StatusCode::INTERNAL_SERVER_ERROR;
	}
	match web.app.set_snapshot(
		site,
		&snapshot.p,
		&snapshot.w,
		snapshot.width,
		snapshot.height,
	) {
		Ok(()) => StatusCode::NO_CONTENT,
		Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
	}
}

// only pages with views this week, so made up paths are never stored
fn worth(web: &Web, site: u64, snapshot: &Snapshot) -> bool {
	if matches!(
		web.app.snapshot_age(site, &snapshot.p, &snapshot.w),
		Ok(Some(_))
	) {
		return true;
	}
	if web
		.app
		.snapshot_count(site)
		.is_ok_and(|count| count >= PAGES * 3)
	{
		return false;
	}
	run(
		&web.stats,
		json!({
			"site_id": snapshot.d,
			"metrics": ["pageviews"],
			"date_range": "7d",
			"filters": [["is", "event:page", [snapshot.p]]],
			"include": { "imports": false },
		}),
	)
	.is_ok_and(|reply| reply["results"][0]["metrics"][0].as_f64().unwrap_or(0.0) > 0.0)
}

#[derive(Deserialize)]
struct Ask {
	site_id: String,
	date_range: Value,
	#[serde(default)]
	filters: Vec<Value>,
	page: String,
	class: String,
}

async fn clicks(State(web): State<Arc<Web>>, viewer: Viewer, Json(ask): Json<Ask>) -> Reply {
	if web.app.site_role(viewer.user, &ask.site_id)?.is_none() {
		return Err(Error::not_found("This site doesn't exist."));
	}
	let mut filters = ask.filters.clone();
	filters.extend([
		json!(["is", "event:name", [EVENT]]),
		json!(["is", "event:page", [ask.page]]),
		json!(["is", "event:props:~w", [ask.class]]),
	]);
	let reply = run(
		&web.stats,
		json!({
			"site_id": ask.site_id,
			"metrics": ["events"],
			"date_range": ask.date_range,
			"dimensions": ["event:props:~xy"],
			"filters": filters,
			"include": { "imports": false },
		}),
	)
	.map_err(Error::bad)?;
	let clicks: Vec<Value> = reply["results"]
		.as_array()
		.into_iter()
		.flatten()
		.filter_map(|row| {
			let (x, y) = row["dimensions"][0].as_str()?.split_once(':')?;
			Some(json!([
				x.parse::<f64>().ok()?,
				y.parse::<f64>().ok()?,
				row["metrics"][0]
			]))
		})
		.collect();
	let site = web
		.app
		.site(&ask.site_id)?
		.ok_or_else(|| Error::not_found("This site doesn't exist."))?;
	let snapshot = web.app.snapshot_size(site.id, &ask.page, &ask.class)?;
	Ok(Json(json!({ "clicks": clicks, "snapshot": snapshot })))
}

#[derive(Deserialize)]
struct Which {
	page: String,
	class: String,
}

async fn snapshot(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path(domain): Path<String>,
	Query(which): Query<Which>,
) -> Reply<Response> {
	if web.app.site_role(viewer.user, &domain)?.is_none() {
		return Err(Error::not_found("This site doesn't exist."));
	}
	let site = web
		.app
		.site(&domain)?
		.ok_or_else(|| Error::not_found("This site doesn't exist."))?;
	let path = file(&web, site.id, &which.page, &which.class);
	let packed = blocking(move || fs::read(path))
		.await
		.map_err(|_| Error::not_found("No snapshot yet."))?;
	Ok((
		[
			(header::CONTENT_TYPE, "application/json"),
			(header::CONTENT_ENCODING, "gzip"),
			(header::CACHE_CONTROL, "private, max-age=300"),
		],
		packed,
	)
		.into_response())
}

impl App {
	fn snapshot_age(&self, site: u64, page: &str, class: &str) -> rusqlite::Result<Option<i64>> {
		self.conn()
			.query_row(
				"select updated_at from snapshots where site = ?1 and page = ?2 and class = ?3",
				params![site as i64, page, class],
				|row| row.get(0),
			)
			.optional()
	}

	fn snapshot_count(&self, site: u64) -> rusqlite::Result<i64> {
		self.conn().query_row(
			"select count(*) from snapshots where site = ?1",
			[site as i64],
			|row| row.get(0),
		)
	}

	fn snapshot_size(&self, site: u64, page: &str, class: &str) -> rusqlite::Result<Value> {
		Ok(self
			.conn()
			.query_row(
				"select width, height, updated_at from snapshots where site = ?1 and page = ?2 and class = ?3",
				params![site as i64, page, class],
				|row| {
					Ok(json!({
						"width": row.get::<_, i64>(0)?,
						"height": row.get::<_, i64>(1)?,
						"updated_at": row.get::<_, i64>(2)?,
					}))
				},
			)
			.optional()?
			.unwrap_or(Value::Null))
	}

	fn set_snapshot(
		&self,
		site: u64,
		page: &str,
		class: &str,
		width: u32,
		height: u32,
	) -> rusqlite::Result<()> {
		self.conn().execute(
			"insert into snapshots (site, page, class, width, height, updated_at) values (?1, ?2, ?3, ?4, ?5, unixepoch())
			on conflict (site, page, class) do update set width = excluded.width, height = excluded.height, updated_at = excluded.updated_at",
			params![site as i64, page, class, width, height],
		)?;
		Ok(())
	}
}
