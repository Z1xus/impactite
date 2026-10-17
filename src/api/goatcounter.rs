use std::collections::{BTreeMap, HashMap};
use std::net::IpAddr;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use jiff::civil::Date;
use jiff::tz::TimeZone;
use jiff::{Timestamp, ToSpan};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{Call, Fail, broken, reply};
use crate::app::Site;
use crate::export;
use crate::ingest::{Client, Hit};
use crate::names;
use crate::stats::{self, answer, period};
use crate::web::Web;

const LIMIT: usize = 100;
const VIEWS: &str = "pageview";

pub fn routes() -> Router<Arc<Web>> {
	Router::new()
		.route("/api/v0/me", get(me))
		.route("/api/v0/count", post(count))
		.route("/api/v0/sites", get(sites))
		.route("/api/v0/sites/{id}", get(site))
		.route("/api/v0/paths", get(paths))
		.route("/api/v0/stats/total", get(total))
		.route("/api/v0/stats/hits", get(hits))
		.route("/api/v0/stats/hits/{path_id}", get(refs))
		.route("/api/v0/stats/{page}", get(page))
		.route("/api/v0/stats/{page}/{id}", get(detail))
		.route("/api/v0/export", post(start_export))
		.route("/api/v0/export/{id}", get(export_status))
		.route("/api/v0/export/{id}/download", get(download))
}

#[derive(Deserialize)]
struct CountHit {
	#[serde(default)]
	path: String,
	#[serde(default)]
	event: Value,
	#[serde(default, rename = "ref")]
	referrer: String,
	#[serde(default)]
	title: String,
	#[serde(default)]
	query: String,
	#[serde(default)]
	bot: u32,
	#[serde(default)]
	user_agent: String,
	#[serde(default)]
	location: String,
	#[serde(default)]
	ip: String,
	#[serde(default)]
	session: String,
	created_at: Option<String>,
}

fn bad(message: impl Into<String>) -> Fail {
	(StatusCode::BAD_REQUEST, message.into())
}

impl Call {
	// goatcounter picks the site by host, here also by ?site=domain or the only site
	fn counter(&self, web: &Web) -> Result<Site, Fail> {
		let sites = web.app.user_sites(self.user).map_err(broken)?;
		let wanted = self
			.params
			.get("site")
			.map_or(self.host.as_str(), String::as_str);
		let wanted = wanted.strip_prefix("www.").unwrap_or(wanted);
		if let Some(site) = sites
			.iter()
			.find(|site| site.domain == wanted || site.id.to_string() == wanted)
		{
			return Ok(site.clone());
		}
		match sites.as_slice() {
			[site] => Ok(site.clone()),
			_ => Err(bad("Pick a site with ?site=domain")),
		}
	}

	fn range(&self, web: &Web) -> Result<(Site, Date, Date), Fail> {
		let site = self.counter(web)?;
		let tz = TimeZone::get(&site.timezone).unwrap_or(TimeZone::UTC);
		let date = |name: &str| {
			self.params
				.get(name)
				.map(|value| {
					value
						.get(..10)
						.unwrap_or(value)
						.parse::<Date>()
						.map_err(|_| bad(format!("{name}: must be a date")))
				})
				.transpose()
		};
		let today = Timestamp::now().to_zoned(tz.clone()).date();
		let to = date("end")?.unwrap_or(today);
		let from = match date("start")? {
			Some(from) => from,
			None => to.checked_sub(6.days()).map_err(broken)?,
		};
		if from > to {
			return Err(bad("start: must be before end"));
		}
		let (created, _) = stats::created(&web.app, &site, &tz).map_err(broken)?;
		let (from, to) = stats::bound(from, to, created, today);
		Ok((site, from, to))
	}

	fn number(&self, name: &str, default: usize) -> Result<usize, Fail> {
		self.params
			.get(name)
			.map_or(Ok(default), |value| value.parse())
			.map_err(|_| bad(format!("{name}: must be a number")))
	}

