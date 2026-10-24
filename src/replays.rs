use std::collections::HashMap;
use std::convert::Infallible;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, ErrorKind, Write};
use std::net::SocketAddr;
use std::path::{Path as FilePath, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};

use axum::body::{Body, Bytes};
use axum::extract::{ConnectInfo, Path, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use flate2::Compression;
use flate2::read::MultiGzDecoder;
use flate2::write::GzEncoder;
use futures_util::StreamExt;
use rusqlite::{OptionalExtension, params};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::app::{App, lock, now};
use crate::auth::accounts::Viewer;
use crate::ingest::{ua, unpack};
use crate::limit::Limit;
use crate::web::{Error, Reply, Web, blocking};

const KEEP_DAYS: i64 = 30;
const MAX_BYTES: i64 = 5 << 20;
// so uploads from anyone can't fill the disk
const DAILY_BYTES: i64 = 256 << 20;

// room for an office of tabs behind one address, each sends every 5 s
static CLIENTS: LazyLock<Limit> = LazyLock::new(|| Limit::new(240, 1));
static SPENT: LazyLock<Mutex<HashMap<u64, (i64, i64)>>> = LazyLock::new(Mutex::default);

fn spend(site: u64, bytes: i64) -> bool {
	let day = now() / 86_400;
	let mut spent = lock(&SPENT);
	let (at, total) = spent.entry(site).or_insert((day, 0));
	if *at != day {
		(*at, *total) = (day, 0);
	}
	if *total >= DAILY_BYTES {
		return false;
	}
	*total += bytes;
	true
}

pub fn tracker() -> Router<Arc<Web>> {
	Router::new().route("/api/replay", post(record))
}

pub fn routes() -> Router<Arc<Web>> {
	Router::new()
		.route("/api/app/sites/{domain}/replays", get(list))
		.route(
			"/api/app/sites/{domain}/replays/{key}",
			get(show).delete(remove),
		)
}

fn dir(data: &FilePath, site: u64) -> PathBuf {
	data.join("replays").join(site.to_string())
}

#[derive(Deserialize)]
struct Chunk {
	d: String,
	id: String,
	e: Vec<Value>,
	u: String,
}

// what one chunk adds to the recording it belongs to
struct Seen {
	start: i64,
	end: i64,
	pages: i64,
	clicks: i64,
}

fn seen(events: &[Value]) -> Option<Seen> {
	let times = events
		.iter()
		.filter_map(|event| event["timestamp"].as_i64());
	let start = times.clone().min()?;
	let end = times.max()?;
	// rrweb: 4 is a page load, 3 with source 2 and type 2 is a click
	let pages = events.iter().filter(|event| event["type"] == 4).count() as i64;
	let clicks = events
		.iter()
		.filter(|event| {
			event["type"] == 3 && event["data"]["source"] == 2 && event["data"]["type"] == 2
		})
		.count() as i64;
	Some(Seen {
		start,
		end,
		pages,
		clicks,
	})
}

async fn record(
	State(web): State<Arc<Web>>,
	ConnectInfo(peer): ConnectInfo<SocketAddr>,
	headers: HeaderMap,
	body: Bytes,
) -> StatusCode {
	let agent = headers
		.get(header::USER_AGENT)
		.and_then(|value| value.to_str().ok())
		.unwrap_or_default();
	let visitor = web.ingest.proxies.visitor(peer, &headers);
	let client = visitor.ip.to_string();
	if !CLIENTS.open(&client) {
		return StatusCode::TOO_MANY_REQUESTS;
	}
	CLIENTS.strike(&client);
	let Some(chunk) = unpack(&body, 16 << 20)
		.filter(|_| !ua::is_bot(agent))
		.and_then(|raw| serde_json::from_slice::<Chunk>(&raw).ok())
		.filter(|chunk| {
			(12..=40).contains(&chunk.id.len())
				&& chunk.id.bytes().all(|byte| byte.is_ascii_alphanumeric())
		})
	else {
		return StatusCode::BAD_REQUEST;
	};
	let Some(site) = web
		.ingest
		.site(&chunk.d)
		.filter(|site| site.wants("replays"))
	else {
		return StatusCode::NO_CONTENT;
	};
	let Some(seen) = seen(&chunk.e) else {
		return StatusCode::NO_CONTENT;
	};
	let site = site.site.id;
	let key = chunk.id.to_ascii_lowercase();
	match web.app.replay_bytes(site, &key) {
		Ok(Some(bytes)) if bytes > MAX_BYTES => return StatusCode::NO_CONTENT,
		Err(_) => return StatusCode::INTERNAL_SERVER_ERROR,
		_ => {}
	}
	if !spend(site, body.len() as i64) {
		return StatusCode::NO_CONTENT;
	}

	let path = dir(&web.data, site).join(format!("{key}.gz"));
	let line = Value::Array(chunk.e).to_string();
	let written = blocking(move || {
		fs::create_dir_all(path.parent().unwrap_or(&path))?;
		let mut gz = GzEncoder::new(Vec::new(), Compression::default());
		gz.write_all(line.as_bytes())?;
		gz.write_all(b"\n")?;
		let packed = gz.finish()?;
		OpenOptions::new()
			.create(true)
			.append(true)
			.open(path)?
			.write_all(&packed)?;
		Ok(packed.len() as i64)
	})
	.await;
	let Ok(bytes) = written else {
		return StatusCode::INTERNAL_SERVER_ERROR;
	};
	let agent = ua::parse(agent);
	let entry =
		url::Url::parse(&chunk.u).map_or_else(|_| String::new(), |url| url.path().to_owned());
	let country = visitor
		.country
		.unwrap_or_else(|| web.ingest.country(visitor.ip));
	let saved = web.app.save_replay(&Replay {
		site,
		key: &key,
		seen,
		entry: &entry,
		country: &country,
		device: agent.device,
		browser: agent.browser,
		os: agent.os,
		bytes,
	});
	match saved {
		Ok(()) => StatusCode::NO_CONTENT,
		Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
	}
}

struct Replay<'a> {
	site: u64,
	key: &'a str,
	seen: Seen,
	entry: &'a str,
	country: &'a str,
	device: &'a str,
	browser: &'a str,
	os: &'a str,
	bytes: i64,
}

