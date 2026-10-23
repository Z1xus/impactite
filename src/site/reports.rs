use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Path, State};
use axum::routing::{delete, get};
use axum::{Json, Router};
use jiff::civil::Date;
use jiff::tz::TimeZone;
use jiff::{Timestamp, ToSpan};
use rusqlite::params;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::app::{App, Site};
use crate::auth::accounts::Viewer;
use crate::notify::{self, Note, grouped};
use crate::stats::{self, day_number};
use crate::web::{Error, Reply, Web};

const KINDS: [&str; 7] = [
	"daily", "weekly", "biweekly", "monthly", "spike", "drop", "export",
];
const SEND_HOUR: i8 = 7;
const MAX_THRESHOLD: u64 = 1_000_000_000;

pub fn routes() -> Router<Arc<Web>> {
	Router::new()
		.route("/api/app/sites/{domain}/reports", get(list).post(add))
		.route("/api/app/sites/{domain}/reports/{id}", delete(remove))
}

struct Report {
	id: i64,
	site: Site,
	kind: String,
	email: String,
	threshold: u64,
	last: i64,
}

#[derive(Deserialize)]
struct NewReport {
	kind: String,
	email: String,
	#[serde(default)]
	threshold: u64,
}

pub async fn tick(web: &Arc<Web>) {
	let Ok(reports) = web.app.reports() else {
		return;
	};
	for report in reports {
		// one stuck webhook must not hold up the other reports or the sweep behind them
		let done = tokio::time::timeout(Duration::from_secs(120), check(web, &report)).await;
		if let Err(err) = done.unwrap_or_else(|_| Err(anyhow::anyhow!("timed out"))) {
			tracing::warn!(%err, site = report.site.domain, kind = report.kind, "report");
		}
	}
}

async fn check(web: &Web, report: &Report) -> anyhow::Result<()> {
	let tz = TimeZone::get(&report.site.timezone).unwrap_or(TimeZone::UTC);
	let now = Timestamp::now().to_zoned(tz);
	let today = now.date();
	let morning = now.hour() >= SEND_HOUR;
	match report.kind.as_str() {
		"daily" | "weekly" | "biweekly" | "monthly" => {
			let sunday =
				today.checked_sub(i64::from(today.weekday().to_monday_one_offset()).days())?;
			let (start, end) = match report.kind.as_str() {
				"daily" => {
					let day = today.checked_sub(1.day())?;
					(day, day)
				}
				"weekly" => (sunday.checked_sub(6.days())?, sunday),
				"biweekly" => (sunday.checked_sub(13.days())?, sunday),
				_ => {
					let end = today.first_of_month().checked_sub(1.day())?;
					(end.first_of_month(), end)
				}
			};
			// biweekly skips every other sunday
			let gap = if report.kind == "biweekly" { 14 } else { 1 };
			if !morning || i64::from(day_number(end)) - report.last < gap {
				return Ok(());
			}
			summary(web, report, start, end).await?;
			web.app.mark_report(report.id, day_number(end).into())?;
		}
		"export" => {
			let yesterday = today.checked_sub(1.day())?;
			if !morning || report.last >= i64::from(day_number(yesterday)) {
				return Ok(());
			}
			let bucket = crate::s3::Bucket::parse(&report.email)?;
			let archive = tokio::task::block_in_place(|| {
				crate::export::day_archive(web, &report.site, yesterday)
			})?;
			let name = format!("{}/{yesterday}.impactite.zip", report.site.domain);
			anyhow::ensure!(
				bucket.local()
					|| url::Url::parse(&report.email.replacen("s3://", "https://", 1))
						.is_ok_and(|url| crate::outbound::allowed(&url)),
				"the bucket address is not public"
			);
			let http = if bucket.local() {
				&web.http
			} else {
				&web.hooks
			};
			bucket.put(http, &name, archive).await?;
			web.app
				.mark_report(report.id, day_number(yesterday).into())?;
		}
		"spike" => {
			let ts = now.timestamp().as_second();
			let current = web.ingest.current_visitors(report.site.id) as u64;
			if current < report.threshold || ts - report.last < 12 * 3600 {
				return Ok(());
			}
			let subject = format!("Traffic spike on {}", report.site.domain);
			let line = format!(
				"{} has {current} visitors right now. Your alert is at {}.",
				report.site.domain, report.threshold
			);
			let note = Note::alert(subject, line, link(web, report, None));
			deliver(web, report, &note).await?;
			web.app.mark_report(report.id, ts)?;
		}
		_ => {
			let yesterday = today.checked_sub(1.day())?;
			if !morning || report.last >= i64::from(day_number(yesterday)) {
				return Ok(());
			}
			let visitors = totals(web, report, yesterday, yesterday)?["visitors"]
				.as_u64()
				.unwrap_or(0);
			if visitors < report.threshold {
				let subject = format!("Traffic drop on {}", report.site.domain);
				let line = format!(
					"{} had {visitors} visitors on {yesterday}. Your alert is at {}.",
					report.site.domain, report.threshold
				);
				let link = link(web, report, Some((yesterday, yesterday)));
				deliver(web, report, &Note::alert(subject, line, link)).await?;
			}
			web.app
				.mark_report(report.id, day_number(yesterday).into())?;
		}
	}
	Ok(())
}

