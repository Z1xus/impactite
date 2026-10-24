use std::collections::{BTreeMap, HashMap};
use std::convert::Infallible;
use std::net::IpAddr;
use std::sync::{Arc, LazyLock};
use std::time::Duration;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use futures_util::StreamExt;
use jiff::civil::Date;
use jiff::tz::TimeZone;
use jiff::{Timestamp, ToSpan};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::oneshot;
use url::Url;

use crate::app::{now, token};
use crate::auth::accounts::Viewer;
use crate::limit::Limit;
use crate::notify::{self, Note};
use crate::outbound;
use crate::uptime::calc::{Spans, round, strip, uptime};
use crate::uptime::log::percentile;
use crate::uptime::run::Msg;
use crate::uptime::store::Scope;
use crate::uptime::visits_missed;
use crate::uptime::{Config, Digest, Kind, Live, Monitor, State as Health};
use crate::web::{Error, Reply, Web, blocking};

pub(super) const DAY: i64 = 86_400;
// streams end now and then, so rights are checked again on reconnect
pub(super) const STREAM_LIFE: Duration = Duration::from_secs(600);
pub(super) const INTERVALS: [i64; 8] = [30, 60, 120, 300, 600, 900, 1800, 3600];
static CHECKS: LazyLock<Limit> = LazyLock::new(|| Limit::new(30, 1));
static TESTS: LazyLock<Limit> = LazyLock::new(|| Limit::new(5, 1));
const ALERTS: usize = 20;

pub fn routes() -> Router<Arc<Web>> {
	Router::new()
		.route("/api/app/uptime", get(list).post(create))
		.route("/api/app/uptime/live", get(live))
		.route("/api/app/sites/{domain}/incidents", get(site_incidents))
		.route(
			"/api/app/uptime/maintenance",
			get(maintenance).post(add_maintenance),
		)
		.route(
			"/api/app/uptime/maintenance/{id}",
			delete(remove_maintenance),
		)
		.route("/api/app/uptime/probe", post(probe))
		.route("/api/app/uptime/alerts", get(alerts).post(add_alert))
		.route("/api/app/uptime/alerts/{id}", delete(remove_alert))
		.route("/api/app/uptime/alerts/{id}/test", post(test_alert))
		.route("/api/app/uptime/incidents/{id}/notes", post(add_note))
		.route(
			"/api/app/uptime/incidents/{id}/notes/{note}",
			delete(remove_note),
		)
		.route(
			"/api/app/uptime/{id}",
			get(show).patch(update).delete(remove),
		)
		.route("/api/app/uptime/{id}/pause", post(pause))
		.route("/api/app/uptime/{id}/resume", post(resume))
		.route("/api/app/uptime/{id}/check", post(check))
}

#[derive(Deserialize)]
pub(super) struct Scoped {
	pub(super) team: i64,
}

#[derive(Deserialize)]
struct Window {
	range: Option<String>,
}

#[derive(Deserialize, Default)]
pub struct Fields {
	pub team: Option<i64>,
	pub target: Option<String>,
	pub name: Option<String>,
	pub kind: Option<String>,
	pub interval: Option<i64>,
	pub muted: Option<bool>,
	pub method: Option<String>,
	pub status: Option<String>,
	pub contains: Option<String>,
	pub lacks: Option<String>,
	pub headers: Option<BTreeMap<String, String>>,
	pub timeout: Option<u64>,
	pub slow: Option<u32>,
	pub grace: Option<i64>,
}

pub(super) fn member(web: &Web, user: i64, team: i64) -> Reply<String> {
	web.app
		.role(user, team)?
		.ok_or_else(|| Error::not_found("This team doesn't exist."))
}

pub(super) fn editor(web: &Web, user: i64, team: i64) -> Reply<()> {
	if member(web, user, team)? == "viewer" {
		return Err(Error::forbidden("You can't change uptime in this team."));
	}
	Ok(())
}

pub(super) fn owned(web: &Web, user: i64, id: i64) -> Reply<(Monitor, String)> {
	let monitor = web
		.app
		.monitor(id)?
		.ok_or_else(|| Error::not_found("This monitor doesn't exist."))?;
	let role = member(web, user, monitor.team)
		.map_err(|_| Error::not_found("This monitor doesn't exist."))?;
	Ok((monitor, role))
}

