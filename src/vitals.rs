use std::collections::BTreeMap;
use std::sync::Arc;
use std::thread;

use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::site::shares::Reader;
use crate::stats::{Stats, run};
use crate::web::{Error, Reply, Web};

pub const EVENT: &str = "~vitals";

struct Vital {
	key: &'static str,
	good: f64,
	poor: f64,
	// buckets of `step` below each bound keep a value's precision where it matters
	steps: &'static [(f64, f64)],
}

const TIMES: &[(f64, f64)] = &[(1000.0, 50.0), (5000.0, 100.0), (10000.0, 500.0)];

const VITALS: [Vital; 5] = [
	Vital {
		key: "lcp",
		good: 2500.0,
		poor: 4000.0,
		steps: TIMES,
	},
	Vital {
		key: "inp",
		good: 200.0,
		poor: 500.0,
		steps: &[(500.0, 10.0), (1000.0, 50.0), (2000.0, 100.0)],
	},
	Vital {
		key: "cls",
		good: 0.1,
		poor: 0.25,
		steps: &[(0.25, 0.01), (1.0, 0.05)],
	},
	Vital {
		key: "fcp",
		good: 1800.0,
		poor: 3000.0,
		steps: TIMES,
	},
	Vital {
		key: "ttfb",
		good: 800.0,
		poor: 1800.0,
		steps: TIMES,
	},
];

impl Vital {
	fn bucket(&self, value: f64) -> String {
		let cap = self.steps.last().map_or(0.0, |&(bound, _)| bound);
		let value = match self.steps.iter().find(|&&(bound, _)| value < bound) {
			Some(&(_, step)) => (value / step).floor() * step,
			None => cap,
		};
		if self.key == "cls" {
			format!("{value:.2}")
		} else {
			format!("{value:.0}")
		}
	}

	fn rating(&self, value: f64) -> &'static str {
		if value < self.good {
			"good"
		} else if value < self.poor {
			"ni"
		} else {
			"poor"
		}
	}

	fn prop(&self) -> String {
		format!("event:props:~{}", self.key)
	}
}

pub fn props(raw: &Value) -> Vec<(String, String)> {
	VITALS
		.iter()
		.filter_map(|vital| {
			let value = raw[vital.key]
				.as_f64()
				.filter(|value| value.is_finite() && *value >= 0.0)?;
			Some((format!("~{}", vital.key), vital.bucket(value)))
		})
		.collect()
}

pub fn routes() -> Router<Arc<Web>> {
	Router::new()
		.route("/api/app/vitals", post(overview))
		.route("/api/app/vitals/pages", post(pages))
		.route("/api/app/vitals/trend", post(trend))
}

#[derive(Deserialize)]
struct Ask {
	site_id: String,
	date_range: Value,
	#[serde(default)]
	filters: Vec<Value>,
	metric: Option<String>,
	grain: Option<String>,
}

impl Ask {
	fn vital(&self) -> Result<&'static Vital, String> {
		VITALS
			.iter()
			.find(|vital| Some(vital.key) == self.metric.as_deref())
			.ok_or_else(|| "Pick lcp, inp, cls, fcp or ttfb.".to_owned())
	}
}

fn histogram(
	stats: &Stats,
	ask: &Ask,
	vital: &Vital,
	page: Option<&str>,
) -> Result<Vec<(f64, f64)>, String> {
	let mut filters = ask.filters.clone();
	filters.push(json!(["is", "event:name", [EVENT]]));
	if let Some(page) = page {
		filters.push(json!(["is", "event:page", [page]]));
	}
	let reply = run(
		stats,
		json!({
			"site_id": ask.site_id,
			"metrics": ["events"],
			"date_range": ask.date_range,
			"dimensions": [vital.prop()],
			"filters": filters,
			"include": { "imports": false },
			"pagination": { "limit": 1000 },
		}),
	)?;
	let mut bins: Vec<(f64, f64)> = reply["results"]
		.as_array()
		.into_iter()
		.flatten()
		.filter_map(|row| {
			Some((
				row["dimensions"][0].as_str()?.parse().ok()?,
				row["metrics"][0].as_f64()?,
			))
		})
		.collect();
	bins.sort_by(|a, b| a.0.total_cmp(&b.0));
	Ok(bins)
}

// bins are sorted by value
fn p75(bins: &[(f64, f64)], samples: f64) -> f64 {
	let mut seen = 0.0;
	bins.iter()
		.find(|&&(_, count)| {
			seen += count;
			seen >= samples * 0.75
		})
		.map_or(0.0, |&(value, _)| value)
}

fn summary(vital: &Vital, bins: &[(f64, f64)]) -> Value {
	let samples = bins.iter().fold(0.0, |sum, &(_, count)| sum + count);
	if samples == 0.0 {
		return json!({ "key": vital.key, "samples": 0 });
	}
	let p75 = p75(bins, samples);
	let share = |rating: &str| {
		bins.iter()
			.filter(|&&(value, _)| vital.rating(value) == rating)
			.fold(0.0, |sum, &(_, count)| sum + count)
			/ samples
	};
	json!({
		"key": vital.key,
		"samples": samples,
		"p75": p75,
		"rating": vital.rating(p75),
		"shares": [share("good"), share("ni"), share("poor")],
	})
}

