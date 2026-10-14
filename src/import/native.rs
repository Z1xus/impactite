use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use anyhow::{Context, bail};
use crater::{Db, Dim, Event};
use jiff::civil::DateTime;
use jiff::tz::TimeZone;
use serde_json::{Map, Value};
use zip::ZipArchive;

use super::insert;
use crate::app::{App, Site};

pub fn import(db: &Db, app: &App, site: &Site, path: &Path) -> anyhow::Result<(u64, Option<i64>)> {
	let mut archive = ZipArchive::new(File::open(path).context("open export")?)?;
	let meta: Value = serde_json::from_reader(archive.by_name("site.json")?)?;
	if meta["format"] != "impactite" {
		bail!("This is not an impactite export.");
	}
	for goal in meta["goals"].as_array().into_iter().flatten() {
		let page = goal["page"].as_bool().unwrap_or(false);
		let value = goal["value"].as_str().unwrap_or_default();
		let name = match page {
			true => format!("Visit {value}"),
			false => value.to_owned(),
		};
		// goals that already exist are kept as they are
		let _ = app.add_goal(site.id, page, value, &name);
	}
	let source = meta["timezone"]
		.as_str()
		.and_then(|zone| TimeZone::get(zone).ok())
		.unwrap_or(TimeZone::UTC);
	let target = TimeZone::get(&site.timezone).unwrap_or(TimeZone::UTC);
	let (mut count, mut first) = (0, None);
	for line in BufReader::new(archive.by_name("events.ndjson")?).lines() {
		let line: Map<String, Value> = serde_json::from_str(&line?)?;
		let text = |key: &str| line.get(key).and_then(Value::as_str).unwrap_or_default();
		let id = |key: &str| u64::from_str_radix(text(key), 16).context("bad id");
		let local: DateTime = text("t").parse().context("bad time")?;
		let ts = local.to_zoned(source.clone())?.timestamp();
		first = Some(first.unwrap_or(ts.as_second()).min(ts.as_second()));
		let mut event = Event {
			site: site.id,
			ts: ts.as_second(),
			offset: target.to_offset(ts).seconds(),
			visitor: id("v")?,
			session: id("s")?,
			interactive: line.get("i") != Some(&Value::Bool(false)),
			scroll_depth: line
				.get("sd")
				.and_then(Value::as_u64)
				.map(|depth| depth.min(100) as u8),
			engagement_ms: line.get("e").and_then(Value::as_u64).unwrap_or(0) as u32,
			props: line
				.get("p")
				.and_then(Value::as_object)
				.into_iter()
				.flatten()
				.map(|(key, value)| (key.clone(), value.as_str().unwrap_or_default().to_owned()))
				.collect(),
			..Event::default()
		};
		for dim in Dim::ALL {
			event.dims[dim] = text(dim.name()).to_owned();
		}
		insert(db, event);
		count += 1;
	}
	Ok((count, first))
}
