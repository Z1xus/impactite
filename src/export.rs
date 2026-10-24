use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{BufWriter, Cursor, Read, Seek, Write};
use std::ops::RangeInclusive;
use std::path::{Path as FilePath, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};

use axum::body::{Body, Bytes};
use axum::extract::{Path, Query, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use crater::{Db, Dim, Event, Field, Filter, Metrics, Report};
use flate2::Compression;
use flate2::write::GzEncoder;
use jiff::civil::Date;
use jiff::tz::TimeZone;
use jiff::{Timestamp, ToSpan};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

use crate::app::{Site, lock, now, token};
use crate::auth::accounts::Viewer;
use crate::mail::Letter;
use crate::site::goals::Goal;
use crate::stats::{self, day_number};
use crate::web::{Error, Reply, Web};

#[derive(Deserialize)]
pub struct ExportRequest {
	site_id: String,
	date_range: Value,
	#[serde(default)]
	filters: Vec<Value>,
	#[serde(default)]
	format: Format,
}

#[derive(Deserialize, Default, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum Format {
	#[default]
	Impactite,
	Plausible,
	Goatcounter,
}

const KEEP: i64 = 86_400;

#[derive(Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
enum Progress {
	Running,
	Done,
	Failed,
}

struct Job {
	user: i64,
	site: String,
	name: String,
	kind: &'static str,
	state: Progress,
	done: f32,
	email: bool,
	at: i64,
}

static JOBS: LazyLock<Mutex<HashMap<String, Job>>> = LazyLock::new(Mutex::default);

fn file(data: &FilePath, id: &str) -> PathBuf {
	data.join("exports").join(id)
}

pub fn routes() -> Router<Arc<Web>> {
	Router::new()
		.route("/api/app/export", post(export))
		.route("/api/app/exports", get(latest))
		.route("/api/app/exports/{id}", get(status))
		.route("/api/app/exports/{id}/email", post(email))
		.route("/api/app/exports/{id}/file", get(download))
}

// big sites take minutes, so the file is built in the background and kept for a day
async fn export(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Json(body): Json<ExportRequest>,
) -> Reply {
	if web.app.site_role(viewer.user, &body.site_id)?.is_none() {
		return Err(Error::not_found("This site doesn't exist."));
	}
	let period =
		stats::period(&web.stats, &body.site_id, &body.date_range, false).map_err(Error::bad)?;
	let goals = web.app.goals(period.site.id)?;
	let filters = body
		.filters
		.iter()
		.map(|raw| stats::filters(raw, &goals))
		.collect::<Result<Vec<_>, _>>()
		.map_err(Error::bad)?
		.into_iter()
		.flatten()
		.map(|(filter, _)| filter)
		.collect::<Vec<_>>();
	let stem = format!("{}_{}_{}", period.site.domain, period.from, period.to);
	let (name, kind) = match body.format {
		Format::Goatcounter => (format!("{stem}.goatcounter.csv.gz"), "application/gzip"),
		Format::Plausible => (format!("{stem}.plausible.zip"), "application/zip"),
		Format::Impactite => (format!("{stem}.impactite.zip"), "application/zip"),
	};
	{
		let jobs = lock(&JOBS);
		if let Some((id, _)) = jobs.iter().find(|(_, job)| {
			job.user == viewer.user && job.name == name && job.state == Progress::Running
		}) {
			return Ok(Json(json!({ "id": id })));
		}
	}
	let id = token()?;
	let path = file(&web.data, &id);
	fs::create_dir_all(path.parent().unwrap_or(&web.data))?;
	lock(&JOBS).insert(
		id.clone(),
		Job {
			user: viewer.user,
			site: period.site.domain.clone(),
			name,
			kind,
			state: Progress::Running,
			done: 0.0,
			email: false,
			at: now(),
		},
	);
	let db = web.stats.db.clone();
	let titles = web.app.titles(period.site.id).unwrap_or_default();
	let format = body.format;
	let job = id.clone();
	tokio::spawn(async move {
		let tracked = job.clone();
		let built = tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
			let progress = |done: f32| {
				if let Some(entry) = lock(&JOBS).get_mut(&tracked) {
					entry.done = done;
				}
			};
			let out = BufWriter::new(fs::File::create(&path)?);
			let out = match format {
				Format::Plausible => build(
					out,
					&progress,
					&db,
					period.site.id,
					period.from,
					period.to,
					filters,
				)?,
				Format::Impactite => native(
					out,
					&progress,
					&db,
					&period.site,
					&goals,
					&titles,
					day_number(period.from)..=day_number(period.to),
				)?,
				Format::Goatcounter => goatcounter(
					out,
					&progress,
					&db,
					&period.site,
					&titles,
					day_number(period.from)..=day_number(period.to),
				)?,
			};
			out.into_inner()?.sync_all()?;
			Ok(())
		})
		.await
		.map_err(anyhow::Error::from)
		.and_then(|built| built);
		if let Err(err) = &built {
			tracing::error!(%err, "export");
			let _ = fs::remove_file(file(&web.data, &job));
		}
		let (user, site, email) = {
			let mut jobs = lock(&JOBS);
			let Some(entry) = jobs.get_mut(&job) else {
				return;
			};
			entry.state = if built.is_ok() {
				Progress::Done
			} else {
				Progress::Failed
			};
			(entry.user, entry.site.clone(), entry.email)
		};
		if email && let Err(err) = tell(&web, user, &site, &job, built.is_ok()).await {
			tracing::warn!(%err, "export mail");
		}
	});
	Ok(Json(json!({ "id": id })))
}

