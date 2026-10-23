use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, anyhow};
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Query, State};
use axum::routing::post;
use axum::{Json, Router};
use jiff::Timestamp;
use jiff::civil::DateTime;
use jiff::tz::TimeZone;
use rusqlite::{Connection, OpenFlags, Transaction, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::app::{App, now, token};
use crate::auth::accounts::Viewer;
use crate::outbound;
use crate::uptime::api::{DAY, INTERVALS, editor, parse, private};
use crate::uptime::log::{Check, Hour, Log};
use crate::uptime::probe::OTHER;
use crate::uptime::{Config, Kind, State as Health};
use crate::web::{Error, Reply, Web};

const UPLOAD: usize = 1 << 30;

pub fn routes() -> Router<Arc<Web>> {
	Router::new().route(
		"/api/app/uptime/import",
		post(upload).layer(DefaultBodyLimit::max(UPLOAD)),
	)
}

#[derive(Default, Serialize)]
pub struct Summary {
	pub monitors: u32,
	pub existing: u32,
	pub checks: u64,
	pub incidents: u32,
	pub alerts: u32,
	pub maintenance: u32,
	pub pages: u32,
	pub skipped: Vec<Skip>,
	pub noted: Vec<Skip>,
}

#[derive(Serialize)]
pub struct Skip {
	pub name: String,
	pub reason: String,
}

impl Summary {
	fn skip(&mut self, name: &str, reason: impl Into<String>) {
		self.skipped.push(Skip {
			name: name.to_owned(),
			reason: reason.into(),
		});
	}

	fn note(&mut self, name: &str, reason: &str) {
		self.noted.push(Skip {
			name: name.to_owned(),
			reason: reason.to_owned(),
		});
	}
}

#[derive(Deserialize)]
struct Upload {
	team: i64,
	#[serde(default)]
	dry: bool,
}

async fn upload(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Query(query): Query<Upload>,
	body: Bytes,
) -> Reply {
	editor(&web, viewer.user, query.team)?;
	let (shared, user, team, dry) = (web.clone(), viewer.user, query.team, query.dry);
	let summary = tokio::task::spawn_blocking(move || -> anyhow::Result<Summary> {
		let dir = shared.data.join("uploads");
		fs::create_dir_all(&dir)?;
		let path = dir.join(format!("{}.db", token()?));
		fs::write(&path, &body)?;
		let done = kuma(&shared.app, &shared.uptime.log, &path, team, user, dry);
		let _ = fs::remove_file(&path);
		done
	})
	.await?
	.map_err(|err| Error::bad(err.to_string()))?;
	web.uptime.sync();
	Ok(Json(json!(summary)))
}

#[derive(Default)]
struct Source {
	id: i64,
	name: String,
	kind: String,
	active: bool,
	interval: i64,
	url: String,
	hostname: String,
	port: i64,
	keyword: String,
	invert: bool,
	ignore_tls: bool,
	upside_down: bool,
	codes: String,
	token: Option<String>,
	method: String,
	body: String,
	headers: String,
	timeout: f64,
	created: String,
	auth: String,
}

struct Plan {
	kind: Kind,
	target: String,
	config: Config,
	interval: i64,
	token: Option<String>,
	notes: Vec<&'static str>,
}

fn columns(conn: &Connection, table: &str) -> rusqlite::Result<HashSet<String>> {
	let mut stmt = conn.prepare(&format!("pragma table_info(\"{table}\")"))?;
	stmt.query_map([], |row| row.get(1))?.collect()
}

fn select(have: &HashSet<String>, wanted: &[&str]) -> String {
	wanted
		.iter()
		.map(|name| {
			if have.contains(*name) {
				format!("\"{name}\"")
			} else {
				"null".to_owned()
			}
		})
		.collect::<Vec<_>>()
		.join(", ")
}

fn sources(kuma: &Connection) -> rusqlite::Result<Vec<Source>> {
	let have = columns(kuma, "monitor")?;
	let list = select(
		&have,
		&[
			"id",
			"name",
			"type",
			"active",
			"interval",
			"url",
			"hostname",
			"port",
			"keyword",
			"invert_keyword",
			"ignore_tls",
			"upside_down",
			"accepted_statuscodes_json",
			"push_token",
			"method",
			"body",
			"headers",
			"timeout",
			"created_date",
			"auth_method",
		],
	);
	let mut stmt = kuma.prepare(&format!("select {list} from monitor order by id"))?;
	stmt.query_map([], |row| {
		let text = |at| {
			row.get::<_, Option<String>>(at)
				.map(Option::unwrap_or_default)
		};
		let flag = |at| row.get::<_, Option<i64>>(at).map(|n| n.unwrap_or(0) != 0);
		Ok(Source {
			id: row.get(0)?,
			name: text(1)?,
			kind: text(2)?,
			active: flag(3)?,
			interval: row.get::<_, Option<i64>>(4)?.unwrap_or(60),
			url: text(5)?,
			hostname: text(6)?,
			port: row.get::<_, Option<i64>>(7)?.unwrap_or(0),
			keyword: text(8)?,
			invert: flag(9)?,
			ignore_tls: flag(10)?,
			upside_down: flag(11)?,
			codes: text(12)?,
			token: row.get(13)?,
			method: text(14)?,
			body: text(15)?,
			headers: text(16)?,
			timeout: row.get::<_, Option<f64>>(17)?.unwrap_or(0.0),
			created: text(18)?,
			auth: text(19)?,
		})
	})?
	.collect()
}

fn snap(seconds: i64) -> i64 {
	*INTERVALS
		.iter()
		.min_by_key(|interval| (interval.abs_diff(seconds), **interval))
		.unwrap_or(&60)
}

fn plan(m: &Source, admin: bool) -> Result<Plan, String> {
	let mut notes = Vec::new();
	let mut config = Config::default();
	let (kind, target, host) = match m.kind.as_str() {
		"http" | "keyword" => {
			let (kind, target, host) = parse(&m.url)
				.filter(|(kind, _, _)| *kind == Kind::Http)
				.ok_or("the address is not valid")?;
			config.method = m.method.to_uppercase();
			if config.method.is_empty() {
				config.method = "GET".to_owned();
			}
			if !["GET", "HEAD", "POST"].contains(&config.method.as_str()) {
				return Err(format!("{} requests are not supported", config.method));
			}
			if config.method == "POST" && !m.body.trim().is_empty() {
				return Err("a request body is not supported".to_owned());
			}
			if let Ok(codes) = serde_json::from_str::<Vec<String>>(&m.codes)
				&& !codes.is_empty()
			{
				config.status = codes.join(",");
			}
			if let Ok(Value::Object(headers)) = serde_json::from_str(&m.headers) {
				config.headers = headers
					.into_iter()
					.map(|(name, value)| {
						let value = value
							.as_str()
							.map_or_else(|| value.to_string(), str::to_owned);
						(name, value)
					})
					.collect();
			}
			if m.kind == "keyword" {
				if m.invert {
					config.lacks = m.keyword.clone();
				} else {
					config.contains = m.keyword.clone();
				}
			}
			if m.timeout >= 1.0 {
				config.timeout = (m.timeout.round() as u64).clamp(1, 60);
			}
			if m.ignore_tls {
				notes.push("TLS errors are not ignored");
			}
			if !m.auth.is_empty() {
				notes.push("sign in details are not kept");
			}
			(kind, target, host)
		}
		"port" => {
			let (kind, target, host) = parse(&format!("{}:{}", m.hostname, m.port))
				.filter(|(kind, _, _)| *kind == Kind::Tcp)
				.ok_or("the host or port is not valid")?;
			(kind, target, host)
		}
		"push" => {
			if m.token.as_deref().is_none_or(str::is_empty) {
				return Err("it has no push token".to_owned());
			}
			(Kind::Heartbeat, String::new(), String::new())
		}
		"group" => return Err("a group, its monitors are imported".to_owned()),
		"json-query" => return Err("json queries are not supported".to_owned()),
		other => return Err(format!("{other} monitors are not supported")),
	};
	if m.upside_down {
		return Err("upside down mode is not supported".to_owned());
	}
	if private(&host) && !admin {
		return Err("local addresses need an instance admin".to_owned());
	}
	config.check().map_err(str::to_owned)?;
	let interval = if kind == Kind::Heartbeat {
		m.interval.clamp(60, 7 * DAY)
	} else {
		snap(m.interval)
	};
	Ok(Plan {
		kind,
		target,
		config,
		interval,
		token: m.token.clone().filter(|_| kind == Kind::Heartbeat),
		notes,
	})
}

fn unix(text: &str, zone: &TimeZone) -> Option<i64> {
	if let Ok(at) = text.parse::<Timestamp>() {
		return Some(at.as_second());
	}
	let civil: DateTime = text.get(..19)?.parse().ok()?;
	Some(civil.to_zoned(zone.clone()).ok()?.timestamp().as_second())
}

fn cause(msg: &str) -> String {
	let lower = msg.to_lowercase();
	let code = lower
		.split_once("status code ")
		.map(|(_, rest)| rest)
		.or_else(|| lower.split_once(" - ").map(|(first, _)| first))
		.map(|rest| rest.chars().take(3).collect::<String>())
		.filter(|code| code.len() == 3 && code.bytes().all(|byte| byte.is_ascii_digit()));
	if let Some(code) = code {
		format!("HTTP {code}")
	} else if lower.contains("econnrefused") {
		"Connection refused".to_owned()
	} else if lower.contains("timeout")
		|| lower.contains("timed out")
		|| lower.contains("etimedout")
	{
		"Timed out".to_owned()
	} else if lower.contains("enotfound") || lower.contains("getaddrinfo") {
		"DNS lookup failed".to_owned()
	} else if lower.contains("certificate") {
		"Certificate error".to_owned()
	} else if lower.contains("no heartbeat") {
		"No ping in time".to_owned()
	} else {
		let line: String = msg
			.lines()
			.next()
			.unwrap_or_default()
			.chars()
			.take(60)
			.collect();
		if line.trim().is_empty() {
			"Down".to_owned()
		} else {
			line.trim().to_owned()
		}
	}
}

struct Beat {
	at: i64,
	ms: u32,
	msg: String,
}

#[derive(Default)]
struct History {
	checks: Vec<Check>,
	incidents: Vec<(i64, Option<i64>, String)>,
	open: Option<(i64, String)>,
	held: Vec<Beat>,
	first: Option<i64>,
}

impl History {
	fn add(&mut self, beat: &Beat, state: Health) {
		self.checks.push(Check {
			at: beat.at,
			ms: beat.ms,
			state,
			code: if state == Health::Down { OTHER } else { 2 },
		});
	}

	// a pending beat counts as a failure only when a down beat follows
	fn release(&mut self, down: bool, slow: u32) {
		for beat in std::mem::take(&mut self.held) {
			if down {
				self.open.get_or_insert_with(|| (beat.at, cause(&beat.msg)));
				self.add(&beat, Health::Down);
			} else {
				let state = if beat.ms > slow {
					Health::Slow
				} else {
					Health::Up
				};
				self.add(&beat, state);
			}
		}
	}

	fn beat(&mut self, status: i64, beat: Beat, slow: u32) {
		self.first.get_or_insert(beat.at);
		match status {
			2 => {
				self.held.push(beat);
			}
			0 => {
				self.release(true, slow);
				self.open.get_or_insert_with(|| (beat.at, cause(&beat.msg)));
				self.add(&beat, Health::Down);
			}
			1 => {
				self.release(false, slow);
				if let Some((started, why)) = self.open.take() {
					self.incidents.push((started, Some(beat.at), why));
				}
				let state = if beat.ms > slow {
					Health::Slow
				} else {
					Health::Up
				};
				self.add(&beat, state);
			}
			_ => {
				self.release(false, slow);
				self.add(&beat, Health::Maintenance);
			}
		}
	}
}

fn beats(kuma: &Connection, id: i64, slow: u32) -> anyhow::Result<History> {
	let mut past = History::default();
	let mut stmt = kuma.prepare(
		"select time, status, ping, msg from heartbeat where monitor_id = ?1 order by time",
	)?;
	let mut rows = stmt.query([id])?;
	while let Some(row) = rows.next()? {
		let time: String = row.get(0)?;
		let Some(at) =
			unix(&time, &TimeZone::UTC).filter(|at| (0..=i64::from(u32::MAX)).contains(at))
		else {
			continue;
		};
		let beat = Beat {
			at,
			ms: row
				.get::<_, Option<f64>>(2)?
				.unwrap_or(0.0)
				.clamp(0.0, 65_535.0) as u32,
			msg: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
		};
		past.beat(row.get(1)?, beat, slow);
	}
	past.release(false, slow);
	Ok(past)
}

// kuma 2 keeps hourly and daily totals when it trims old beats
fn totals(kuma: &Connection, id: i64, checks: &[Check]) -> Vec<Hour> {
	let seen: BTreeSet<i64> = checks.iter().map(|check| check.at / DAY).collect();
	let mut hours = BTreeMap::new();
	let mut days = BTreeSet::new();
	for table in ["stat_hourly", "stat_daily"] {
		let Ok(mut stmt) = kuma.prepare(&format!(
			"select timestamp, ping, ping_min, ping_max, up, down from {table} where monitor_id = ?1 order by timestamp"
		)) else {
			continue;
		};
		let Ok(rows) = stmt.query_map([id], |row| {
			Ok((
				row.get::<_, i64>(0)?,
				row.get::<_, Option<f64>>(1)?.unwrap_or(0.0),
				row.get::<_, Option<f64>>(2)?.unwrap_or(0.0),
				row.get::<_, Option<f64>>(3)?.unwrap_or(0.0),
				row.get::<_, Option<i64>>(4)?.unwrap_or(0),
				row.get::<_, Option<i64>>(5)?.unwrap_or(0),
			))
		}) else {
			continue;
		};
		for (at, avg, min, max, up, down) in rows.flatten() {
			let day = at / DAY;
			if !(0..=i64::from(u32::MAX)).contains(&at) || seen.contains(&day) {
				continue;
			}
			if table == "stat_hourly" {
				days.insert(day);
			} else if days.contains(&day) {
				continue;
			}
			let hour = (at / 3600) as u32;
			let count = |n: i64| n.clamp(0, i64::from(u16::MAX)) as u16;
			hours.insert(
				hour,
				Hour::rolled(
					hour,
					count(up),
					count(down),
					avg as u32,
					min as u32,
					max as u32,
				),
			);
		}
	}
	hours.into_values().collect()
}

fn targets(config: &Value) -> Result<Vec<String>, String> {
	let text = |key: &str| config[key].as_str().map(str::trim).unwrap_or_default();
	let kind = text("type");
	let url = |raw: String, start: &str| {
		let ok = raw.starts_with(start)
			&& url::Url::parse(&raw).is_ok_and(|url| outbound::allowed(&url));
		ok.then_some(raw)
			.ok_or_else(|| format!("{kind} needs a public https address"))
	};
	match kind {
		"smtp" => {
			let to: Vec<String> = text("smtpTo")
				.split(',')
				.map(str::trim)
				.filter(|to| to.contains('@'))
				.map(str::to_owned)
				.collect();
			if to.is_empty() {
				Err("it has no address to send to".to_owned())
			} else {
				Ok(to)
			}
		}
		"webhook" => url(text("webhookURL").to_owned(), "https://").map(|to| vec![to]),
		"discord" => url(
			text("discordWebhookUrl").to_owned(),
			"https://discord.com/api/webhooks/",
		)
		.map(|to| vec![to]),
		"slack" => url(
			text("slackwebhookURL").to_owned(),
			"https://hooks.slack.com/",
		)
		.map(|to| vec![to]),
		"telegram" => {
			let (bot, chat) = (text("telegramBotToken"), text("telegramChatID"));
			let made = url::Url::parse_with_params(
				&format!("https://api.telegram.org/bot{bot}/sendMessage"),
				[("chat_id", chat)],
			);
			match made {
				Ok(made) if !bot.is_empty() && !chat.is_empty() => Ok(vec![made.to_string()]),
				_ => Err("telegram needs a bot token and a chat".to_owned()),
			}
		}
		"ntfy" => {
			let server = match text("ntfyserverurl") {
				"" => "https://ntfy.sh",
				server => server.trim_end_matches('/'),
			};
			let topic = text("ntfytopic");
			let secured = !matches!(text("ntfyAuthenticationMethod"), "" | "none");
			if secured || topic.is_empty() || topic.contains('/') {
				return Err("ntfy with sign in is not supported".to_owned());
			}
			let to = url(format!("{server}/{topic}"), "https://")?;
			if url::Url::parse(&to)
				.is_ok_and(|to| to.host_str().is_some_and(|host| host.starts_with("ntfy.")))
			{
				Ok(vec![to])
			} else {
				Err("ntfy needs a server named ntfy.something".to_owned())
			}
		}
		other => Err(format!("{other} is not supported")),
	}
}

// read only, and a dry run writes nothing
pub fn kuma(
	app: &App,
	log: &Log,
	path: &Path,
	team: i64,
	user: i64,
	dry: bool,
) -> anyhow::Result<Summary> {
	let source = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
	let not_kuma = || anyhow!("This isn't an Uptime Kuma database.");
	let tables: HashSet<String> = source
		.prepare("select name from sqlite_master where type = 'table'")
		.and_then(|mut stmt| stmt.query_map([], |row| row.get(0))?.collect())
		.map_err(|_| not_kuma())?;
	if !tables.contains("monitor") || !tables.contains("heartbeat") {
		return Err(not_kuma());
	}
	let admin = app.is_admin(user);
	let mut conn = app.conn();
	let tx = conn.transaction()?;
	let mut made = Vec::new();
	let mut out = Summary::default();
	match run(
		&source,
		&tables,
		&tx,
		log,
		(team, user, admin, dry),
		&mut made,
		&mut out,
	) {
		Ok(()) if !dry => tx.commit()?,
		Ok(()) => drop(tx),
		Err(err) => {
			drop(tx);
			made.into_iter().for_each(|id| log.remove(id));
			return Err(err);
		}
	}
	Ok(out)
}

fn run(
	source: &Connection,
	tables: &HashSet<String>,
	tx: &Transaction,
	log: &Log,
	(team, user, admin, dry): (i64, i64, bool, bool),
	made: &mut Vec<i64>,
	out: &mut Summary,
) -> anyhow::Result<()> {
	let at = now();
	let alerts = alerts(source, tables, tx, team, out)?;
	let mut ids: HashMap<i64, i64> = HashMap::new();
	for m in sources(source).context("read monitors")? {
		let name = if m.name.trim().is_empty() {
			format!("Monitor {}", m.id)
		} else {
			m.name.trim().chars().take(80).collect()
		};
		let plan = match plan(&m, admin) {
			Ok(plan) => plan,
			Err(reason) => {
				out.skip(&name, reason);
				continue;
			}
		};
		let found: Option<(i64, i64)> = match &plan.token {
			Some(token) => tx.query_row(
				"select id, team from monitors where token = ?1",
				[token],
				|row| Ok((row.get(0)?, row.get(1)?)),
			),
			None => tx.query_row(
				"select id, team from monitors where team = ?1 and kind = ?2 and target = ?3 and name = ?4",
				params![team, plan.kind.as_str(), plan.target, name],
				|row| Ok((row.get(0)?, row.get(1)?)),
			),
		}
		.ok();
		if let Some((id, owner)) = found {
			if owner == team {
				ids.insert(m.id, id);
				out.existing += 1;
			} else {
				out.skip(&name, "its push token belongs to another team");
			}
			continue;
		}
		let muted = !alerts
			.links
			.get(&m.id)
			.is_some_and(|linked| linked.iter().any(|id| alerts.mapped.contains(id)));
		let created = unix(&m.created, &TimeZone::UTC).unwrap_or(at).min(at);
		let past = beats(source, m.id, plan.config.slow).context("read heartbeats")?;
		let created = created.min(past.first.unwrap_or(created));
		tx.execute(
			"insert into monitors (team, name, kind, target, config, interval, paused, muted, token, created_by, created_at)
			values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
			params![
				team,
				name,
				plan.kind.as_str(),
				plan.target,
				serde_json::to_string(&plan.config)?,
				plan.interval,
				!m.active,
				muted,
				plan.token,
				user,
				created
			],
		)?;
		let id = tx.last_insert_rowid();
		made.push(id);
		ids.insert(m.id, id);
		out.monitors += 1;
		if !m.active {
			tx.execute(
				"insert into pauses (monitor, from_at) values (?1, ?2)",
				params![id, at],
			)?;
		}
		for note in &plan.notes {
			out.note(&name, note);
		}
		let rolled = totals(source, m.id, &past.checks);
		if !dry {
			log.backfill(id, &past.checks, rolled, at)?;
		}
		out.checks += past.checks.len() as u64;
		let last = past.checks.last().map(|check| check.at);
		let mut insert = tx.prepare_cached(
			"insert into incidents (monitor, started_at, ended_at, cause) values (?1, ?2, ?3, ?4)",
		)?;
		let open = past
			.open
			.map(|(started, why)| (started, (!m.active).then_some(last.unwrap_or(started)), why));
		for (started, ended, why) in past.incidents.into_iter().chain(open) {
			insert.execute(params![id, started, ended, why])?;
			out.incidents += 1;
		}
	}
	maintenance(source, tables, tx, team, &ids, out)?;
	pages(source, tables, tx, team, &ids, at, out)?;
	Ok(())
}

struct Alerts {
	mapped: HashSet<i64>,
	links: HashMap<i64, Vec<i64>>,
}

fn alerts(
	source: &Connection,
	tables: &HashSet<String>,
	tx: &Transaction,
	team: i64,
	out: &mut Summary,
) -> anyhow::Result<Alerts> {
	let mut alerts = Alerts {
		mapped: HashSet::new(),
		links: HashMap::new(),
	};
	if !tables.contains("notification") {
		return Ok(alerts);
	}
	let have = columns(source, "notification")?;
	let list = select(&have, &["id", "name", "config", "active"]);
	let mut stmt = source.prepare(&format!("select {list} from notification order by id"))?;
	let rows = stmt.query_map([], |row| {
		Ok((
			row.get::<_, i64>(0)?,
			row.get::<_, Option<String>>(1)?.unwrap_or_default(),
			row.get::<_, Option<String>>(2)?.unwrap_or_default(),
			row.get::<_, Option<i64>>(3)?.unwrap_or(1) != 0,
		))
	})?;
	for row in rows {
		let (id, name, config, active) = row?;
		let name = if name.is_empty() {
			"Notification"
		} else {
			name.as_str()
		};
		let config: Value = serde_json::from_str(&config).unwrap_or_default();
		if !active {
			out.skip(name, "it is turned off");
			continue;
		}
		match targets(&config) {
			Ok(list) => {
				alerts.mapped.insert(id);
				for to in list {
					let added = tx.execute(
						"insert into alerts (team, target, created_at)
						select ?1, ?2, ?3 where not exists (select 1 from alerts where team = ?1 and target = ?2)",
						params![team, to, now()],
					)?;
					out.alerts += added as u32;
				}
			}
			Err(reason) => out.skip(name, reason),
		}
	}
	if tables.contains("monitor_notification") {
		let mut stmt =
			source.prepare("select monitor_id, notification_id from monitor_notification")?;
		for row in stmt.query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)))? {
			let (monitor, notification) = row?;
			alerts.links.entry(monitor).or_default().push(notification);
		}
	}
	Ok(alerts)
}