fn member(web: &Web, user: i64, domain: &str) -> Reply<u64> {
	if web.app.site_role(user, domain)?.is_none() {
		return Err(Error::not_found("This site doesn't exist."));
	}
	Ok(web
		.app
		.site(domain)?
		.ok_or_else(|| Error::not_found("This site doesn't exist."))?
		.id)
}

#[derive(Deserialize)]
struct Page {
	before: Option<i64>,
}

async fn list(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path(domain): Path<String>,
	Query(page): Query<Page>,
) -> Reply {
	let site = member(&web, viewer.user, &domain)?;
	Ok(Json(json!({
		"replays": web.app.replays(site, page.before.unwrap_or(i64::MAX))?,
	})))
}

// streamed, so a long recording never sits whole in memory
async fn show(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path((domain, key)): Path<(String, String)>,
) -> Reply<Response> {
	let site = member(&web, viewer.user, &domain)?;
	let replay = web
		.app
		.replay(site, &key)?
		.ok_or_else(|| Error::not_found("This replay was deleted."))?;
	let path =
		dir(&web.data, site).join(format!("{}.gz", replay["key"].as_str().unwrap_or_default()));
	let (send, receive) = tokio::sync::mpsc::channel::<Bytes>(4);
	tokio::task::spawn_blocking(move || {
		let Ok(file) = fs::File::open(path) else {
			return;
		};
		let mut first = true;
		for line in BufReader::new(MultiGzDecoder::new(file)).lines() {
			let Ok(line) = line else { break };
			// one line per uploaded chunk, each a json array
			let Some(events) = line
				.strip_prefix('[')
				.and_then(|rest| rest.strip_suffix(']'))
			else {
				continue;
			};
			if events.is_empty() {
				continue;
			}
			let piece = if first {
				events.to_owned()
			} else {
				format!(",{events}")
			};
			first = false;
			if send.blocking_send(Bytes::from(piece)).is_err() {
				return;
			}
		}
	});
	let head = Bytes::from(format!(r#"{{"replay":{replay},"events":["#));
	let events = futures_util::stream::unfold(receive, |mut receive| async move {
		receive.recv().await.map(|piece| (piece, receive))
	});
	let body = futures_util::stream::once(async move { head })
		.chain(events)
		.chain(futures_util::stream::once(async {
			Bytes::from_static(b"]}")
		}))
		.map(Ok::<_, Infallible>);
	Ok((
		[(header::CONTENT_TYPE, "application/json")],
		Body::from_stream(body),
	)
		.into_response())
}

async fn remove(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path((domain, key)): Path<(String, String)>,
) -> Reply<StatusCode> {
	let site = web.editable(viewer.user, &domain)?.id;
	let key = key.to_ascii_lowercase();
	web.app.conn().execute(
		"delete from replays where site = ?1 and key = ?2",
		params![site as i64, key],
	)?;
	let path = dir(&web.data, site).join(format!("{key}.gz"));
	blocking(move || gone(fs::remove_file(path))).await?;
	Ok(StatusCode::NO_CONTENT)
}

fn gone(result: std::io::Result<()>) -> std::io::Result<()> {
	match result {
		Err(err) if err.kind() != ErrorKind::NotFound => Err(err),
		_ => Ok(()),
	}
}

// every recording of the site, when it is deleted or its stats are reset
pub fn clear(app: &App, data: &FilePath, site: u64) -> std::io::Result<()> {
	app.conn()
		.execute("delete from replays where site = ?1", [site as i64])
		.map_err(std::io::Error::other)?;
	gone(fs::remove_dir_all(dir(data, site)))
}

// recordings are kept for a month
pub fn prune(app: &App, data: &FilePath) -> std::io::Result<()> {
	let cutoff = (now() - KEEP_DAYS * 86_400) * 1000;
	let old: Vec<(i64, String)> = {
		let conn = app.conn();
		let mut stmt = conn
			.prepare("delete from replays where ended_at < ?1 returning site, key")
			.map_err(std::io::Error::other)?;
		stmt.query_map([cutoff], |row| Ok((row.get(0)?, row.get(1)?)))
			.map_err(std::io::Error::other)?
			.collect::<rusqlite::Result<_>>()
			.map_err(std::io::Error::other)?
	};
	for (site, key) in old {
		gone(fs::remove_file(
			dir(data, site as u64).join(format!("{key}.gz")),
		))?;
	}
	Ok(())
}

impl App {
	fn replay_bytes(&self, site: u64, key: &str) -> rusqlite::Result<Option<i64>> {
		self.conn()
			.query_row(
				"select bytes from replays where site = ?1 and key = ?2",
				params![site as i64, key],
				|row| row.get(0),
			)
			.optional()
	}

	fn save_replay(&self, replay: &Replay) -> rusqlite::Result<()> {
		self.conn().execute(
			"insert into replays (site, key, started_at, ended_at, pages, clicks, entry, country, device, browser, os, bytes)
			values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
			on conflict (site, key) do update set
				started_at = min(started_at, excluded.started_at),
				ended_at = max(ended_at, excluded.ended_at),
				pages = pages + excluded.pages,
				clicks = clicks + excluded.clicks,
				bytes = bytes + excluded.bytes",
			params![
				replay.site as i64,
				replay.key,
				replay.seen.start,
				replay.seen.end,
				replay.seen.pages,
				replay.seen.clicks,
				replay.entry,
				replay.country,
				replay.device,
				replay.browser,
				replay.os,
				replay.bytes,
			],
		)?;
		Ok(())
	}

	fn replays(&self, site: u64, before: i64) -> rusqlite::Result<Vec<Value>> {
		let conn = self.conn();
		let mut stmt = conn.prepare(&format!(
			"select {COLUMNS} from replays where site = ?1 and started_at < ?2 order by started_at desc limit 50"
		))?;
		stmt.query_map(params![site as i64, before], row)?.collect()
	}

	fn replay(&self, site: u64, key: &str) -> rusqlite::Result<Option<Value>> {
		self.conn()
			.query_row(
				&format!("select {COLUMNS} from replays where site = ?1 and key = ?2"),
				params![site as i64, key.to_ascii_lowercase()],
				row,
			)
			.optional()
	}
}

const COLUMNS: &str =
	"key, started_at, ended_at, pages, clicks, entry, country, device, browser, os";

fn row(row: &rusqlite::Row) -> rusqlite::Result<Value> {
	Ok(json!({
		"key": row.get::<_, String>(0)?,
		"started_at": row.get::<_, i64>(1)?,
		"ended_at": row.get::<_, i64>(2)?,
		"pages": row.get::<_, i64>(3)?,
		"clicks": row.get::<_, i64>(4)?,
		"entry": row.get::<_, String>(5)?,
		"country": row.get::<_, String>(6)?,
		"device": row.get::<_, String>(7)?,
		"browser": row.get::<_, String>(8)?,
		"os": row.get::<_, String>(9)?,
	}))
}