pub(super) fn changeable(web: &Web, user: i64, id: i64) -> Reply<Monitor> {
	let (monitor, role) = owned(web, user, id)?;
	if role == "viewer" {
		return Err(Error::forbidden("You can't change uptime in this team."));
	}
	Ok(monitor)
}

pub(super) fn base(web: &Web) -> &str {
	web.url.trim_end_matches('/')
}

pub(super) fn live_of(monitor: &Monitor, live: Option<Live>) -> Live {
	live.unwrap_or(Live {
		state: if monitor.paused {
			Health::Paused
		} else {
			Health::Pending
		},
		since: monitor.created_at,
		latency: None,
		at: monitor.created_at,
		cert: None,
	})
}

fn event_json(id: i64, live: &Live) -> Value {
	json!({
		"id": id,
		"state": live.state,
		"since": live.since,
		"latency": live.latency,
		"at": live.at,
		"cert_expires": live.cert,
	})
}

fn target(web: &Web, monitor: &Monitor, editor: bool) -> String {
	match (monitor.kind, &monitor.token) {
		(Kind::Heartbeat, Some(token)) if editor => format!("{}/ping/{token}", base(web)),
		(Kind::Heartbeat, _) => String::new(),
		_ => monitor.target.clone(),
	}
}

pub(super) fn summary(
	web: &Web,
	monitor: &Monitor,
	site: Option<&str>,
	spans: &Spans,
	digest: &Digest,
	editor: bool,
	at: i64,
) -> Value {
	let live = live_of(monitor, web.uptime.live(monitor.id));
	let watched = digest.watched_from(monitor.created_at);
	let fraction = |from: i64| uptime(spans, watched, from, at).map(round);
	let mut item = json!({
		"id": monitor.id,
		"name": monitor.name,
		"kind": monitor.kind,
		"target": target(web, monitor, editor),
		"site": site,
		"interval": monitor.interval,
		"paused": monitor.paused,
		"muted": monitor.muted,
		"state": live.state,
		"since": live.since,
		"latency": live.latency,
		"at": live.at,
		"uptime_24h": fraction(at - DAY),
		"uptime_30d": fraction(at - 30 * DAY),
		"p50_24h": digest.p50.filter(|_| monitor.kind != Kind::Heartbeat),
		"cert_expires": live.cert,
	});
	// set after the macro, which would copy the cells a second time
	item["strip"] = Value::Array(strip(spans, watched, at, &digest.slow_days));
	item
}

async fn list(State(web): State<Arc<Web>>, viewer: Viewer, Query(query): Query<Scoped>) -> Reply {
	let role = member(&web, viewer.user, query.team)?;
	let at = now();
	let items = overview(&web, query.team, role != "viewer", at).await?;
	Ok(Json(
		json!({ "monitors": items, "role": role, "today": at / DAY * DAY }),
	))
}

pub(super) async fn overview(
	web: &Arc<Web>,
	team: i64,
	editable: bool,
	at: i64,
) -> Reply<Vec<Value>> {
	let monitors = web.app.team_monitors(team)?;
	let mut spans = web.app.spans(&Scope::Team(team), at - 90 * DAY)?;
	let ids: Vec<i64> = monitors.iter().map(|(monitor, _)| monitor.id).collect();
	let reader = web.clone();
	let digests: HashMap<i64, Digest> = blocking(move || {
		Ok(ids
			.into_iter()
			.map(|id| (id, reader.uptime.digest(id, at)))
			.collect())
	})
	.await?;
	Ok(monitors
		.iter()
		.map(|(monitor, site)| {
			let spans = spans.remove(&monitor.id).unwrap_or_default();
			summary(
				web,
				monitor,
				site.as_deref(),
				&spans,
				&digests[&monitor.id],
				editable,
				at,
			)
		})
		.collect())
}

// host:port is tcp, a bare host is its https page
pub(super) fn parse(raw: &str) -> Option<(Kind, String, String)> {
	let raw = raw.trim();
	if raw.starts_with("http://") || raw.starts_with("https://") {
		let url = Url::parse(raw).ok()?;
		let host = url.host_str()?.to_owned();
		return Some((Kind::Http, url.to_string(), host));
	}
	if raw.contains("://") || raw.contains('/') {
		return None;
	}
	if let Ok(url) = Url::parse(&format!("tcp://{raw}"))
		&& let (Some(host), Some(port)) = (url.host_str(), url.port())
		&& port > 0
	{
		return Some((Kind::Tcp, format!("{host}:{port}"), host.to_owned()));
	}
	let url = Url::parse(&format!("https://{raw}")).ok()?;
	let host = url.host_str()?.to_owned();
	Some((Kind::Http, url.to_string(), host))
}