fn maintenance(
	source: &Connection,
	tables: &HashSet<String>,
	tx: &Transaction,
	team: i64,
	ids: &HashMap<i64, i64>,
	out: &mut Summary,
) -> anyhow::Result<()> {
	if !tables.contains("maintenance") {
		return Ok(());
	}
	let have = columns(source, "maintenance")?;
	let list = select(
		&have,
		&[
			"id",
			"title",
			"strategy",
			"active",
			"start_date",
			"end_date",
			"timezone",
		],
	);
	let mut stmt = source.prepare(&format!("select {list} from maintenance order by id"))?;
	let rows = stmt.query_map([], |row| {
		let text = |at| {
			row.get::<_, Option<String>>(at)
				.map(Option::unwrap_or_default)
		};
		Ok((
			row.get::<_, i64>(0)?,
			text(1)?,
			text(2)?,
			row.get::<_, Option<i64>>(3)?.unwrap_or(1) != 0,
			text(4)?,
			text(5)?,
			text(6)?,
		))
	})?;
	let mut linked = if tables.contains("monitor_maintenance") {
		Some(
			source
				.prepare("select monitor_id from monitor_maintenance where maintenance_id = ?1")?,
		)
	} else {
		None
	};
	for row in rows {
		let (id, title, strategy, active, start, end, zone) = row?;
		let name = if title.is_empty() {
			"Maintenance"
		} else {
			title.as_str()
		};
		if !matches!(strategy.as_str(), "single" | "manual") {
			out.skip(name, "recurring maintenance is not supported");
			continue;
		}
		let zone = TimeZone::get(&zone).unwrap_or(TimeZone::UTC);
		let (Some(starts), Some(ends)) = (unix(&start, &zone), unix(&end, &zone)) else {
			out.skip(name, "it has no start and end date");
			continue;
		};
		if !active || ends <= starts {
			out.skip(name, "it is turned off");
			continue;
		}
		let monitors: Vec<i64> = match linked.as_mut() {
			Some(stmt) => stmt
				.query_map([id], |row| row.get::<_, i64>(0))?
				.filter_map(|monitor| ids.get(&monitor.ok()?).copied())
				.collect(),
			None => Vec::new(),
		};
		if monitors.is_empty() {
			out.skip(name, "none of its monitors were imported");
			continue;
		}
		let note: String = title.trim().chars().take(500).collect();
		let known: bool = tx.query_row(
			"select exists (select 1 from maintenance where team = ?1 and starts_at = ?2 and ends_at = ?3 and note = ?4)",
			params![team, starts, ends, note],
			|row| row.get(0),
		)?;
		if known {
			continue;
		}
		tx.execute(
			"insert into maintenance (team, starts_at, ends_at, note) values (?1, ?2, ?3, ?4)",
			params![team, starts, ends, note],
		)?;
		let window = tx.last_insert_rowid();
		for monitor in monitors {
			tx.execute(
				"insert or ignore into maintenance_monitors (maintenance, monitor) values (?1, ?2)",
				params![window, monitor],
			)?;
		}
		out.maintenance += 1;
	}
	Ok(())
}

