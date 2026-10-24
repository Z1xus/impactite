use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use jiff::civil::Date;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::site::shares::Reader;
use crate::stats::{Stats, run};
use crate::web::{Error, Reply, Web};

const DRIVERS: [&str; 2] = ["visit:source", "event:page"];

pub fn routes() -> Router<Arc<Web>> {
	Router::new().route("/api/app/insights", post(insights))
}

#[derive(Deserialize)]
struct Ask {
	site_id: String,
	date_range: Value,
	previous: Option<Value>,
	#[serde(default)]
	filters: Vec<Value>,
	#[serde(default = "yes")]
	imports: bool,
}

fn yes() -> bool {
	true
}

async fn insights(State(web): State<Arc<Web>>, reader: Reader, Json(ask): Json<Ask>) -> Reply {
	if !reader.reads(&web, &ask.site_id) {
		return Err(Error::not_found("Site not found."));
	}
	let spikes = spikes(&web.stats, &ask).map_err(Error::bad)?;
	let change = match &ask.previous {
		Some(previous) => change(&web.stats, &ask, previous).map_err(Error::bad)?,
		None => Value::Null,
	};
	Ok(Json(json!({ "change": change, "spikes": spikes })))
}

fn visitors(
	stats: &Stats,
	ask: &Ask,
	range: &Value,
	dimension: Option<&str>,
) -> Result<HashMap<String, f64>, String> {
	let reply = run(
		stats,
		json!({
			"site_id": ask.site_id,
			"metrics": ["visitors"],
			"date_range": range,
			"dimensions": dimension.into_iter().collect::<Vec<_>>(),
			"filters": ask.filters,
			"include": { "imports": ask.imports },
			"pagination": { "limit": 1000 },
		}),
	)?;
	Ok(reply["results"]
		.as_array()
		.into_iter()
		.flatten()
		.map(|row| {
			let key = match &row["dimensions"][0] {
				Value::String(text) => text.clone(),
				Value::Null => String::new(),
				other => other.to_string(),
			};
			(key, row["metrics"][0].as_f64().unwrap_or_default())
		})
		.collect())
}

// the value that moved the most in the direction of the change, if it explains a good part of it
fn driver(
	now: &HashMap<String, f64>,
	before: &HashMap<String, f64>,
	scale: f64,
	moved: f64,
) -> Option<(String, f64)> {
	let mut keys: Vec<&String> = now.keys().chain(before.keys()).collect();
	keys.sort();
	keys.dedup();
	keys.into_iter()
		.map(|key| {
			let delta = now.get(key).copied().unwrap_or_default()
				- before.get(key).copied().unwrap_or_default() * scale;
			(key.clone(), delta)
		})
		.filter(|(key, delta)| !key.is_empty() && delta * moved.signum() >= moved.abs() * 0.3)
		.max_by(|a, b| (a.1 * moved.signum()).total_cmp(&(b.1 * moved.signum())))
}

fn drivers(
	stats: &Stats,
	ask: &Ask,
	now: &Value,
	before: &Value,
	scale: f64,
	moved: f64,
) -> Result<Vec<Value>, String> {
	let mut found = Vec::new();
	for dimension in DRIVERS {
		let current = visitors(stats, ask, now, Some(dimension))?;
		let previous = visitors(stats, ask, before, Some(dimension))?;
		if let Some((value, delta)) = driver(&current, &previous, scale, moved) {
			found.push(json!({ "dimension": dimension, "value": value, "delta": delta.round() }));
		}
	}
	Ok(found)
}

fn change(stats: &Stats, ask: &Ask, previous: &Value) -> Result<Value, String> {
	let total = |range: &Value| -> Result<f64, String> {
		Ok(visitors(stats, ask, range, None)?
			.into_values()
			.next()
			.unwrap_or_default())
	};
	let (now, before) = (total(&ask.date_range)?, total(previous)?);
	let moved = now - before;
	if before < 1.0 || moved.abs() < 10.0 || (moved / before).abs() < 0.1 {
		return Ok(Value::Null);
	}
	Ok(json!({
		"percent": (moved / before * 100.0).round(),
		"drivers": drivers(stats, ask, &ask.date_range, previous, 1.0, moved)?,
	}))
}

fn spikes(stats: &Stats, ask: &Ask) -> Result<Vec<Value>, String> {
	if ask.date_range == json!("30m") || ask.date_range == json!("day") {
		return Ok(Vec::new());
	}
	let mut days: Vec<(Date, f64)> = visitors(stats, ask, &ask.date_range, Some("time:day"))?
		.into_iter()
		.filter_map(|(day, count)| Some((day.parse().ok()?, count)))
		.collect();
	days.sort_by_key(|&(day, _)| day);
	if days.len() < 7 {
		return Ok(Vec::new());
	}
	let mut sorted: Vec<f64> = days.iter().map(|&(_, count)| count).collect();
	sorted.sort_by(f64::total_cmp);
	let median = sorted[sorted.len() / 2];
	let spiking = |count: f64| count >= median * 2.0 && count - median >= 20.0;

	// a run of high days counts once, at its peak
	let mut peaks: Vec<(Date, f64)> = Vec::new();
	let mut previous: Option<Date> = None;
	for &(day, count) in days.iter().filter(|&&(_, count)| spiking(count)) {
		match peaks.last_mut() {
			Some(peak) if previous.is_some_and(|last| last.tomorrow().ok() == Some(day)) => {
				if count > peak.1 {
					*peak = (day, count);
				}
			}
			_ => peaks.push((day, count)),
		}
		previous = Some(day);
	}
	peaks.sort_by(|a, b| b.1.total_cmp(&a.1));
	peaks.truncate(3);

	let scale = 1.0 / days.len() as f64;
	peaks
		.into_iter()
		.map(|(day, count)| {
			let on = json!([day.to_string(), day.to_string()]);
			Ok(json!({
				"day": day.to_string(),
				"excess": (count - median).round(),
				"drivers": drivers(stats, ask, &on, &ask.date_range, scale, count - median)?,
			}))
		})
		.collect()
}