pub(super) fn private(host: &str) -> bool {
	match host.trim_matches(['[', ']']).parse::<IpAddr>() {
		Ok(ip) => !outbound::public(ip),
		Err(_) => host == "localhost" || host.ends_with(".localhost"),
	}
}

fn fill(web: &Web, user: i64, monitor: &mut Monitor, fields: Fields) -> Reply<()> {
	let mut host = None;
	if let Some(raw) = fields
		.target
		.as_deref()
		.filter(|_| monitor.kind != Kind::Heartbeat)
	{
		if raw.len() > 2048 {
			return Err(Error::bad("This address is too long."));
		}
		let (kind, target, name) = parse(raw)
			.ok_or_else(|| Error::bad("Enter a URL like https://example.com or a host:port."))?;
		if monitor.id != 0 && kind != monitor.kind {
			return Err(Error::bad(
				"A monitor keeps its kind. Add a new one instead.",
			));
		}
		if private(&name) && !web.app.is_admin(user) {
			return Err(Error::forbidden(
				"Local addresses can only be monitored by an instance admin.",
			));
		}
		if target != monitor.target {
			monitor.created_by = Some(user);
		}
		monitor.kind = kind;
		monitor.target = target;
		host = Some(name);
	}
	if let Some(name) = fields.name {
		monitor.name = name.trim().to_owned();
	}
	if monitor.name.is_empty() {
		monitor.name = host.unwrap_or_else(|| "Heartbeat".to_owned());
	}
	if monitor.name.chars().count() > 80 {
		return Err(Error::bad("This name is too long."));
	}
	if let Some(interval) = fields.interval {
		monitor.interval = interval;
	}
	let allowed = if monitor.kind == Kind::Heartbeat {
		(60..=7 * DAY).contains(&monitor.interval)
	} else {
		INTERVALS.contains(&monitor.interval)
	};
	if !allowed {
		return Err(Error::bad("Choose an interval from the list."));
	}
	if let Some(muted) = fields.muted {
		monitor.muted = muted;
	}
	let config = &mut monitor.config;
	if let Some(method) = fields.method {
		config.method = method.to_uppercase();
	}
	if let Some(status) = fields.status {
		config.status = status;
	}
	if let Some(contains) = fields.contains {
		config.contains = contains;
	}
	if let Some(lacks) = fields.lacks {
		config.lacks = lacks;
	}
	if let Some(headers) = fields.headers {
		config.headers = headers;
	}
	if let Some(timeout) = fields.timeout {
		config.timeout = timeout;
	}
	if let Some(slow) = fields.slow {
		config.slow = slow;
	}
	if let Some(grace) = fields.grace {
		config.grace = grace;
	}
	config.check().map_err(Error::bad)?;
	Ok(())
}

async fn create(State(web): State<Arc<Web>>, viewer: Viewer, Json(fields): Json<Fields>) -> Reply {
	let created = add(&web, viewer.user, fields)?;
	Ok(Json(json!({
		"id": created.id,
		"name": created.name,
		"kind": created.kind,
		"target": target(&web, &created, true),
	})))
}

pub(super) fn add(web: &Web, user: i64, fields: Fields) -> Reply<Monitor> {
	let monitor = draft(web, user, fields)?;
	let id = web.app.add_monitor(&monitor)?;
	web.uptime.sync();
	web.app.monitor(id)?.ok_or_else(Error::broken)
}

fn draft(web: &Web, user: i64, fields: Fields) -> Reply<Monitor> {
	let team = fields.team.ok_or_else(|| Error::bad("Choose a team."))?;
	editor(web, user, team)?;
	let heartbeat = fields.kind.as_deref() == Some("heartbeat");
	let mut monitor = Monitor {
		id: 0,
		team,
		site: None,
		name: String::new(),
		kind: if heartbeat {
			Kind::Heartbeat
		} else {
			Kind::Http
		},
		target: String::new(),
		config: Config::default(),
		interval: 60,
		paused: false,
		muted: false,
		token: None,
		created_by: Some(user),
		created_at: 0,
	};
	if heartbeat {
		monitor.token = Some(token()?[..32].to_owned());
	} else if fields
		.target
		.as_deref()
		.is_none_or(|target| target.trim().is_empty())
	{
		return Err(Error::bad("Enter a URL or a host:port."));
	}
	fill(web, user, &mut monitor, fields)?;
	Ok(monitor)
}