async fn tell(web: &Web, user: i64, site: &str, id: &str, done: bool) -> anyhow::Result<()> {
	let base = web.url.trim_end_matches('/');
	let link = format!("{base}/{site}/settings/data?export={id}");
	let (title, line, button) = if done {
		(
			"Your export is ready",
			format!("Your export of {site} is ready to download. It's available for 24 hours."),
			Some(("Download export", link.as_str())),
		)
	} else {
		(
			"Your export didn't finish",
			format!(
				"Something went wrong while exporting {site}. Try again from your site settings."
			),
			None,
		)
	};
	let lines = [line];
	let letter = Letter {
		base,
		title,
		lines: &lines,
		table: None,
		button,
	};
	let to = web.app.user_email(user)?;
	web.mail
		.send(&to, title, &letter.text(), &letter.html())
		.await
}

fn describe(web: &Web, id: &str, job: &Job) -> Value {
	json!({ "id": id, "state": job.state, "name": job.name, "done": job.done, "mail": web.mail.ready(), "email": job.email })
}

#[derive(Deserialize)]
struct Latest {
	site_id: String,
}

async fn latest(State(web): State<Arc<Web>>, viewer: Viewer, Query(ask): Query<Latest>) -> Reply {
	let jobs = lock(&JOBS);
	let job = jobs
		.iter()
		.filter(|(_, job)| job.user == viewer.user && job.site == ask.site_id)
		.max_by_key(|(_, job)| job.at)
		.map(|(id, job)| describe(&web, id, job));
	Ok(Json(job.unwrap_or(Value::Null)))
}

fn mine(user: i64, id: &str) -> Reply<(Progress, String, &'static str)> {
	lock(&JOBS)
		.get(id)
		.filter(|job| job.user == user)
		.map(|job| (job.state, job.name.clone(), job.kind))
		.ok_or_else(|| Error::not_found("This export expired. Start a new one."))
}

async fn status(State(web): State<Arc<Web>>, viewer: Viewer, Path(id): Path<String>) -> Reply {
	mine(viewer.user, &id)?;
	let jobs = lock(&JOBS);
	let job = jobs
		.get(&id)
		.ok_or_else(|| Error::not_found("This export expired. Start a new one."))?;
	Ok(Json(describe(&web, &id, job)))
}

