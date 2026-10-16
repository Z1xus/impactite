use std::collections::{HashMap, HashSet};
use std::io::{Cursor, Write};
use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use crater::{Db, Dim, Field, Filter, Metrics, Query, Report};
use flate2::Compression;
use flate2::write::GzEncoder;
use jiff::civil::Date;
use jiff::tz::TimeZone;
use jiff::{Timestamp, ToSpan};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

use crate::accounts::{Viewer, Web, fail};
use crate::app::Site;
use crate::goals::Goal;
use crate::stats::{self, day_number};

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

pub async fn export(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Json(body): Json<ExportRequest>,
) -> Response {
	if !matches!(web.app.site_role(viewer.user, &body.site_id), Ok(Some(_))) {
		return fail(StatusCode::NOT_FOUND, "Site not found.");
	}
	let prepared =
		stats::period(&web.stats, &body.site_id, &body.date_range, false).and_then(|period| {
			let goals = web
				.app
				.goals(period.site.id)
				.map_err(|err| err.to_string())?;
			let filters = body
				.filters
				.iter()
				.map(|raw| stats::filters(raw, &goals))
				.collect::<Result<Vec<_>, _>>()?
				.into_iter()
				.flatten()
				.map(|(filter, _)| filter)
				.collect::<Vec<_>>();
			Ok((period, filters))
		});
	let (period, filters) = match prepared {
		Ok(prepared) => prepared,
		Err(message) => return fail(StatusCode::BAD_REQUEST, &message),
	};
	let stem = format!("{}_{}_{}", period.site.domain, period.from, period.to);
	let (name, kind) = match body.format {
		Format::Goatcounter => (format!("{stem}.goatcounter.csv.gz"), "application/gzip"),
		Format::Plausible => (format!("{stem}.plausible.zip"), "application/zip"),
		Format::Impactite => (format!("{stem}.impactite.zip"), "application/zip"),
	};
	let db = web.stats.db.clone();
	let goals = web.app.goals(period.site.id).unwrap_or_default();
	let titles = web.app.titles(period.site.id).unwrap_or_default();
	let format = body.format;
	let built = tokio::task::spawn_blocking(move || match format {
		Format::Plausible => build(&db, period.site.id, period.from, period.to, filters),
		Format::Impactite => native(&db, &period.site, &goals, &titles, period.from, period.to),
		Format::Goatcounter => goatcounter(&db, &period.site, &titles, period.from, period.to),
	})
	.await;
	match built {
		Ok(Ok(bytes)) => {
			let mut response = bytes.into_response();
			let headers = response.headers_mut();
			headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(kind));
			if let Ok(value) = HeaderValue::from_str(&format!("attachment; filename=\"{name}\"")) {
				headers.insert(header::CONTENT_DISPOSITION, value);
			}
			response
		}
		_ => fail(StatusCode::INTERNAL_SERVER_ERROR, "The export failed."),
	}
}

fn build(
	db: &Db,
	site: u64,
	from: Date,
	to: Date,
	filters: Vec<Filter>,
) -> anyhow::Result<Vec<u8>> {
	let query = |breakdown: Option<Field>| -> anyhow::Result<Report> {
		Ok(db.query(&Query {
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
	let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
	let mut add = |table: &str, header: &[&str], rows: Vec<Vec<String>>| -> anyhow::Result<()> {
		zip.start_file(
			format!("imported_{table}_{stamp}.csv"),
			SimpleFileOptions::default(),
		)?;
		let mut csv = csv::Writer::from_writer(Vec::new());
		csv.write_record(header)?;
		for row in rows {
			csv.write_record(row)?;
		}
		zip.write_all(&csv.into_inner()?)?;
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
	Ok(zip.finish()?.into_inner())
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
	native(&web.stats.db, site, &goals, &titles, day, day)
}

fn native(
	db: &Db,
	site: &Site,
	goals: &[Goal],
	titles: &HashMap<String, String>,
	from: Date,
	to: Date,
) -> anyhow::Result<Vec<u8>> {
	let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
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
	db.events(site.id, day_number(from)..=day_number(to), |event| {
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
	})?;
	if let Some(err) = failed {
		return Err(err.into());
	}
	Ok(zip.finish()?.into_inner())
}

pub(crate) fn goatcounter(
	db: &Db,
	site: &Site,
	titles: &HashMap<String, String>,
	from: Date,
	to: Date,
) -> anyhow::Result<Vec<u8>> {
	let tz = TimeZone::get(&site.timezone).unwrap_or(TimeZone::UTC);
	let mut csv = csv::Writer::from_writer(GzEncoder::new(Vec::new(), Compression::default()));
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
	db.events(site.id, day_number(from)..=day_number(to), |event| {
		let name = &event.dims[Dim::Name];
		if name == "engagement" {
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
		let location = match event.dims[Dim::Region].is_empty() {
			true => event.dims[Dim::Country].clone(),
			false => event.dims[Dim::Region].clone(),
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
			match custom {
				true => String::new(),
				false => titles
					.get(&event.dims[Dim::Page])
					.cloned()
					.unwrap_or_default(),
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
		if let Err(err) = csv.write_record(row) {
			failed.get_or_insert(err);
		}
	})?;
	if let Some(err) = failed {
		return Err(err.into());
	}
	Ok(csv.into_inner().map_err(|err| err.into_error())?.finish()?)
}