fn pages(
	source: &Connection,
	tables: &HashSet<String>,
	tx: &Transaction,
	team: i64,
	ids: &HashMap<i64, i64>,
	at: i64,
	out: &mut Summary,
) -> anyhow::Result<()> {
	if !["status_page", "group", "monitor_group"]
		.iter()
		.all(|table| tables.contains(*table))
	{
		return Ok(());
	}
	let mut pages = source.prepare("select id, slug, title from status_page order by id")?;
	let mut groups =
		source.prepare("select id from \"group\" where status_page_id = ?1 order by weight, id")?;
	let mut members = source
		.prepare("select monitor_id from monitor_group where group_id = ?1 order by weight, id")?;
	let rows = pages.query_map([], |row| {
		Ok((
			row.get::<_, i64>(0)?,
			row.get::<_, String>(1)?,
			row.get::<_, Option<String>>(2)?.unwrap_or_default(),
		))
	})?;
	for row in rows {
		let (id, slug, title) = row?;
		let slug = slug.to_lowercase();
		let name: String = if title.trim().is_empty() {
			slug.clone()
		} else {
			title.trim().chars().take(80).collect()
		};
		let fine = (3..=40).contains(&slug.len())
			&& slug
				.bytes()
				.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-');
		if !fine {
			out.skip(&name, "its address is not allowed");
			continue;
		}
		let taken: Option<(i64, String)> = tx
			.query_row(
				"select team, name from status_pages where slug = ?1",
				[&slug],
				|row| Ok((row.get(0)?, row.get(1)?)),
			)
			.ok();
		if let Some((owner, existing)) = taken {
			if owner != team || existing != name {
				out.skip(&name, format!("the address {slug} is taken"));
			}
			continue;
		}
		let mut order: Vec<i64> = Vec::new();
		for group in groups.query_map([id], |row| row.get::<_, i64>(0))? {
			for monitor in members.query_map([group?], |row| row.get::<_, i64>(0))? {
				if let Some(ours) = ids.get(&monitor?)
					&& !order.contains(ours)
				{
					order.push(*ours);
				}
			}
		}
		order.truncate(50);
		if order.is_empty() {
			out.skip(&name, "none of its monitors were imported");
			continue;
		}
		tx.execute(
			"insert into status_pages (team, slug, name, monitors, created_at) values (?1, ?2, ?3, ?4, ?5)",
			params![team, slug, name, serde_json::to_string(&order)?, at],
		)?;
		out.pages += 1;
	}
	Ok(())
}