async fn email(State(web): State<Arc<Web>>, viewer: Viewer, Path(id): Path<String>) -> Reply {
	if !web.mail.ready() {
		return Err(Error::bad("Email isn't set up on this server."));
	}
	mine(viewer.user, &id)?;
	let mut jobs = lock(&JOBS);
	let job = jobs
		.get_mut(&id)
		.ok_or_else(|| Error::not_found("This export expired. Start a new one."))?;
	job.email = job.state == Progress::Running;
	Ok(Json(json!({ "state": job.state })))
}

async fn download(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path(id): Path<String>,
) -> Reply<Response> {
	let (state, name, kind) = mine(viewer.user, &id)?;
	if state != Progress::Done {
		return Err(Error::not_found("This export isn't ready yet."));
	}
	let mut source = fs::File::open(file(&web.data, &id))?;
	let size = source.metadata()?.len();
	let (send, receive) = tokio::sync::mpsc::channel::<std::io::Result<Bytes>>(4);
	tokio::task::spawn_blocking(move || {
		let mut buf = vec![0; 256 << 10];
		loop {
			let piece = match source.read(&mut buf) {
				Ok(0) => return,
				Ok(n) => Ok(Bytes::copy_from_slice(&buf[..n])),
				Err(err) => Err(err),
			};
			if send.blocking_send(piece).is_err() {
				return;
			}
		}
	});
	let body = futures_util::stream::unfold(receive, |mut receive| async move {
		receive.recv().await.map(|piece| (piece, receive))
	});
	Ok((
		[
			(header::CONTENT_TYPE, kind.to_owned()),
			(header::CONTENT_LENGTH, size.to_string()),
			(
				header::CONTENT_DISPOSITION,
				format!("attachment; filename=\"{name}\""),
			),
		],
		Body::from_stream(body),
	)
		.into_response())
}

pub fn prune(data: &FilePath) {
	let cutoff = now() - KEEP;
	let mut jobs = lock(&JOBS);
	jobs.retain(|_, job| job.state == Progress::Running || job.at > cutoff);
	let Ok(entries) = fs::read_dir(data.join("exports")) else {
		return;
	};
	for entry in entries.flatten() {
		let name = entry.file_name();
		if !jobs.contains_key(&*name.to_string_lossy()) {
			let _ = fs::remove_file(entry.path());
		}
	}
}