fn query(
	web: &Web,
	report: &Report,
	from: Date,
	to: Date,
	dimension: Option<&str>,
) -> anyhow::Result<Value> {
	stats::run(
		&web.stats,
		json!({
			"site_id": report.site.domain,
			"date_range": [from.to_string(), to.to_string()],
			"metrics": ["visitors", "pageviews", "bounce_rate", "visit_duration"],
			"dimensions": dimension.into_iter().collect::<Vec<_>>(),
			"pagination": { "limit": 5 },
			"include": { "imports": true },
		}),
	)
	.map_err(anyhow::Error::msg)
}

fn totals(web: &Web, report: &Report, from: Date, to: Date) -> anyhow::Result<Value> {
	let result = query(web, report, from, to, None)?;
	let metrics = &result["results"][0]["metrics"];
	Ok(json!({
		"visitors": metrics[0],
		"pageviews": metrics[1],
		"bounce": metrics[2],
		"duration": metrics[3],
	}))
}

async fn summary(web: &Web, report: &Report, from: Date, to: Date) -> anyhow::Result<()> {
	let days = from.until(to)?.get_days() + 1;
	let before = (from.checked_sub(days.days())?, from.checked_sub(1.day())?);
	let now = totals(web, report, from, to)?;
	let then = totals(web, report, before.0, before.1)?;
	let value = |key: &str| now[key].as_u64().unwrap_or(0);
	let change = |key: &str| match then[key].as_f64().unwrap_or(0.0) {
		0.0 => String::new(),
		b => format!(
			" ({:+.0}%)",
			(now[key].as_f64().unwrap_or(0.0) - b) / b * 100.0
		),
	};
	let top = |dimension: &str| -> anyhow::Result<Vec<(String, u64)>> {
		let result = query(web, report, from, to, Some(dimension))?;
		Ok(result["results"]
			.as_array()
			.into_iter()
			.flatten()
			.map(|row| {
				(
					row["dimensions"][0].as_str().unwrap_or("").to_owned(),
					row["metrics"][0].as_u64().unwrap_or(0),
				)
			})
			.collect())
	};
	let daily = stats::run(
		&web.stats,
		json!({
			"site_id": report.site.domain,
			"date_range": [from.to_string(), to.to_string()],
			"metrics": ["visitors"],
			"dimensions": ["time:day"],
			"pagination": { "limit": 400 },
			"include": { "imports": true },
		}),
	)
	.map_err(anyhow::Error::msg)?;
	let series = from
		.series(1.day())
		.take_while(|day| *day <= to)
		.map(|day| {
			daily["results"]
				.as_array()
				.into_iter()
				.flatten()
				.find(|row| row["dimensions"][0].as_str() == Some(&day.to_string()))
				.and_then(|row| row["metrics"][0].as_u64())
				.unwrap_or(0)
		})
		.collect();
	let note = Note {
		subject: format!("{} {} report", report.site.domain, report.kind),
		line: format!("{} to {}", from.strftime("%b %-d"), to.strftime("%b %-d")),
		alert: false,
		fields: vec![
			(
				"Visitors",
				format!("{}{}", grouped(value("visitors")), change("visitors")),
			),
			(
				"Pageviews",
				format!("{}{}", grouped(value("pageviews")), change("pageviews")),
			),
			("Bounce rate", format!("{}%", value("bounce"))),
			("Visit time", format!("{}s", value("duration"))),
		],
		lists: vec![
			("Top pages", top("event:page")?),
			("Top sources", top("visit:source")?),
		],
		series,
		link: link(web, report, Some((from, to))),
	};
	deliver(web, report, &note).await
}

async fn deliver(web: &Web, report: &Report, note: &Note) -> anyhow::Result<()> {
	notify::deliver(web, &report.email, note, &report.site.domain, &report.kind).await
}

fn link(web: &Web, report: &Report, range: Option<(Date, Date)>) -> String {
	let base = format!("{}/{}", web.url.trim_end_matches('/'), report.site.domain);
	match range {
		Some((from, to)) => format!("{base}?period=custom&from={from}&to={to}"),
		None => format!("{base}?period=realtime"),
	}
}

