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
use crate::card;
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
			let line = format!(
				"{} has {current} visitors right now. Your alert is at {}.",
				report.site.domain, report.threshold
			);
			deliver(
				web,
				report,
				&Note::alert(subject, line, link(web, report, None)),
			)
			.await?;
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

struct Note {
	subject: String,
	line: String,
	alert: bool,
	fields: Vec<(&'static str, String)>,
	lists: Vec<(&'static str, Vec<(String, u64)>)>,
	series: Vec<u64>,
	link: String,
}

impl Note {
	fn alert(subject: String, line: String, link: String) -> Self {
		Self {
			subject,
			line,
			alert: true,
			fields: Vec::new(),
			lists: Vec::new(),
			series: Vec::new(),
			link,
		}
	}

	// the monospace block in mails and plain webhooks
	fn table(&self) -> String {
		let fields = self
			.fields
			.iter()
			.map(|(label, value)| format!("{label:<13} {value}"));
		let lists = self.lists.iter().map(|(label, rows)| {
			let rows = rows
				.iter()
				.map(|(name, count)| format!("{:>8}  {name}", grouped(*count)));
			std::iter::once(format!("\n{label}"))
				.chain(rows)
				.collect::<Vec<_>>()
				.join("\n")
		});
		fields.chain(lists).collect::<Vec<_>>().join("\n")
	}
}

pub(crate) fn grouped(n: u64) -> String {
	let digits = n.to_string();
	let mut out = String::new();
	for (i, ch) in digits.chars().enumerate() {
		if i > 0 && (digits.len() - i).is_multiple_of(3) {
			out.push(',');
		}
		out.push(ch);
	}
	out
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

// mail, or a webhook: slack and discord get their own layout, anything else gets json
async fn deliver(web: &Web, report: &Report, note: &Note) -> anyhow::Result<()> {
	let to = report.email.as_str();
	let table = note.table();
	if !to.starts_with("https://") {
		let lines = [note.line.clone()];
		let letter = crate::mail::Letter {
			base: &web.url,
			title: &note.subject,
			lines: &lines,
			table: (!table.is_empty()).then_some(table.as_str()),
			button: Some(("Open dashboard", &note.link)),
		};
		return web
			.mail
			.send(to, &note.subject, &letter.text(), &letter.html())
			.await;
	}
	let request = if to.starts_with("https://hooks.slack.com/") {
		web.http.post(to).json(&slack(note))
	} else if to.contains("discord.com/api/webhooks/") {
		discord(web, note, to)?
	} else {
		let text = [note.line.as_str(), &table, &note.link].join("\n\n");
		web.http.post(to).json(&json!({
			"subject": note.subject,
			"text": text,
			"site": report.site.domain,
			"kind": report.kind,
			"link": note.link,
		}))
	};
	request.send().await?.error_for_status()?;
	Ok(())
}

fn discord(web: &Web, note: &Note, to: &str) -> anyhow::Result<reqwest::RequestBuilder> {
	let mut fields: Vec<Value> = note
		.fields
		.iter()
		.take(3)
		.map(|(name, value)| json!({ "name": name, "value": value, "inline": true }))
		.collect();
	fields.extend(
		note.lists
			.iter()
			.filter(|(_, rows)| !rows.is_empty())
			.map(|(name, rows)| {
				let value = rows
					.iter()
					.map(|(row, count)| format!("{row}  {}", grouped(*count)))
					.collect::<Vec<_>>()
					.join("\n");
				json!({ "name": name, "value": value.chars().take(1000).collect::<String>(), "inline": true })
			}),
	);
	let mut embed = json!({
		"title": note.subject,
		"url": note.link,
		"description": note.line,
		"color": if note.alert { 0xf0645a } else { 0x38e1f2 },
		"fields": fields,
		"footer": { "text": "impactite" },
	});
	if note.series.is_empty() {
		return Ok(web.http.post(to).json(&json!({ "embeds": [embed] })));
	}
	embed["image"] = json!({ "url": "attachment://chart.png" });
	let mut chart = card::Canvas::new(800, 200, card::INK);
	chart.area([0, 8, 800, 192], &note.series, 8);
	let form = reqwest::multipart::Form::new()
		.text("payload_json", json!({ "embeds": [embed] }).to_string())
		.part(
			"files[0]",
			reqwest::multipart::Part::bytes(chart.png())
				.file_name("chart.png")
				.mime_str("image/png")?,
		);
	Ok(web.http.post(to).multipart(form))
}

fn slack(note: &Note) -> Value {
	let mut blocks = vec![
		json!({ "type": "header", "text": { "type": "plain_text", "text": note.subject } }),
		json!({ "type": "section", "text": { "type": "mrkdwn", "text": note.line } }),
	];
	if !note.fields.is_empty() {
		let fields: Vec<Value> = note
			.fields
			.iter()
			.map(|(name, value)| json!({ "type": "mrkdwn", "text": format!("*{name}*\n{value}") }))
			.collect();
		blocks.push(json!({ "type": "section", "fields": fields }));
	}
	for (name, rows) in note.lists.iter().filter(|(_, rows)| !rows.is_empty()) {
		let rows = rows
			.iter()
			.map(|(row, count)| format!("{row}  {}", grouped(*count)))
			.collect::<Vec<_>>()
			.join("\n");
		blocks.push(
			json!({ "type": "section", "text": { "type": "mrkdwn", "text": format!("*{name}*\n{rows}") } }),
		);
	}
	blocks.push(json!({
		"type": "actions",
		"elements": [{ "type": "button", "text": { "type": "plain_text", "text": "Open dashboard" }, "url": note.link }],
	}));
	json!({ "text": format!("{}. {}", note.subject, note.line), "blocks": blocks })
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

async fn list(State(web): State<Arc<Web>>, viewer: Viewer, Path(domain): Path<String>) -> Response {
	match web.app.editable(viewer.user, &domain) {
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
	let Some(site) = web.app.editable(viewer.user, &domain) else {
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
	let Some(site) = web.app.editable(viewer.user, &domain) else {
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