fn build<W: Write + Seek>(
	out: W,
	progress: &dyn Fn(f32),
	db: &Db,
	site: u64,
	from: Date,
	to: Date,
	filters: Vec<Filter>,
) -> anyhow::Result<W> {
	let query = |breakdown: Option<Field>| -> anyhow::Result<Report> {
		Ok(db.query(&crater::Query {
			site,
			days: day_number(from)..=day_number(to),
			filters: filters.clone(),
			breakdown,
			limit: usize::MAX,
			daily: true,
			imports: false,
			since: None,
		})?)
	};
	let date = |offset: usize| {
		from.checked_add((offset as i64).days())
			.map(|day| day.to_string())
			.unwrap_or_default()
	};
	let stamp = format!("{}_{}", from.strftime("%Y%m%d"), to.strftime("%Y%m%d"));
	let mut zip = ZipWriter::new(out);
	let mut written = 0;
	let mut add = |table: &str, header: &[&str], rows: Vec<Vec<String>>| -> anyhow::Result<()> {
		zip.start_file(
			format!("imported_{table}_{stamp}.csv"),
			SimpleFileOptions::default(),
		)?;
		let mut csv = csv::Writer::from_writer(Vec::new());
		csv.write_record(header)?;
		for row in rows {
			csv.write_record(row.into_iter().map(cell))?;
		}
		zip.write_all(&csv.into_inner()?)?;
		written += 1;
		// the plausible archive has ten tables
		progress(written as f32 / 10.0);
		Ok(())
	};
	let n = |value: u64| value.to_string();
	let daily = |field: Field| -> anyhow::Result<Vec<(String, String, Metrics)>> {
		Ok(query(Some(field))?
			.daily
			.into_iter()
			.enumerate()
			.flat_map(|(offset, rows)| {
				rows.into_iter()
					.map(move |(value, metrics)| (date(offset), value.to_string(), metrics))
			})
			.collect())
	};
	let visit = |m: &Metrics| {
		[
			n(m.visitors),
			n(m.visits),
			n(m.duration),
			n(m.bounces),
			n(m.pageviews),
		]
	};

	let totals = query(None)?;
	add(
		"visitors",
		&[
			"date",
			"visitors",
			"pageviews",
			"bounces",
			"visits",
			"visit_duration",
		],
		totals
			.days
			.iter()
			.enumerate()
			.filter(|(_, m)| m.visits > 0)
			.map(|(offset, m)| {
				vec![
					date(offset),
					n(m.visitors),
					n(m.pageviews),
					n(m.bounces),
					n(m.visits),
					n(m.duration),
				]
			})
			.collect(),
	)?;
	add(
		"sources",
		&[
			"date",
			"source",
			"referrer",
			"utm_source",
			"utm_medium",
			"utm_campaign",
			"utm_content",
			"utm_term",
			"pageviews",
			"visitors",
			"visits",
			"visit_duration",
			"bounces",
		],
		daily(Field::Dim(Dim::Source))?
			.into_iter()
			.map(|(day, value, m)| {
				vec![
					day,
					value,
					String::new(),
					String::new(),
					String::new(),
					String::new(),
					String::new(),
					String::new(),
					n(m.pageviews),
					n(m.visitors),
					n(m.visits),
					n(m.duration),
					n(m.bounces),
				]
			})
			.collect(),
	)?;
	add(
		"pages",
		&[
			"date",
			"hostname",
			"page",
			"visits",
			"visitors",
			"pageviews",
			"total_scroll_depth",
			"total_scroll_depth_visits",
			"total_time_on_page",
			"total_time_on_page_visits",
		],
		daily(Field::Dim(Dim::Page))?
			.into_iter()
			.map(|(day, value, m)| {
				vec![
					day,
					String::new(),
					value,
					n(m.visits),
					n(m.visitors),
					n(m.pageviews),
					n(m.scroll),
					n(m.scrolled),
					n(m.engaged / 1000),
					n(m.engaged_views),
				]
			})
			.collect(),
	)?;
	add(
		"entry_pages",
		&[
			"date",
			"entry_page",
			"visitors",
			"entrances",
			"visit_duration",
			"bounces",
			"pageviews",
		],
		daily(Field::Entry)?
			.into_iter()
			.map(|(day, value, m)| {
				vec![
					day,
					value,
					n(m.visitors),
					n(m.visits),
					n(m.duration),
					n(m.bounces),
					n(m.pageviews),
				]
			})
			.collect(),
	)?;
	add(
		"exit_pages",
		&[
			"date",
			"exit_page",
			"visitors",
			"visit_duration",
			"exits",
			"bounces",
			"pageviews",
		],
		daily(Field::Exit)?
			.into_iter()
			.map(|(day, value, m)| {
				vec![
					day,
					value,
					n(m.visitors),
					n(m.duration),
					n(m.visits),
					n(m.bounces),
					n(m.pageviews),
				]
			})
			.collect(),
	)?;
	add(
		"custom_events",
		&["date", "name", "link_url", "path", "visitors", "events"],
		daily(Field::Dim(Dim::Name))?
			.into_iter()
			.filter(|(_, value, _)| value != "pageview" && value != "engagement")
			.map(|(day, value, m)| {
				vec![
					day,
					value,
					String::new(),
					String::new(),
					n(m.visitors),
					n(m.events),
				]
			})
			.collect(),
	)?;
	for (table, field, header) in [
		(
			"locations",
			Dim::Country,
			["date", "country", "region", "city"].as_slice(),
		),
		("devices", Dim::Device, ["date", "device"].as_slice()),
		(
			"browsers",
			Dim::Browser,
			["date", "browser", "browser_version"].as_slice(),
		),
		(
			"operating_systems",
			Dim::Os,
			["date", "operating_system", "operating_system_version"].as_slice(),
		),
	] {
		let mut columns = header.to_vec();
		columns.extend([
			"visitors",
			"visits",
			"visit_duration",
			"bounces",
			"pageviews",
		]);
		let rows = daily(Field::Dim(field))?
			.into_iter()
			.map(|(day, value, m)| {
				let mut row = vec![day, value];
				row.extend((2..header.len()).map(|i| {
					if header[i] == "city" {
						"0".to_owned()
					} else {
						String::new()
					}
				}));
				row.extend(visit(&m));
				row
			})
			.collect();
		add(table, &columns, rows)?;
	}
	Ok(zip.finish()?)
}