	fn ids(&self, name: &str) -> Vec<u64> {
		self.params
			.get(name)
			.map(|value| {
				value
					.trim_matches(['[', ']'])
					.split(',')
					.filter_map(|id| id.trim().parse().ok())
					.collect()
			})
			.unwrap_or_default()
	}
}

fn ask(
	web: &Web,
	site: &Site,
	(from, to): (Date, Date),
	dimensions: &[&str],
	filters: Value,
) -> Result<Vec<(Vec<String>, u64)>, Fail> {
	let answer = answer(
		&web.stats,
		json!({
			"site_id": site.domain,
			"metrics": ["visitors"],
			"date_range": [from.to_string(), to.to_string()],
			"dimensions": dimensions,
			"filters": filters,
		}),
	)
	.map_err(bad)?;
	Ok(answer
		.results
		.into_iter()
		.map(|line| {
			let dimensions = line
				.dimensions
				.into_iter()
				.map(|value| match value {
					Value::String(text) => text,
					_ => String::new(),
				})
				.collect();
			(dimensions, line.metrics[0].as_u64().unwrap_or_default())
		})
		.collect())
}

struct Entry {
	id: u64,
	path: String,
	title: String,
	event: bool,
	count: u64,
}

// stable ids for paths and events, small enough for javascript numbers
fn path_id(path: &str, event: bool) -> u64 {
	let hash = blake3::hash(format!("{}{path}", u8::from(event)).as_bytes());
	let mut id = [0; 8];
	id.copy_from_slice(&hash.as_bytes()[..8]);
	u64::from_le_bytes(id) >> 11
}

fn entries(web: &Web, site: &Site, range: (Date, Date)) -> Result<Vec<Entry>, Fail> {
	let titles = web.app.titles(site.id).map_err(broken)?;
	let pages = ask(
		web,
		site,
		range,
		&["event:page"],
		json!([["is", "event:name", [VIEWS]]]),
	)?;
	let events = ask(
		web,
		site,
		range,
		&["event:name"],
		json!([["is_not", "event:name", [VIEWS, "engagement"]]]),
	)?;
	let mut entries: Vec<Entry> = pages
		.into_iter()
		.map(|row| (row, false))
		.chain(events.into_iter().map(|row| (row, true)))
		.map(|((mut dimensions, count), event)| {
			let path = dimensions.swap_remove(0);
			Entry {
				id: path_id(&path, event),
				title: if event {
					String::new()
				} else {
					titles.get(&path).cloned().unwrap_or_default()
				},
				path,
				event,
				count,
			}
		})
		.collect();
	entries.sort_by(|a, b| b.count.cmp(&a.count).then(a.path.cmp(&b.path)));
	Ok(entries)
}

fn entry_filter(entry: &Entry) -> Value {
	if entry.event {
		json!([["is", "event:name", [entry.path]]])
	} else {
		json!([
			["is", "event:name", [VIEWS]],
			["is", "event:page", [entry.path]]
		])
	}
}

fn days(from: Date, to: Date) -> impl Iterator<Item = Date> {
	from.series(1.day()).take_while(move |day| *day <= to)
}

async fn me(State(web): State<Arc<Web>>, call: Call) -> Response {
	reply(web.app.me(call.user).map_err(broken).map(
		|me| json!({ "user": { "id": me["id"], "email": me["email"], "user_name": me["name"] } }),
	))
}

fn site_json(site: &Site) -> Value {
	json!({
		"id": site.id,
		"code": site.domain,
		"cname": site.domain,
		"link_domain": site.domain,
		"settings": { "timezone": site.timezone },
		"created_at": Timestamp::from_second(site.created_at).ok().map(|at| at.to_string()),
	})
}

async fn sites(State(web): State<Arc<Web>>, call: Call) -> Response {
	reply(
		web.app
			.user_sites(call.user)
			.map_err(broken)
			.map(|sites| json!({ "sites": sites.iter().map(site_json).collect::<Vec<_>>() })),
	)
}

