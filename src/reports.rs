use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get};
use axum::{Json, Router};
use jiff::civil::Date;
use jiff::tz::TimeZone;
use jiff::{Timestamp, ToSpan};
use rusqlite::params;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::accounts::{Viewer, Web, fail, internal};
use crate::app::{App, Site};
use crate::stats::{self, day_number};

const KINDS: [&str; 7] = [
	"daily", "weekly", "biweekly", "monthly", "spike", "drop", "export",
];
const SEND_HOUR: i8 = 7;

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

// runs every minute, sends whatever is due
pub async fn tick(web: &Arc<Web>) {
	let Ok(reports) = web.app.reports() else {
		return;
	};
	for report in reports {
		if let Err(err) = check(web, &report).await {
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
			bucket.put(&web.http, &name, archive).await?;
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
			let text = format!(
				"{} has {current} visitors right now, at or above your alert of {}.\n\n{}",
				report.site.domain,
				report.threshold,
				link(web, report, None)
			);
			deliver(web, report, &subject, &text).await?;
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
				let text = format!(
					"{} had {visitors} visitors on {yesterday}, below your alert of {}.\n\n{}",
					report.site.domain,
					report.threshold,
					link(web, report, Some((yesterday, yesterday)))
				);
				deliver(web, report, &subject, &text).await?;
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
	let change = |key: &str| {
		let (a, b) = (
			now[key].as_f64().unwrap_or(0.0),
			then[key].as_f64().unwrap_or(0.0),
		);
		match b {
			0.0 => String::new(),
			_ => format!(" ({:+.0}%)", (a - b) / b * 100.0),
		}
	};
	let top = |dimension: &str| -> anyhow::Result<String> {
		let result = query(web, report, from, to, Some(dimension))?;
		Ok(result["results"]
			.as_array()
			.into_iter()
			.flatten()
			.map(|row| {
				format!(
					"{:>8}  {}",
					row["metrics"][0].to_string(),
					row["dimensions"][0].as_str().unwrap_or("")
				)
			})
			.collect::<Vec<_>>()
			.join("\n"))
	};
	let text = format!(
		"{domain}, {from} to {to}\n\n\
		visitors      {visitors}{visitors_change}\n\
		pageviews     {pageviews}{pageviews_change}\n\
		bounce rate   {bounce}%\n\
		visit time    {duration}s\n\n\
		top pages\n{pages}\n\n\
		top sources\n{sources}\n\n{link}",
		domain = report.site.domain,
		visitors = now["visitors"],
		visitors_change = change("visitors"),
		pageviews = now["pageviews"],
		pageviews_change = change("pageviews"),
		bounce = now["bounce"],
		duration = now["duration"],
		pages = top("event:page")?,
		sources = top("visit:source")?,
		link = link(web, report, Some((from, to))),
	);
	let subject = format!("{} {} report", report.site.domain, report.kind);
	deliver(web, report, &subject, &text).await
}

// mail, or a webhook: slack and discord get their own format, anything else gets json
async fn deliver(web: &Web, report: &Report, subject: &str, text: &str) -> anyhow::Result<()> {
	let to = report.email.as_str();
	if !to.starts_with("https://") {
		return web.mail.send(to, subject, text, &html(text)).await;
	}
	let body = if to.starts_with("https://hooks.slack.com/") {
		json!({ "text": format!("*{subject}*\n```{text}```") })
	} else if to.contains("discord.com/api/webhooks/") {
		let text: String = text.chars().take(1900).collect();
		json!({ "content": format!("**{subject}**\n```{text}```") })
	} else {
		json!({ "subject": subject, "text": text, "site": report.site.domain, "kind": report.kind })
	};
	web.http
		.post(to)
		.json(&body)
		.send()
		.await?
		.error_for_status()?;
	Ok(())
}

fn link(web: &Web, report: &Report, range: Option<(Date, Date)>) -> String {
	let base = format!("{}/{}", web.url.trim_end_matches('/'), report.site.domain);
	match range {
		Some((from, to)) => format!("{base}?period=custom&from={from}&to={to}"),
		None => format!("{base}?period=realtime"),
	}
}

fn shown(to: &str) -> String {
	if let Ok(bucket) = crate::s3::Bucket::parse(to) {
		return bucket.shown();
	}
	match url::Url::parse(to) {
		Ok(url) if to.starts_with("https://") => {
			format!("webhook to {}", url.host_str().unwrap_or_default())
		}
		_ => to.to_owned(),
	}
}

// monospace keeps the plain text columns readable in mail clients
fn html(text: &str) -> String {
	let escaped = text
		.replace('&', "&amp;")
		.replace('<', "&lt;")
		.replace('>', "&gt;");
	format!("<pre style=\"font-family:ui-monospace,monospace;font-size:13px\">{escaped}</pre>")
}

fn editable(web: &Web, user: i64, domain: &str) -> Option<Site> {
	let role = web.app.site_role(user, domain).ok().flatten()?;
	(role != "viewer").then_some(())?;
	web.app.site(domain).ok().flatten()
}

async fn list(State(web): State<Arc<Web>>, viewer: Viewer, Path(domain): Path<String>) -> Response {
	match editable(&web, viewer.user, &domain) {
		Some(site) => respond(&web, site.id),
		None => fail(StatusCode::FORBIDDEN, "You can't change this site."),
	}
}

async fn add(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path(domain): Path<String>,
	Json(body): Json<NewReport>,
) -> Response {
	let Some(site) = editable(&web, viewer.user, &domain) else {
		return fail(StatusCode::FORBIDDEN, "You can't change this site.");
	};
	let alert = matches!(body.kind.as_str(), "spike" | "drop");
	let to = body.email.trim();
	if body.kind == "export" {
		if let Err(err) = crate::s3::Bucket::parse(to) {
			return fail(
				StatusCode::BAD_REQUEST,
				&format!("Check the bucket: {err}."),
			);
		}
	} else {
		let webhook = to.starts_with("https://") && url::Url::parse(to).is_ok();
		if !KINDS.contains(&body.kind.as_str())
			|| !(webhook || to.contains('@'))
			|| (alert && body.threshold == 0)
		{
			return fail(
				StatusCode::BAD_REQUEST,
				"Enter an email or an https webhook. Alerts also need a threshold.",
			);
		}
	}
	// starts as sent so nothing goes out right away
	let tz = TimeZone::get(&site.timezone).unwrap_or(TimeZone::UTC);
	let now = Timestamp::now().to_zoned(tz);
	let last = match body.kind.as_str() {
		"spike" => 0,
		_ => i64::from(day_number(now.date())),
	};
	match web
		.app
		.add_report(site.id, &body.kind, to, body.threshold, last)
	{
		Ok(()) => respond(&web, site.id),
		Err(err) => internal(err),
	}
}

async fn remove(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path((domain, id)): Path<(String, i64)>,
) -> Response {
	let Some(site) = editable(&web, viewer.user, &domain) else {
		return fail(StatusCode::FORBIDDEN, "You can't change this site.");
	};
	match web.app.remove_report(site.id, id) {
		Ok(()) => respond(&web, site.id),
		Err(err) => internal(err),
	}
}

fn respond(web: &Web, site: u64) -> Response {
	match web.app.site_reports(site) {
		Ok(reports) => Json(json!({ "reports": reports })).into_response(),
		Err(err) => internal(err),
	}
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
				// a webhook url is its secret, only its host goes back
				"email": shown(&row.get::<_, String>(2)?),
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