// a spreadsheet runs a cell that starts like a formula
fn cell(value: String) -> String {
	if value.starts_with(['=', '+', '-', '@', '\t', '\r']) {
		format!("'{value}")
	} else {
		value
	}
}

fn utc(local: i64, tz: &TimeZone) -> anyhow::Result<Timestamp> {
	let civil = Timestamp::from_second(local)?
		.to_zoned(TimeZone::UTC)
		.datetime();
	Ok(civil.to_zoned(tz.clone())?.timestamp())
}

pub(crate) fn day_archive(web: &Web, site: &Site, day: Date) -> anyhow::Result<Vec<u8>> {
	let goals = web.app.goals(site.id)?;
	let titles = web.app.titles(site.id)?;
	Ok(native(
		Cursor::new(Vec::new()),
		&|_| {},
		&web.stats.db,
		site,
		&goals,
		&titles,
		day_number(day)..=day_number(day),
	)?
	.into_inner())
}

fn native<W: Write + Seek>(
	out: W,
	progress: &dyn Fn(f32),
	db: &Db,
	site: &Site,
	goals: &[Goal],
	titles: &HashMap<String, String>,
	days: RangeInclusive<i32>,
) -> anyhow::Result<W> {
	let mut zip = ZipWriter::new(out);
	zip.start_file("site.json", SimpleFileOptions::default())?;
	serde_json::to_writer_pretty(
		&mut zip,
		&json!({
			"format": "impactite",
			"version": 1,
			"domain": site.domain,
			"timezone": site.timezone,
			"goals": goals.iter().map(|goal| json!({ "page": goal.page, "value": goal.value, "name": goal.name, "scroll": goal.scroll, "currency": goal.currency })).collect::<Vec<_>>(),
			"titles": titles,
		}),
	)?;
	zip.start_file("events.ndjson", SimpleFileOptions::default())?;
	let mut failed = None;
	let mut each = |event: Event| {
		let mut line = Map::new();
		let local = Timestamp::from_second(event.ts)
			.map(|ts| ts.to_zoned(TimeZone::UTC).datetime().to_string())
			.unwrap_or_default();
		line.insert("t".into(), json!(local));
		line.insert("v".into(), json!(format!("{:x}", event.visitor)));
		line.insert("s".into(), json!(format!("{:x}", event.session)));
		if !event.interactive {
			line.insert("i".into(), json!(false));
		}
		if let Some(depth) = event.scroll_depth {
			line.insert("sd".into(), json!(depth));
		}
		if event.engagement_ms > 0 {
			line.insert("e".into(), json!(event.engagement_ms));
		}
		for dim in Dim::ALL {
			if !event.dims[dim].is_empty() {
				line.insert(dim.name().into(), json!(event.dims[dim]));
			}
		}
		if let Some((amount, currency)) = &event.revenue {
			line.insert("r".into(), json!([amount, currency]));
		}
		if !event.props.is_empty() {
			let props: Map<String, Value> = event
				.props
				.into_iter()
				.map(|(key, value)| (key, json!(value)))
				.collect();
			line.insert("p".into(), Value::Object(props));
		}
		if let Err(err) = writeln!(zip, "{}", Value::Object(line)) {
			failed.get_or_insert(err);
		}
	};
	let (first, last) = days.into_inner();
	for day in first..=last {
		db.events(site.id, day..=day, &mut each)?;
		progress((day - first + 1) as f32 / (last - first + 1) as f32);
	}
	if let Some(err) = failed {
		return Err(err.into());
	}
	Ok(zip.finish()?)
}