async fn site(State(web): State<Arc<Web>>, Path(id): Path<String>, call: Call) -> Response {
	reply((|| {
		let sites = web.app.user_sites(call.user).map_err(broken)?;
		sites
			.iter()
			.find(|site| site.id.to_string() == id || site.domain == id)
			.map(site_json)
			.ok_or((StatusCode::NOT_FOUND, "Site not found".to_owned()))
	})())
}

async fn count(State(web): State<Arc<Web>>, call: Call) -> Response {
	let result = (|| {
		let site = call.counter(&web)?;
		let tracked = web
			.ingest
			.site(&site.domain)
			.ok_or_else(|| broken("site"))?;
		let raw = call
			.params
			.get("hits")
			.ok_or_else(|| bad("hits: required"))?;
		let hits: Vec<CountHit> =
			serde_json::from_str(raw).map_err(|err| bad(format!("hits: {err}")))?;
		let no_sessions = call
			.params
			.get("no_sessions")
			.is_some_and(|value| value == "true");
		let mut errors = serde_json::Map::new();
		for (i, hit) in hits.into_iter().enumerate() {
			let event = matches!(&hit.event, Value::Bool(true))
				|| matches!(&hit.event, Value::String(text) if text == "true");
			let problem = match () {
				_ if hit.path.is_empty() => Some("path: required"),
				_ if !no_sessions && hit.session.is_empty() && hit.user_agent.is_empty() => {
					Some("session or user_agent: required unless no_sessions is set")
				}
				_ => None,
			};
			if let Some(problem) = problem {
				errors.insert(i.to_string(), json!(problem));
				continue;
			}
			if hit.bot >= 150 {
				continue;
			}
			// without an ip the session stands in as a private address, so its hits group
			let ip = hit.ip.parse().unwrap_or_else(|_| {
				let mut octets = [0; 16];
				octets.copy_from_slice(&blake3::hash(hit.session.as_bytes()).as_bytes()[..16]);
				octets[0] = 0xfd;
				IpAddr::from(octets)
			});
			// the caller vouches for hits without a user agent through `bot`
			let ua = if hit.user_agent.is_empty() {
				"goatcounter api, no user agent"
			} else {
				hit.user_agent.as_str()
			};
			let client = Client {
				ip,
				ua,
				country: Some(hit.location.chars().take(2).collect::<String>())
					.filter(|code| code.chars().count() == 2)
					.map(|code| code.to_uppercase()),
				language: String::new(),
			};
			let query = hit.query.trim_start_matches('?');
			let tracked_hit = Hit {
				name: if event {
					hit.path.clone()
				} else {
					VIEWS.into()
				},
				hostname: site.domain.clone(),
				path: if event {
					String::new()
				} else {
					format!("/{}", hit.path.trim_start_matches('/'))
				},
				params: url::form_urlencoded::parse(query.as_bytes())
					.into_owned()
					.collect(),
				referrer: hit.referrer,
				props: Vec::new(),
				revenue: None,
				interactive: true,
				scroll_depth: None,
				engagement_ms: 0,
				at: hit.created_at.as_deref().and_then(|at| at.parse().ok()),
				title: hit.title,
			};
			let _ = web.ingest.track(&tracked, tracked_hit, &client);
		}
		Ok::<_, Fail>(errors)
	})();
	match result {
		Ok(errors) if errors.is_empty() => (StatusCode::ACCEPTED, Json(json!({}))).into_response(),
		Ok(errors) => (StatusCode::BAD_REQUEST, Json(json!({ "errors": errors }))).into_response(),
		Err(fail) => reply(Err(fail)),
	}
}

async fn paths(State(web): State<Arc<Web>>, call: Call) -> Response {
	reply((|| {
		let site = call.counter(&web)?;
		let all = period(&web.stats, &site.domain, &json!("all"), true).map_err(bad)?;
		let limit = call.number("limit", LIMIT)?.clamp(1, LIMIT);
		let after = call.number("after", 0)? as u64;
		let mut entries = entries(&web, &site, (all.from, all.to))?;
		entries.sort_by_key(|entry| entry.id);
		entries.retain(|entry| entry.id > after);
		let more = entries.len() > limit;
		entries.truncate(limit);
		Ok(json!({
			"paths": entries
				.iter()
				.map(|entry| json!({ "id": entry.id, "path": entry.path, "title": entry.title, "event": entry.event }))
				.collect::<Vec<_>>(),
			"more": more,
		}))
	})())
}