// each question scans on its own, so they run side by side
fn parallel<F>(work: impl Iterator<Item = F>) -> Result<Vec<Value>, String>
where
	F: FnOnce() -> Result<Value, String> + Send,
{
	thread::scope(|scope| {
		let running: Vec<_> = work.map(|job| scope.spawn(job)).collect();
		running
			.into_iter()
			.map(|job| job.join().map_err(|_| "a query failed".to_owned())?)
			.collect()
	})
}

async fn overview(State(web): State<Arc<Web>>, reader: Reader, Json(raw): Json<Value>) -> Reply {
	if !reader.reads(&web, raw["site_id"].as_str().unwrap_or_default()) {
		return Err(Error::not_found("This site doesn't exist."));
	}
	Ok(Json(measure(&web.stats, raw).map_err(Error::bad)?))
}

async fn pages(State(web): State<Arc<Web>>, reader: Reader, Json(raw): Json<Value>) -> Reply {
	if !reader.reads(&web, raw["site_id"].as_str().unwrap_or_default()) {
		return Err(Error::not_found("This site doesn't exist."));
	}
	Ok(Json(slowest(&web.stats, raw).map_err(Error::bad)?))
}

async fn trend(State(web): State<Arc<Web>>, reader: Reader, Json(raw): Json<Value>) -> Reply {
	if !reader.reads(&web, raw["site_id"].as_str().unwrap_or_default()) {
		return Err(Error::not_found("This site doesn't exist."));
	}
	Ok(Json(over_time(&web.stats, raw).map_err(Error::bad)?))
}

pub fn measure(stats: &Stats, raw: Value) -> Result<Value, String> {
	let ask: Ask = serde_json::from_value(raw).map_err(|err| format!("#: {err}"))?;
	let vitals = parallel(VITALS.iter().map(|vital| {
		let ask = &ask;
		move || Ok(summary(vital, &histogram(stats, ask, vital, None)?))
	}))?;
	Ok(json!({ "vitals": vitals }))
}

pub fn slowest(stats: &Stats, raw: Value) -> Result<Value, String> {
	let ask: Ask = serde_json::from_value(raw).map_err(|err| format!("#: {err}"))?;
	let vital = ask.vital()?;
	let mut filters = ask.filters.clone();
	filters.push(json!(["is", "event:name", [EVENT]]));
	let top = run(
		stats,
		json!({
			"site_id": ask.site_id,
			"metrics": ["events"],
			"date_range": ask.date_range,
			"dimensions": ["event:page"],
			"filters": filters,
			"include": { "imports": false },
			"pagination": { "limit": 9 },
		}),
	)?;
	let mut pages = parallel(
		top["results"]
			.as_array()
			.into_iter()
			.flatten()
			.filter_map(|row| row["dimensions"][0].as_str())
			.map(|page| {
				let ask = &ask;
				move || {
					let mut line = summary(vital, &histogram(stats, ask, vital, Some(page))?);
					line["page"] = json!(page);
					Ok(line)
				}
			}),
	)?;
	pages.sort_by(|a, b| {
		let p75 = |line: &Value| line["p75"].as_f64().unwrap_or_default();
		p75(b).total_cmp(&p75(a))
	});
	Ok(json!({ "pages": pages }))
}

pub fn over_time(stats: &Stats, raw: Value) -> Result<Value, String> {
	let ask: Ask = serde_json::from_value(raw).map_err(|err| format!("#: {err}"))?;
	let vital = ask.vital()?;
	let mut filters = ask.filters.clone();
	filters.push(json!(["is", "event:name", [EVENT]]));
	let reply = run(
		stats,
		json!({
			"site_id": ask.site_id,
			"metrics": ["events"],
			"date_range": ask.date_range,
			"dimensions": [ask.grain.as_deref().unwrap_or("time:day"), vital.prop()],
			"filters": filters,
			"include": { "imports": false },
			"pagination": { "limit": 10000 },
		}),
	)?;
	let mut buckets: BTreeMap<&str, Vec<(f64, f64)>> = BTreeMap::new();
	for row in reply["results"].as_array().into_iter().flatten() {
		let (Some(time), Some(value), Some(count)) = (
			row["dimensions"][0].as_str(),
			row["dimensions"][1]
				.as_str()
				.and_then(|value| value.parse().ok()),
			row["metrics"][0].as_f64(),
		) else {
			continue;
		};
		buckets.entry(time).or_default().push((value, count));
	}
	let trend: Vec<Value> = buckets
		.into_iter()
		.map(|(time, mut bins)| {
			bins.sort_by(|a, b| a.0.total_cmp(&b.0));
			let samples = bins.iter().fold(0.0, |sum, &(_, count)| sum + count);
			json!({ "time": time, "p75": p75(&bins, samples), "samples": samples })
		})
		.collect();
	Ok(json!({ "trend": trend, "good": vital.good, "poor": vital.poor }))
}