pub(crate) fn goatcounter<W: Write>(
	out: W,
	progress: &dyn Fn(f32),
	db: &Db,
	site: &Site,
	titles: &HashMap<String, String>,
	days: RangeInclusive<i32>,
) -> anyhow::Result<W> {
	let tz = TimeZone::get(&site.timezone).unwrap_or(TimeZone::UTC);
	let mut csv = csv::Writer::from_writer(GzEncoder::new(out, Compression::default()));
	csv.write_record([
		"2Path",
		"Title",
		"Event",
		"UserAgent",
		"Browser",
		"System",
		"Session",
		"Bot",
		"Referrer",
		"Referrer scheme",
		"Screen size",
		"Location",
		"FirstVisit",
		"Date",
	])?;
	let mut seen = HashSet::new();
	let mut failed = None;
	let mut each = |event: Event| {
		let name = &event.dims[Dim::Name];
		if name == "engagement" || name.starts_with('~') {
			return;
		}
		let custom = name != "pageview";
		let joined = |a: &str, b: &str| format!("{a} {b}").trim().to_owned();
		let referrer = event.dims[Dim::Referrer]
			.trim_start_matches("https://")
			.trim_start_matches("http://")
			.to_owned();
		let (reference, scheme) = match (referrer.is_empty(), event.dims[Dim::UtmSource].is_empty())
		{
			(false, _) => (referrer, "h"),
			(true, false) => (event.dims[Dim::UtmSource].clone(), "c"),
			(true, true) => (String::new(), ""),
		};
		// we keep device classes, not widths, so each class gets a typical width
		let width = match event.dims[Dim::Device].as_str() {
			"Mobile" => "390",
			"Tablet" => "820",
			"Desktop" | "Laptop" => "1920",
			_ => "0",
		};
		let location = if event.dims[Dim::Region].is_empty() {
			event.dims[Dim::Country].clone()
		} else {
			event.dims[Dim::Region].clone()
		};
		let date = utc(event.ts, &tz)
			.map(|ts| ts.to_string())
			.unwrap_or_default();
		let row = [
			if custom {
				name.clone()
			} else {
				event.dims[Dim::Page].clone()
			},
			if custom {
				String::new()
			} else {
				titles
					.get(&event.dims[Dim::Page])
					.cloned()
					.unwrap_or_default()
			},
			custom.to_string(),
			String::new(),
			joined(&event.dims[Dim::Browser], &event.dims[Dim::BrowserVersion]),
			joined(&event.dims[Dim::Os], &event.dims[Dim::OsVersion]),
			format!("{:x}", event.session),
			"0".into(),
			reference,
			scheme.into(),
			format!("{width},0,1"),
			location,
			seen.insert(event.visitor).to_string(),
			date,
		];
		if let Err(err) = csv.write_record(row.map(cell)) {
			failed.get_or_insert(err);
		}
	};
	let (first, last) = days.into_inner();
	for day in first..=last {
		db.events(site.id, day..=day, &mut each)?;
		progress((day - first + 1) as f32 / (last - first + 1) as f32);
	}
	if let Some(err) = failed {
		return Err(err.into());
	}
	Ok(csv
		.into_inner()
		.map_err(csv::IntoInnerError::into_error)?
		.finish()?)
}