async fn total(State(web): State<Arc<Web>>, call: Call) -> Response {
	reply((|| {
		let (site, from, to) = call.range(&web)?;
		// goatcounter counts a visitor once per page or event a day, then adds those up
		let sum = |rows: Vec<(Vec<String>, u64)>| rows.iter().map(|(_, count)| count).sum::<u64>();
		let mut pages = HashMap::new();
		let mut events = HashMap::new();
		for day in days(from, to) {
			let range = (day, day);
			let views = json!([["is", "event:name", [VIEWS]]]);
			let others = json!([["is_not", "event:name", [VIEWS, "engagement"]]]);
			pages.insert(
				day.to_string(),
				sum(ask(&web, &site, range, &["event:page"], views)?),
			);
			events.insert(
				day.to_string(),
				sum(ask(&web, &site, range, &["event:name"], others)?),
			);
		}
		let mut hourly: HashMap<&str, [u64; 24]> = HashMap::new();
		let hours = ask(&web, &site, (from, to), &["time:hour"], json!([]))?;
		for (dimensions, count) in &hours {
			if let Some((day, hour)) = dimensions.first().and_then(|label| label.split_once(' ')) {
				let hour: usize = hour
					.get(..2)
					.and_then(|hour| hour.parse().ok())
					.unwrap_or(0);
				hourly.entry(day).or_default()[hour.min(23)] = *count;
			}
		}
		let stats: Vec<Value> = days(from, to)
			.map(|day| {
				let day = day.to_string();
				let daily =
					pages.get(&day).copied().unwrap_or(0) + events.get(&day).copied().unwrap_or(0);
				let hours = spread(hourly.get(day.as_str()).copied().unwrap_or_default(), daily);
				json!({ "day": day, "hourly": hours, "daily": daily })
			})
			.collect();
		let total = pages.values().chain(events.values()).sum::<u64>();
		Ok(
			json!({ "total": total, "total_events": events.values().sum::<u64>(), "total_utc": total, "stats": stats }),
		)
	})())
}

async fn hits(State(web): State<Arc<Web>>, call: Call) -> Response {
	reply((|| {
		let (site, from, to) = call.range(&web)?;
		let limit = call.number("limit", 20)?.clamp(1, LIMIT);
		let include = call.ids("include_paths");
		let exclude = call.ids("exclude_paths");
		let mut entries = entries(&web, &site, (from, to))?;
		entries.retain(|entry| {
			(include.is_empty() || include.contains(&entry.id)) && !exclude.contains(&entry.id)
		});
		let more = entries.len() > limit;
		entries.truncate(limit);

		// one query per kind gives every listed path its days
		let listed = |event: bool| -> Vec<&str> {
			entries
				.iter()
				.filter(|entry| entry.event == event)
				.map(|entry| entry.path.as_str())
				.collect()
		};
		let mut daily: HashMap<(bool, String), BTreeMap<String, u64>> = HashMap::new();
		for (event, dimension) in [(false, "event:page"), (true, "event:name")] {
			let names = listed(event);
			if names.is_empty() {
				continue;
			}
			let filter = if event {
				json!([["is", "event:name", names]])
			} else {
				json!([["is", "event:name", [VIEWS]], ["is", "event:page", names]])
			};
			for (dimensions, count) in
				ask(&web, &site, (from, to), &["time:day", dimension], filter)?
			{
				if let [day, path] = dimensions.as_slice() {
					daily
						.entry((event, path.clone()))
						.or_default()
						.insert(day.clone(), count);
				}
			}
		}
		let total = ask(&web, &site, (from, to), &[], json!([]))?
			.first()
			.map_or(0, |(_, count)| *count);
		let hits: Vec<Value> = entries
			.iter()
			.map(|entry| {
				let counts = daily.get(&(entry.event, entry.path.clone()));
				let stats: Vec<Value> = days(from, to)
					.map(|day| {
						let count = counts
							.and_then(|counts| counts.get(&day.to_string()))
							.copied()
							.unwrap_or_default();
						json!({ "day": day.to_string(), "daily": count })
					})
					.collect();
				json!({
					"path_id": entry.id,
					"path": entry.path,
					"event": entry.event,
					"title": entry.title,
					"count": entry.count,
					"max": counts.and_then(|counts| counts.values().max()).copied().unwrap_or_default(),
					"stats": stats,
				})
			})
			.collect();
		Ok(json!({ "hits": hits, "total": total, "more": more }))
	})())
}