async fn update(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path(id): Path<i64>,
	Json(fields): Json<Fields>,
) -> Reply<StatusCode> {
	edit(&web, viewer.user, id, fields)?;
	Ok(StatusCode::NO_CONTENT)
}

pub(super) fn edit(web: &Web, user: i64, id: i64, fields: Fields) -> Reply<()> {
	let mut monitor = changeable(web, user, id)?;
	fill(web, user, &mut monitor, fields)?;
	web.app.save_monitor(&monitor)?;
	web.uptime.sync();
	Ok(())
}

async fn remove(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path(id): Path<i64>,
) -> Reply<StatusCode> {
	discard(&web, viewer.user, id)?;
	Ok(StatusCode::NO_CONTENT)
}

pub(super) fn discard(web: &Web, user: i64, id: i64) -> Reply<()> {
	changeable(web, user, id)?;
	web.app.remove_monitor(id)?;
	web.uptime.sync();
	Ok(())
}

async fn pause(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path(id): Path<i64>,
) -> Reply<StatusCode> {
	hold(&web, viewer.user, id, true)?;
	Ok(StatusCode::NO_CONTENT)
}

async fn resume(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path(id): Path<i64>,
) -> Reply<StatusCode> {
	hold(&web, viewer.user, id, false)?;
	Ok(StatusCode::NO_CONTENT)
}

pub(super) fn hold(web: &Web, user: i64, id: i64, paused: bool) -> Reply<()> {
	changeable(web, user, id)?;
	web.app.set_paused(id, paused)?;
	web.uptime.sync();
	Ok(())
}

async fn check(State(web): State<Arc<Web>>, viewer: Viewer, Path(id): Path<i64>) -> Reply {
	let monitor = changeable(&web, viewer.user, id)?;
	if monitor.kind == Kind::Heartbeat {
		return Err(Error::bad(
			"Heartbeat monitors can't be checked manually. They wait for your service to check in.",
		));
	}
	let who = viewer.user.to_string();
	if !CHECKS.open(&who) {
		return Err(Error::too_many());
	}
	CHECKS.strike(&who);
	let (reply, result) = oneshot::channel();
	web.uptime.send(Msg::Check(id, reply));
	let wait = Duration::from_secs(monitor.config.timeout + 15);
	let out = tokio::time::timeout(wait, result)
		.await
		.map_err(|_| Error::new(StatusCode::GATEWAY_TIMEOUT, "The check took too long."))?
		.map_err(|_| Error::not_found("This monitor isn't ready yet. Try again in a moment."))?;
	Ok(Json(outcome(&monitor, &out)))
}

async fn probe(State(web): State<Arc<Web>>, viewer: Viewer, Json(fields): Json<Fields>) -> Reply {
	let monitor = draft(&web, viewer.user, fields)?;
	if monitor.kind == Kind::Heartbeat {
		return Err(Error::bad(
			"Heartbeat monitors can't be checked manually. They wait for your service to check in.",
		));
	}
	let who = viewer.user.to_string();
	if !CHECKS.open(&who) {
		return Err(Error::too_many());
	}
	CHECKS.strike(&who);
	let local = web.app.is_admin(viewer.user);
	let out = super::probe::run(&web.uptime.clients, &monitor, local).await;
	Ok(Json(outcome(&monitor, &out)))
}

fn outcome(monitor: &Monitor, out: &super::probe::Outcome) -> Value {
	let state = if !out.ok {
		Health::Down
	} else if out.ms > monitor.config.slow {
		Health::Slow
	} else {
		Health::Up
	};
	json!({
		"ok": out.ok,
		"state": state,
		"latency": (out.ms > 0).then_some(out.ms),
		"cause": (!out.ok).then_some(&out.cause),
		"cert_expires": out.cert,
	})
}

const FIVE_MIN: i64 = 300;

fn exact(sorted: &[u32], q: f64) -> Option<u32> {
	let rank = (q * sorted.len() as f64).ceil() as usize;
	sorted.get(rank.max(1) - 1).copied()
}