fn channel(to: &str) -> Value {
	if let Ok(bucket) = crate::s3::Bucket::parse(to) {
		return json!({ "kind": "s3", "label": bucket.shown() });
	}
	notify::classify(to).map_or_else(
		|_| json!({ "kind": "webhook", "label": "" }),
		|target| json!(target),
	)
}

async fn list(State(web): State<Arc<Web>>, viewer: Viewer, Path(domain): Path<String>) -> Reply {
	respond(&web, web.editable(viewer.user, &domain)?.id)
}

async fn add(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path(domain): Path<String>,
	Json(body): Json<NewReport>,
) -> Reply {
	let site = web.editable(viewer.user, &domain)?;
	let alert = matches!(body.kind.as_str(), "spike" | "drop");
	let threshold = if alert { body.threshold } else { 0 };
	let to = body.email.trim();
	if body.kind == "export" {
		let bucket = crate::s3::Bucket::parse(to)
			.map_err(|err| Error::bad(format!("Check the bucket: {err}.")))?;
		let reachable = if bucket.local() {
			web.app.is_admin(viewer.user)
		} else {
			url::Url::parse(&to.replacen("s3://", "https://", 1))
				.is_ok_and(|url| crate::outbound::allowed(&url))
		};
		if !reachable {
			return Err(Error::forbidden(
				"This bucket is not reachable. A bucket on the local network needs an instance admin.",
			));
		}
	} else {
		notify::classify(to).map_err(|reason| Error::bad(format!("{reason}.")))?;
		if !KINDS.contains(&body.kind.as_str())
			|| (alert && !(1..=MAX_THRESHOLD).contains(&body.threshold))
		{
			return Err(Error::bad("Alerts need a threshold."));
		}
	}
	// starts as sent so nothing goes out right away
	let tz = TimeZone::get(&site.timezone).unwrap_or(TimeZone::UTC);
	let now = Timestamp::now().to_zoned(tz);
	let last = match body.kind.as_str() {
		"spike" => 0,
		_ => i64::from(day_number(now.date())),
	};
	web.app
		.add_report(site.id, &body.kind, to, threshold, last)?;
	respond(&web, site.id)
}

async fn remove(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path((domain, id)): Path<(String, i64)>,
) -> Reply {
	let site = web.editable(viewer.user, &domain)?;
	web.app.remove_report(site.id, id)?;
	respond(&web, site.id)
}

fn respond(web: &Web, site: u64) -> Reply {
	Ok(Json(json!({ "reports": web.app.site_reports(site)? })))
}

impl App {
	fn reports(&self) -> rusqlite::Result<Vec<Report>> {
		let conn = self.conn();
		let mut stmt = conn.prepare(
			"select r.id, s.id, s.domain, s.timezone, s.created_at, r.kind, r.email, r.threshold, r.last
			from reports r join sites s on s.id = r.site where s.team is not null",
		)?;
		stmt.query_map([], |row| {
			Ok(Report {
				id: row.get(0)?,
				site: Site {
					id: row.get::<_, i64>(1)? as u64,
					domain: row.get(2)?,
					timezone: row.get(3)?,
					created_at: row.get(4)?,
				},
				kind: row.get(5)?,
				email: row.get(6)?,
				threshold: row.get::<_, i64>(7)? as u64,
				last: row.get(8)?,
			})
		})?
		.collect()
	}

	fn site_reports(&self, site: u64) -> rusqlite::Result<Vec<Value>> {
		let conn = self.conn();
		let mut stmt = conn.prepare(
			"select id, kind, email, threshold from reports where site = ?1 order by kind, email",
		)?;
		stmt.query_map([site as i64], |row| {
			Ok(json!({
				"id": row.get::<_, i64>(0)?,
				"kind": row.get::<_, String>(1)?,
				"to": channel(&row.get::<_, String>(2)?),
				"threshold": row.get::<_, i64>(3)?,
			}))
		})?
		.collect()
	}

	fn add_report(
		&self,
		site: u64,
		kind: &str,
		email: &str,
		threshold: u64,
		last: i64,
	) -> rusqlite::Result<()> {
		self.conn().execute(
			"insert into reports (site, kind, email, threshold, last) values (?1, ?2, ?3, ?4, ?5)
			on conflict (site, kind, email) do update set threshold = ?4",
			params![site as i64, kind, email, threshold as i64, last],
		)?;
		Ok(())
	}

	fn remove_report(&self, site: u64, id: i64) -> rusqlite::Result<()> {
		self.conn().execute(
			"delete from reports where site = ?1 and id = ?2",
			params![site as i64, id],
		)?;
		Ok(())
	}

	fn mark_report(&self, id: i64, last: i64) -> rusqlite::Result<()> {
		self.conn().execute(
			"update reports set last = ?2 where id = ?1",
			params![id, last],
		)?;
		Ok(())
	}
}