async fn refs(State(web): State<Arc<Web>>, Path(id): Path<u64>, call: Call) -> Response {
	reply((|| {
		let (site, from, to) = call.range(&web)?;
		let entry = entries(&web, &site, (from, to))?
			.into_iter()
			.find(|entry| entry.id == id)
			.ok_or((StatusCode::NOT_FOUND, "Path not found".to_owned()))?;
		let refs = ask(
			&web,
			&site,
			(from, to),
			&["visit:source"],
			entry_filter(&entry),
		)?
		.into_iter()
		.map(|(mut dimensions, count)| {
			let name = dimensions.swap_remove(0);
			match name.as_str() {
				"Direct / None" => json!({ "name": "", "count": count, "ref_scheme": null }),
				_ => json!({ "name": name, "count": count, "ref_scheme": "g" }),
			}
		})
		.collect::<Vec<_>>();
		Ok(json!({ "refs": refs, "more": false }))
	})())
}

// goatcounter groups widths, we keep device classes
fn size(device: &str) -> (&'static str, &'static str) {
	match device {
		"Mobile" => ("phone", "Phones"),
		"Tablet" => ("tablet", "Tablets and small laptops"),
		"Laptop" => ("desktop", "Computer monitors"),
		"Desktop" => ("desktophd", "Computer monitors larger than HD"),
		_ => ("unknown", "(unknown)"),
	}
}

fn listed(
	call: &Call,
	rows: Vec<(Vec<String>, u64)>,
	name: impl Fn(&str) -> (String, String),
) -> Result<Value, Fail> {
	let limit = call.number("limit", 10)?.clamp(1, LIMIT);
	let offset = call.number("offset", 0)?;
	let more = rows.len() > offset + limit;
	let stats: Vec<Value> = rows
		.into_iter()
		.skip(offset)
		.take(limit)
		.map(|(dimensions, count)| {
			let (id, name) = name(dimensions.first().map_or("", String::as_str));
			json!({ "id": id, "name": name, "count": count })
		})
		.collect();
	Ok(json!({ "stats": stats, "more": more }))
}

async fn page(State(web): State<Arc<Web>>, Path(page): Path<String>, call: Call) -> Response {
	reply((|| {
		let (site, from, to) = call.range(&web)?;
		let dimension = match page.as_str() {
			"browsers" => "visit:browser",
			"systems" => "visit:os",
			"locations" => "visit:country",
			"sizes" => "visit:device",
			"campaigns" => "visit:utm_campaign",
			"toprefs" => "visit:source",
			"languages" => "visit:language",
			_ => return Err((StatusCode::NOT_FOUND, format!("Unknown page {page}"))),
		};
		let mut rows = ask(&web, &site, (from, to), &[dimension], json!([]))?;
		if matches!(page.as_str(), "campaigns" | "languages") {
			rows.retain(|(dimensions, _)| dimensions[0] != "(none)");
		}
		listed(&call, rows, |value| match page.as_str() {
			"sizes" => {
				let (id, name) = size(value);
				(id.to_owned(), name.to_owned())
			}
			"locations" => (value.to_owned(), names::country(value).to_owned()),
			"languages" => (value.to_owned(), names::language(value).to_owned()),
			_ => (value.to_owned(), value.to_owned()),
		})
	})())
}