async fn show(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path(id): Path<i64>,
	Query(window): Query<Window>,
) -> Reply {
	let (monitor, role) = owned(&web, viewer.user, id)?;
	let editable = role != "viewer";
	let range = window.range.as_deref().unwrap_or("24h");
	let seconds = match range {
		"24h" => DAY,
		"7d" => 7 * DAY,
		"30d" => 30 * DAY,
		"90d" => 90 * DAY,
		_ => return Err(Error::bad("Use 24h, 7d, 30d or 90d.")),
	};
	let at = now();
	let from = at - seconds;
	let spans = web.app.spans(&Scope::Monitor(id), at - 365 * DAY)?;
	let spans = spans.get(&id);
	let empty = Spans::default();
	let spans = spans.unwrap_or(&empty);
	let reader = web.clone();
	let daily = range == "24h";
	let (series, p50, p95, digest) = blocking(move || {
		let log = &reader.uptime.log;
		let digest = reader.uptime.digest(id, at);
		if daily {
			let checks = log.checks(id, from, at)?;
			let mut times: Vec<u32> = checks.iter().map(|check| check.ms).filter(|ms| *ms > 0).collect();
			times.sort_unstable();
			let series: Vec<Value> = checks
				.chunk_by(|a, b| a.at / FIVE_MIN == b.at / FIVE_MIN)
				.map(|group| {
					let mut times: Vec<u32> = group.iter().map(|check| check.ms).filter(|ms| *ms > 0).collect();
					times.sort_unstable();
					let worst = group.iter().map(|check| check.state as u8).max().unwrap_or_default();
					json!({ "t": group[0].at / FIVE_MIN * FIVE_MIN, "ms": exact(&times, 0.5), "state": Health::from_u8(worst) })
				})
				.collect();
			return Ok((series, exact(&times, 0.5), exact(&times, 0.95), digest));
		}
		let hours = log.hours(id, from, at)?;
		let series: Vec<Value> = hours
			.iter()
			.map(|hour| json!({ "t": i64::from(hour.hour) * 3600, "ms": percentile(std::slice::from_ref(hour), 0.5), "state": hour.worst() }))
			.collect();
		Ok((series, percentile(&hours, 0.5), percentile(&hours, 0.95), digest))
	})
	.await?;
	let incidents = web.app.monitor_incidents(id, from)?;
	let downtime: i64 = incidents
		.iter()
		.map(|(_, started, ended, ..)| (ended.unwrap_or(at).min(at) - started.max(&from)).max(0))
		.sum();
	let notes = web.app.notes(
		&incidents
			.iter()
			.map(|incident| incident.0)
			.collect::<Vec<_>>(),
	)?;
	let missed = visits_missed(
		&web,
		monitor.site,
		incidents
			.iter()
			.map(|incident| (incident.0, incident.1, incident.2))
			.collect(),
	)
	.await;
	let incidents: Vec<Value> = incidents
		.into_iter()
		.zip(missed)
		.map(|((incident, started, ended, cause, detail), missed)| {
			json!({
				"visits_missed": missed,
				"id": incident,
				"started_at": started,
				"ended_at": ended,
				"cause": cause,
				"detail": if editable { detail } else { String::new() },
				"duration": ended.unwrap_or(at) - started,
				"notes": notes.get(&incident).cloned().unwrap_or_default(),
			})
		})
		.collect();
	let site = monitor.site.and_then(|site| web.app.monitor_domain(site));
	let mut item = summary(
		&web,
		&monitor,
		site.as_deref(),
		spans,
		&digest,
		editable,
		at,
	);
	item["config"] = if editable {
		json!(monitor.config)
	} else {
		json!(Config {
			headers: BTreeMap::new(),
			..monitor.config.clone()
		})
	};
	item["created_at"] = json!(monitor.created_at);
	let live = live_of(&monitor, web.uptime.live(id));
	let watched = digest.watched_from(monitor.created_at);
	let fraction = |from: i64| uptime(spans, watched, from, at).map(round);
	let heartbeat = monitor.kind == Kind::Heartbeat;
	Ok(Json(json!({
		"monitor": item,
		"role": role,
		"range": range,
		"kpis": {
			"uptime": fraction(from),
			"downtime": fraction(from).map(|_| downtime),
			"incidents": incidents.len(),
			"p50": p50.filter(|_| !heartbeat),
			"p95": p95.filter(|_| !heartbeat),
			"cert_expires": live.cert,
		},
		"series": series,
		"incidents": incidents,
		"maintenance": web.app.maintenance(&Scope::Monitor(id), from)?,
	})))
}

