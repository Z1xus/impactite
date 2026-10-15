use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use jiff::civil::Date;
use jiff::tz::TimeZone;
use jiff::{Timestamp, ToSpan};
use serde_json::{Map, Value, json};

use crate::accounts::Web;
use crate::stats::{error, keyed, run, unauthorized};

type Params = HashMap<String, String>;

pub fn routes() -> Router<Arc<Web>> {
	Router::new()
		.route("/api/v1/stats/realtime/visitors", get(realtime))
		.route("/api/v1/stats/aggregate", get(aggregate))
		.route("/api/v1/stats/timeseries", get(timeseries))
		.route("/api/v1/stats/breakdown", get(breakdown))
}

async fn realtime(
	State(web): State<Arc<Web>>,
	headers: HeaderMap,
	Query(params): Query<Params>,
) -> Response {
	respond(&web, &headers, &params, |web, params| {
		let site = web
			.app
			.site(site_id(params))
			.map_err(|err| err.to_string())?
			.ok_or("Site not found")?;
		Ok(json!(web.ingest.current_visitors(site.id)))
	})
}

async fn aggregate(
	State(web): State<Arc<Web>>,
	headers: HeaderMap,
	Query(params): Query<Params>,
) -> Response {
	respond(&web, &headers, &params, |web, params| {
		let (from, to) = range(web, params)?;
		let metrics = metrics(params);
		let now = ask(web, params, from, to, &[])?;
		let before = match params.get("compare").map(String::as_str) {
			Some("previous_period") => {
				let end = from.checked_sub(1.day()).map_err(|err| err.to_string())?;
				let start = end
					.checked_sub(from.until(to).map_err(|err| err.to_string())?)
					.map_err(|err| err.to_string())?;
				Some(ask(web, params, start, end, &[])?)
			}
			_ => None,
		};
		let mut results = Map::new();
		for (i, name) in metrics.iter().enumerate() {
			let value = &now["results"][0]["metrics"][i];
			let mut entry = json!({ "value": value });
			if let Some(before) = &before {
				entry["change"] = change(name, value, &before["results"][0]["metrics"][i]);
			}
			results.insert(name.clone(), entry);
		}
		Ok(json!({ "results": results }))
	})
}

async fn timeseries(
	State(web): State<Arc<Web>>,
	headers: HeaderMap,
	Query(params): Query<Params>,
) -> Response {
	respond(&web, &headers, &params, |web, params| {
		let (from, to) = range(web, params)?;
		let period = params.get("period").map_or("30d", String::as_str);
		let interval = match params.get("interval").map(String::as_str) {
			Some("hour") => "time:hour",
			Some("date") => "time:day",
			Some("week") => "time:week",
			Some("month") => "time:month",
			Some(other) => return Err(format!("Invalid value for interval: `{other}`")),
			None => match period {
				"day" => "time:hour",
				"6mo" | "12mo" => "time:month",
				_ => "time:day",
			},
		};
		let result = ask(web, params, from, to, &[interval])?;
		Ok(json!({ "results": rows(&result, &metrics(params), "date") }))
	})
}

async fn breakdown(
	State(web): State<Arc<Web>>,
	headers: HeaderMap,
	Query(params): Query<Params>,
) -> Response {
	respond(&web, &headers, &params, |web, params| {
		let (from, to) = range(web, params)?;
		let property = params
			.get("property")
			.ok_or("The `property` parameter is required for breakdown")?;
		let number = |name: &str, default: usize| {
			params
				.get(name)
				.map_or(Ok(default), |value| value.parse())
				.map_err(|_| format!("Invalid value for {name}"))
		};
		let limit = number("limit", 100)?.min(1000);
		let page = number("page", 1)?.max(1);
		let mut result = ask(web, params, from, to, &[property])?;
		let rows_wanted: Vec<Value> = result["results"]
			.as_array_mut()
			.map(std::mem::take)
			.unwrap_or_default()
			.into_iter()
			.skip((page - 1) * limit)
			.take(limit)
			.collect();
		result["results"] = json!(rows_wanted);
		let key = property.rsplit(':').next().unwrap_or(property);
		Ok(json!({ "results": rows(&result, &metrics(params), key) }))
	})
}