async fn detail(
	State(web): State<Arc<Web>>,
	Path((page, id)): Path<(String, String)>,
	call: Call,
) -> Response {
	reply((|| {
		let (site, from, to) = call.range(&web)?;
		let (filter, dimension) = match page.as_str() {
			"browsers" => ("visit:browser", "visit:browser_version"),
			"systems" => ("visit:os", "visit:os_version"),
			"locations" => ("visit:country", "visit:region"),
			"toprefs" => ("visit:source", "visit:referrer"),
			"campaigns" => ("visit:utm_campaign", "visit:utm_source"),
			"sizes" => ("visit:device", "visit:device"),
			_ => return Err((StatusCode::NOT_FOUND, format!("Unknown page {page}"))),
		};
		let value = match page.as_str() {
			"sizes" => ["Mobile", "Tablet", "Laptop", "Desktop", ""]
				.into_iter()
				.find(|device| size(device).0 == id)
				.unwrap_or_default()
				.to_owned(),
			_ => id.clone(),
		};
		let rows = ask(
			&web,
			&site,
			(from, to),
			&[dimension],
			json!([["is", filter, [value]]]),
		)?;
		listed(&call, rows, |value| match page.as_str() {
			"browsers" | "systems" => (value.to_owned(), format!("{id} {value}")),
			"sizes" => {
				let (id, name) = size(value);
				(id.to_owned(), name.to_owned())
			}
			_ => (value.to_owned(), value.to_owned()),
		})
	})())
}

// exports are built on download, so every export is finished at once and covers all days
fn export_json(site: &Site, id: u64) -> Value {
	json!({
		"id": id,
		"site_id": site.id,
		"start_from_hit_id": 0,
		"created_at": Timestamp::now().to_string(),
		"finished_at": Timestamp::now().to_string(),
		"error": null,
	})
}

async fn start_export(State(web): State<Arc<Web>>, call: Call) -> Response {
	reply(call.counter(&web).map(|site| export_json(&site, site.id)))
}

async fn export_status(State(web): State<Arc<Web>>, Path(id): Path<u64>, call: Call) -> Response {
	reply(call.counter(&web).map(|site| export_json(&site, id)))
}

async fn download(State(web): State<Arc<Web>>, Path(_id): Path<u64>, call: Call) -> Response {
	let site = match call.counter(&web) {
		Ok(site) => site,
		Err(fail) => return reply(Err(fail)),
	};
	let Ok(all) = period(&web.stats, &site.domain, &json!("all"), true) else {
		return reply(Err(broken("period")));
	};
	let db = web.stats.db.clone();
	let titles = web.app.titles(site.id).unwrap_or_default();
	let name = format!("{}.goatcounter.csv.gz", site.domain);
	match tokio::task::spawn_blocking(move || {
		export::goatcounter(&db, &site, &titles, all.from, all.to)
	})
	.await
	{
		Ok(Ok(body)) => (
			[
				(header::CONTENT_TYPE, "application/gzip".to_owned()),
				(
					header::CONTENT_DISPOSITION,
					format!("attachment; filename=\"{name}\""),
				),
			],
			body,
		)
			.into_response(),
		_ => reply(Err(broken("export"))),
	}
}

// goatcounter counts a visit per page, hours here count visitors, so the day's visits are
// shared out over its hours in their proportion, largest remainders first
fn spread(hours: [u64; 24], total: u64) -> [u64; 24] {
	let sum: u64 = hours.iter().sum();
	if sum == 0 {
		return hours;
	}
	let mut out = hours.map(|hour| hour * total / sum);
	let mut rest: Vec<usize> = (0..24).collect();
	rest.sort_by_key(|&i| std::cmp::Reverse(hours[i] * total % sum));
	let left = total - out.iter().sum::<u64>();
	for &i in rest.iter().take(left as usize) {
		out[i] += 1;
	}
	out
}