#[derive(Deserialize)]
struct Between {
	from: Option<String>,
	to: Option<String>,
}

async fn site_incidents(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path(domain): Path<String>,
	Query(between): Query<Between>,
) -> Reply {
	let site = web
		.app
		.site(&domain)?
		.ok_or_else(|| Error::not_found("This site doesn't exist."))?;
	let team = web.app.site_team(site.id)?;
	member(&web, viewer.user, team).map_err(|_| Error::not_found("This site doesn't exist."))?;
	let Some(monitor) = web.app.site_monitor(site.id)? else {
		return Ok(Json(json!({ "monitor": null, "incidents": [] })));
	};
	let tz = TimeZone::get(&site.timezone).unwrap_or(TimeZone::UTC);
	let bound = |date: &Option<String>, days: i64| {
		let day: Date = date.as_deref()?.get(..10)?.parse().ok()?;
		let start = day
			.checked_add(days.days())
			.ok()?
			.to_zoned(tz.clone())
			.ok()?;
		Some(start.timestamp().as_second())
	};
	let from = bound(&between.from, 0).unwrap_or(0);
	let to = bound(&between.to, 1).unwrap_or(i64::MAX);
	let at = now();
	let found = web.app.incidents_between(monitor.id, from, to)?;
	let missed = visits_missed(
		&web,
		monitor.site,
		found.iter().map(|item| (item.0, item.1, item.2)).collect(),
	)
	.await;
	let local = |seconds: i64| {
		Timestamp::from_second(seconds)
			.map(|time| {
				time.to_zoned(tz.clone())
					.strftime("%Y-%m-%d %H:%M:%S")
					.to_string()
			})
			.unwrap_or_default()
	};
	let incidents: Vec<Value> = found
		.into_iter()
		.zip(missed)
		.map(|((id, started, ended, cause, _), missed)| {
			json!({
				"id": id,
				"started_at": started,
				"ended_at": ended,
				"cause": cause,
				"duration": ended.unwrap_or(at) - started,
				"visits_missed": missed,
				"start": local(started),
				"end": ended.map(local),
			})
		})
		.collect();
	let live = live_of(&monitor, web.uptime.live(monitor.id));
	Ok(Json(json!({
		"monitor": { "id": monitor.id, "state": live.state, "since": live.since },
		"incidents": incidents,
	})))
}

async fn live(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Query(query): Query<Scoped>,
) -> Reply<impl IntoResponse> {
	member(&web, viewer.user, query.team)?;
	let team = query.team;
	let receiver = web.uptime.subscribe();
	let snapshot: Vec<Value> = web
		.app
		.team_monitors(team)?
		.iter()
		.map(|(monitor, _)| event_json(monitor.id, &live_of(monitor, web.uptime.live(monitor.id))))
		.collect();
	let first = futures_util::stream::once(async move {
		Ok::<_, Infallible>(
			Event::default()
				.event("snapshot")
				.json_data(snapshot)
				.unwrap_or_default(),
		)
	});
	let rest = futures_util::stream::unfold(receiver, move |mut receiver| async move {
		loop {
			match receiver.recv().await {
				Ok(event) if event.team == team => {
					let data = Event::default()
						.event("check")
						.json_data(event_json(event.id, &event.live))
						.unwrap_or_default();
					return Some((Ok::<_, Infallible>(data), receiver));
				}
				Ok(_) => {}
				// a client that fell behind starts over from a snapshot
				Err(_) => return None,
			}
		}
	});
	Ok(Sse::new(
		first
			.chain(rest)
			.take_until(tokio::time::sleep(STREAM_LIFE)),
	)
	.keep_alive(KeepAlive::default()))
}

#[derive(Deserialize)]
struct NewNote {
	text: String,
	public: Option<bool>,
}

fn incident_team(web: &Web, user: i64, incident: i64) -> Reply<()> {
	let team = web
		.app
		.incident_team(incident)
		.ok_or_else(|| Error::not_found("This incident doesn't exist."))?;
	editor(web, user, team)
}

async fn add_note(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path(id): Path<i64>,
	Json(body): Json<NewNote>,
) -> Reply {
	incident_team(&web, viewer.user, id)?;
	let text = body.text.trim();
	if text.is_empty() || text.chars().count() > 1000 {
		return Err(Error::bad("Enter a short update."));
	}
	Ok(Json(web.app.add_note(
		id,
		text,
		body.public.unwrap_or(true),
	)?))
}