fn respond(
	web: &Web,
	headers: &HeaderMap,
	params: &Params,
	build: impl FnOnce(&Web, &Params) -> Result<Value, String>,
) -> Response {
	if !keyed(&web.app, headers, site_id(params)) {
		return unauthorized();
	}
	match build(web, params) {
		Ok(value) => Json(value).into_response(),
		Err(message) => error(StatusCode::BAD_REQUEST, &message),
	}
}

fn site_id(params: &Params) -> &str {
	params.get("site_id").map_or("", String::as_str)
}

fn metrics(params: &Params) -> Vec<String> {
	params
		.get("metrics")
		.map_or("visitors", String::as_str)
		.split(',')
		.map(|name| name.trim().to_owned())
		.collect()
}

fn ask(
	web: &Web,
	params: &Params,
	from: Date,
	to: Date,
	dimensions: &[&str],
) -> Result<Value, String> {
	run(
		&web.stats,
		json!({
			"site_id": site_id(params),
			"metrics": metrics(params),
			"date_range": [from.to_string(), to.to_string()],
			"dimensions": dimensions,
			"filters": filters(params)?,
			"include": { "imports": params.get("with_imported").is_some_and(|value| value == "true") },
		}),
	)
}

fn rows(result: &Value, metrics: &[String], key: &str) -> Vec<Value> {
	result["results"]
		.as_array()
		.into_iter()
		.flatten()
		.map(|row| {
			let mut entry = Map::new();
			entry.insert(key.to_owned(), row["dimensions"][0].clone());
			for (i, name) in metrics.iter().enumerate() {
				entry.insert(name.clone(), row["metrics"][i].clone());
			}
			Value::Object(entry)
		})
		.collect()
}

fn range(web: &Web, params: &Params) -> Result<(Date, Date), String> {
	let parse = |text: &str| {
		text.trim()
			.parse::<Date>()
			.map_err(|_| format!("Failed to parse date: `{text}`"))
	};
	let date = params.get("date");
	let period = params.get("period").map_or("30d", String::as_str);
	if period == "custom" {
		let (from, to) = date
			.and_then(|date| date.split_once(','))
			.ok_or("The `date` parameter must be `start,end` for a custom period")?;
		return Ok((parse(from)?, parse(to)?));
	}
	let today = match date {
		Some(date) => parse(date)?,
		None => {
			let site = web
				.app
				.site(site_id(params))
				.map_err(|err| err.to_string())?
				.ok_or("Site not found")?;
			let tz = TimeZone::get(&site.timezone).unwrap_or(TimeZone::UTC);
			Timestamp::now().to_zoned(tz).date()
		}
	};
	let back = |span: jiff::Span| today.checked_sub(span).map_err(|err| err.to_string());
	Ok(match period {
		"day" => (today, today),
		"7d" => (back(6.days())?, today),
		"30d" => (back(29.days())?, today),
		"month" => (today.first_of_month(), today.last_of_month()),
		"6mo" => (back(5.months())?.first_of_month(), today.last_of_month()),
		"12mo" => (back(11.months())?.first_of_month(), today.last_of_month()),
		_ => return Err(format!("Invalid period `{period}`")),
	})
}

fn filters(params: &Params) -> Result<Vec<Value>, String> {
	params
		.get("filters")
		.map_or("", String::as_str)
		.split(';')
		.filter(|part| !part.trim().is_empty())
		.map(|part| {
			let (at, op) = ["!=", "==", "!~", "~"]
				.into_iter()
				.filter_map(|op| part.find(op).map(|at| (at, op)))
				.min()
				.ok_or_else(|| format!("Invalid filter `{part}`"))?;
			let values: Vec<&str> = part[at + op.len()..].trim().split('|').collect();
			let wildcard = values.iter().any(|value| value.contains('*'));
			let op = match (op, wildcard) {
				("==", false) => "is",
				("==", true) => "matches_wildcard",
				("!=", false) => "is_not",
				("!=", true) => "matches_wildcard_not",
				("~", _) => "contains",
				_ => "contains_not",
			};
			Ok(json!([op, part[..at].trim(), values]))
		})
		.collect()
}

fn change(name: &str, now: &Value, before: &Value) -> Value {
	let (Some(now), Some(before)) = (now.as_f64(), before.as_f64()) else {
		return Value::Null;
	};
	json!(match name {
		"bounce_rate" => (now - before).round() as i64,
		_ if before == 0.0 => 100 * i64::from(now > 0.0),
		_ => ((now - before) / before * 100.0).round() as i64,
	})
}