async fn remove_note(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path((id, note)): Path<(i64, i64)>,
) -> Reply<StatusCode> {
	incident_team(&web, viewer.user, id)?;
	web.app.remove_note(id, note)?;
	Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct NewWindow {
	team: i64,
	starts_at: i64,
	ends_at: i64,
	note: Option<String>,
	monitors: Vec<i64>,
}

fn windows(web: &Web, team: i64) -> Reply {
	let list = web.app.maintenance(&Scope::Team(team), now() - 30 * DAY)?;
	Ok(Json(json!({ "maintenance": list })))
}

async fn maintenance(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Query(query): Query<Scoped>,
) -> Reply {
	member(&web, viewer.user, query.team)?;
	windows(&web, query.team)
}

async fn add_maintenance(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Json(body): Json<NewWindow>,
) -> Reply {
	editor(&web, viewer.user, body.team)?;
	let note = body.note.unwrap_or_default();
	let team: Vec<i64> = web
		.app
		.team_monitors(body.team)?
		.iter()
		.map(|(monitor, _)| monitor.id)
		.collect();
	if body.ends_at <= body.starts_at
		|| body.ends_at <= now()
		|| body.monitors.is_empty()
		|| !body.monitors.iter().all(|id| team.contains(id))
		|| note.chars().count() > 500
	{
		return Err(Error::bad(
			"Pick monitors and a window that ends in the future.",
		));
	}
	web.app.add_maintenance(
		body.team,
		body.starts_at,
		body.ends_at,
		note.trim(),
		&body.monitors,
	)?;
	web.uptime.sync();
	windows(&web, body.team)
}

async fn remove_maintenance(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path(id): Path<i64>,
	Query(query): Query<Scoped>,
) -> Reply {
	editor(&web, viewer.user, query.team)?;
	web.app.remove_maintenance(query.team, id)?;
	web.uptime.sync();
	windows(&web, query.team)
}

#[derive(Deserialize)]
struct NewAlert {
	team: i64,
	target: String,
}

fn targets(web: &Web, team: i64) -> Reply {
	let list: Vec<Value> = web
		.app
		.alerts(team)?
		.into_iter()
		.filter_map(|alert| {
			let target = notify::classify(&alert.target).ok()?;
			Some(json!({
				"id": alert.id,
				"kind": target.kind,
				"label": target.label,
				"sent_at": alert.sent_at,
				"failure": alert.failure,
			}))
		})
		.collect();
	Ok(Json(json!({ "alerts": list, "mail": web.mail.ready() })))
}

async fn alerts(State(web): State<Arc<Web>>, viewer: Viewer, Query(query): Query<Scoped>) -> Reply {
	editor(&web, viewer.user, query.team)?;
	targets(&web, query.team)
}

async fn add_alert(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Json(body): Json<NewAlert>,
) -> Reply {
	editor(&web, viewer.user, body.team)?;
	let to = body.target.trim();
	notify::classify(to).map_err(|reason| Error::bad(format!("{reason}.")))?;
	if web.app.alerts(body.team)?.len() >= ALERTS {
		return Err(Error::bad("Remove an alert before adding another."));
	}
	web.app.add_alert(body.team, to)?;
	targets(&web, body.team)
}

async fn remove_alert(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path(id): Path<i64>,
	Query(query): Query<Scoped>,
) -> Reply {
	editor(&web, viewer.user, query.team)?;
	web.app.remove_alert(query.team, id)?;
	targets(&web, query.team)
}

async fn test_alert(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path(id): Path<i64>,
	Query(query): Query<Scoped>,
) -> Reply {
	editor(&web, viewer.user, query.team)?;
	let who = viewer.user.to_string();
	if !TESTS.open(&who) {
		return Err(Error::too_many());
	}
	TESTS.strike(&who);
	let to = web
		.app
		.alert(query.team, id)
		.ok_or_else(|| Error::not_found("This alert doesn't exist."))?;
	let note = Note::alert(
		"Test alert".to_owned(),
		"Uptime alerts reach this address.".to_owned(),
		format!("{}/uptime", base(&web)),
	);
	let sent = notify::deliver(&web, &to, &note, "Test", "test").await;
	web.app.mark_alert(
		id,
		&sent
			.err()
			.map(|err| notify::failure(&err))
			.unwrap_or_default(),
	)?;
	targets(&web, query.team)
}
