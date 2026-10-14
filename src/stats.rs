use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use crater::{Db, Dim, Filter, Metrics, Op, Query, Report};
use jiff::civil::Date;
use jiff::tz::TimeZone;
use jiff::{Span, Timestamp, ToSpan};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::app::App;

pub struct Stats {
	pub db: Arc<Db>,
	pub app: Arc<App>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
	site_id: String,
	metrics: Vec<String>,
	date_range: Value,
	#[serde(default)]
	dimensions: Vec<String>,
	#[serde(default)]
	filters: Vec<Value>,
	#[serde(default)]
	order_by: Vec<(String, String)>,
	#[serde(default)]
	include: Include,
	#[serde(default)]
	pagination: Pagination,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct Include {
	#[serde(default)]
	time_labels: bool,
	#[serde(default)]
	total_rows: bool,
	#[serde(default)]
	imports: bool,
	#[serde(default)]
	trim_relative_date_range: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Pagination {
	#[serde(default = "default_limit")]
	limit: usize,
	#[serde(default)]
	offset: usize,
}

impl Default for Pagination {
	fn default() -> Self {
		Self {
			limit: default_limit(),
			offset: 0,
		}
	}
}

fn default_limit() -> usize {
	10_000
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Metric {
	Visitors,
	Visits,
	Pageviews,
	ViewsPerVisit,
	BounceRate,
	VisitDuration,
	Events,
	Percentage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Time {
	Hour,
	Day,
	Week,
	Month,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Grouping {
	Total,
	Dim(Dim),
	Time(Time),
}

struct Row {
	dimensions: Vec<Value>,
	metrics: Metrics,
}

pub async fn query(
	State(stats): State<Arc<Stats>>,
	headers: HeaderMap,
	body: Json<Value>,
) -> Response {
	let authorized = headers
		.get(header::AUTHORIZATION)
		.and_then(|value| value.to_str().ok())
		.and_then(|value| value.strip_prefix("Bearer "))
		.is_some_and(|key| stats.app.key_valid(key).unwrap_or(false));
	if !authorized {
		return error(
			StatusCode::UNAUTHORIZED,
			"Missing or invalid API key. Please use a valid Bearer token.",
		);
	}
	match run(&stats, body.0) {
		Ok(value) => Json(value).into_response(),
		Err(message) => error(StatusCode::BAD_REQUEST, &message),
	}
}

fn run(stats: &Stats, raw: Value) -> Result<Value, String> {
	let request: Request = serde_json::from_value(raw).map_err(|err| format!("#: {err}"))?;
	let site = stats
		.app
		.site(&request.site_id)
		.map_err(|err| err.to_string())?
		.ok_or_else(|| format!("#/site_id: Site {} not found", request.site_id))?;
	let tz = TimeZone::get(&site.timezone).unwrap_or(TimeZone::UTC);
	let today = Timestamp::now().to_zoned(tz.clone()).date();
	let created = Timestamp::from_second(site.created_at)
		.map_err(|err| err.to_string())?
		.to_zoned(tz.clone())
		.date();
	let (from, to) = date_range(
		&request.date_range,
		today,
		created,
		request.include.trim_relative_date_range,
	)?;

	let metrics = request
		.metrics
		.iter()
		.map(|name| metric(name).ok_or_else(|| format!("#/metrics: Invalid metric \"{name}\"")))
		.collect::<Result<Vec<_>, _>>()?;
	if metrics.is_empty() {
		return Err("#/metrics: Expected a minimum of 1 items".into());
	}
	let grouping = match request.dimensions.as_slice() {
		[] => Grouping::Total,
		[name] => grouping(name)?,
		_ => return Err("#/dimensions: Only one dimension is supported for now".into()),
	};
	if grouping == Grouping::Time(Time::Hour) && from != to {
		return Err("#/dimensions: time:hour needs a single day date_range".into());
	}
	let filters = request
		.filters
		.iter()
		.map(filters)
		.collect::<Result<Vec<_>, _>>()?
		.concat();

	let report = stats
		.db
		.query(&Query {
			site: site.id,
			days: day_number(from)..=day_number(to),
			filters,
			breakdown: match grouping {
				Grouping::Dim(dim) => Some(dim),
				_ => None,
			},
			limit: usize::MAX,
		})
		.map_err(|err| err.to_string())?;

	let mut rows = rows(&report, grouping, from);
	let total_visitors = report.total.visitors;
	let sort_key = |name: &str| -> Option<usize> {
		match request.dimensions.iter().position(|dim| dim == name) {
			Some(_) => None,
			None => request.metrics.iter().position(|metric| metric == name),
		}
	};
	match request.order_by.first() {
		Some((name, direction)) => {
			let by_metric = sort_key(name);
			rows.sort_by(|a, b| {
				let ordering = match by_metric {
					Some(i) => value(metrics[i], &a.metrics, total_visitors)
						.as_f64()
						.partial_cmp(&value(metrics[i], &b.metrics, total_visitors).as_f64())
						.unwrap_or(std::cmp::Ordering::Equal),
					None => a
						.dimensions
						.first()
						.map(Value::to_string)
						.cmp(&b.dimensions.first().map(Value::to_string)),
				};
				match direction.as_str() {
					"asc" => ordering,
					_ => ordering.reverse(),
				}
			});
		}
		None if !matches!(grouping, Grouping::Time(_)) => rows.sort_by(|a, b| {
			value(metrics[0], &b.metrics, total_visitors)
				.as_f64()
				.partial_cmp(&value(metrics[0], &a.metrics, total_visitors).as_f64())
				.unwrap_or(std::cmp::Ordering::Equal)
		}),
		None => {}
	}

	let total_rows = rows.len();
	let time_labels: Vec<Value> = match grouping {
		Grouping::Time(_) => rows
			.iter()
			.filter_map(|row| row.dimensions.first().cloned())
			.collect(),
		_ => Vec::new(),
	};
	let results: Vec<Value> = rows
		.into_iter()
		.skip(request.pagination.offset)
		.take(request.pagination.limit)
		.map(|row| {
			json!({
				"metrics": metrics.iter().map(|&m| value(m, &row.metrics, total_visitors)).collect::<Vec<_>>(),
				"dimensions": row.dimensions,
			})
		})
		.collect();

	let mut meta = json!({});
	if request.include.time_labels {
		meta["time_labels"] = json!(time_labels);
	}
	if request.include.total_rows {
		meta["total_rows"] = json!(total_rows);
	}
	if request.include.imports {
		meta["imports_included"] = json!(false);
		meta["imports_skip_reason"] = json!("no_imported_data");
	}
	Ok(json!({
		"results": results,
		"meta": meta,
		"query": {
			"site_id": site.domain,
			"metrics": request.metrics,
			"date_range": [
				from.to_zoned(tz.clone()).map(|day| day.timestamp().to_string()).unwrap_or_default(),
				to.checked_add(1.day())
					.and_then(|end| end.to_zoned(tz.clone()))
					.map(|end| end.timestamp().saturating_sub(1.second()).map(|ts| ts.to_string()).unwrap_or_default())
					.unwrap_or_default(),
			],
			"dimensions": request.dimensions,
			"filters": request.filters,
		},
	}))
}

fn rows(report: &Report, grouping: Grouping, from: Date) -> Vec<Row> {
	match grouping {
		Grouping::Total => vec![Row {
			dimensions: Vec::new(),
			metrics: report.total,
		}],
		Grouping::Dim(dim) => report
			.rows
			.iter()
			.map(|(value, metrics)| Row {
				dimensions: vec![json!(label(dim, value))],
				metrics: *metrics,
			})
			.collect(),
		Grouping::Time(Time::Hour) => report
			.hours
			.iter()
			.enumerate()
			.map(|(hour, metrics)| Row {
				dimensions: vec![json!(format!("{from} {hour:02}:00:00"))],
				metrics: *metrics,
			})
			.collect(),
		Grouping::Time(unit) => {
			let mut rows: Vec<Row> = Vec::new();
			for (offset, metrics) in report.days.iter().enumerate() {
				let Ok(day) = from.checked_add((offset as i64).days()) else {
					continue;
				};
				let bucket = match unit {
					Time::Week => day
						.checked_sub(i64::from(day.weekday().to_monday_zero_offset()).days())
						.unwrap_or(day),
					Time::Month => day.first_of_month(),
					_ => day,
				};
				let label = json!(bucket.to_string());
				match rows.last_mut() {
					Some(row) if row.dimensions.first() == Some(&label) => {
						merge(&mut row.metrics, metrics)
					}
					_ => rows.push(Row {
						dimensions: vec![label],
						metrics: *metrics,
					}),
				}
			}
			rows
		}
	}
}

fn label(dim: Dim, value: &str) -> &str {
	match (value, dim) {
		("", Dim::Source) => "Direct / None",
		("", _) => "(none)",
		(value, _) => value,
	}
}

fn unlabel(dim: Dim, value: String) -> String {
	match label(dim, "") == value {
		true => String::new(),
		false => value,
	}
}

fn merge(into: &mut Metrics, other: &Metrics) {
	into.visitors += other.visitors;
	into.visits += other.visits;
	into.pageviews += other.pageviews;
	into.events += other.events;
	into.bounces += other.bounces;
	into.duration += other.duration;
}

fn value(metric: Metric, metrics: &Metrics, total_visitors: u64) -> Value {
	let ratio = |a: u64, b: u64| match b {
		0 => 0.0,
		_ => a as f64 / b as f64,
	};
	match metric {
		Metric::Visitors => json!(metrics.visitors),
		Metric::Visits => json!(metrics.visits),
		Metric::Pageviews => json!(metrics.pageviews),
		Metric::Events => json!(metrics.events),
		Metric::ViewsPerVisit => {
			json!((ratio(metrics.pageviews, metrics.visits) * 100.0).round() / 100.0)
		}
		Metric::BounceRate => {
			json!((ratio(metrics.bounces, metrics.visits) * 100.0).round() as u64)
		}
		Metric::VisitDuration => json!(ratio(metrics.duration, metrics.visits).round() as u64),
		Metric::Percentage => {
			json!((ratio(metrics.visitors, total_visitors) * 1000.0).round() / 10.0)
		}
	}
}

fn metric(name: &str) -> Option<Metric> {
	Some(match name {
		"visitors" => Metric::Visitors,
		"visits" => Metric::Visits,
		"pageviews" => Metric::Pageviews,
		"views_per_visit" => Metric::ViewsPerVisit,
		"bounce_rate" => Metric::BounceRate,
		"visit_duration" => Metric::VisitDuration,
		"events" => Metric::Events,
		"percentage" => Metric::Percentage,
		_ => return None,
	})
}

fn dimension(name: &str) -> Option<Dim> {
	Some(match name {
		"event:name" => Dim::Name,
		"event:hostname" => Dim::Hostname,
		"event:page" => Dim::Page,
		"visit:referrer" => Dim::Referrer,
		"visit:source" => Dim::Source,
		"visit:channel" => Dim::Channel,
		"visit:utm_source" => Dim::UtmSource,
		"visit:utm_medium" => Dim::UtmMedium,
		"visit:utm_campaign" => Dim::UtmCampaign,
		"visit:utm_content" => Dim::UtmContent,
		"visit:utm_term" => Dim::UtmTerm,
		"visit:country" => Dim::Country,
		"visit:region" => Dim::Region,
		"visit:city" => Dim::City,
		"visit:device" | "visit:screen" => Dim::Device,
		"visit:browser" => Dim::Browser,
		"visit:browser_version" => Dim::BrowserVersion,
		"visit:os" => Dim::Os,
		"visit:os_version" => Dim::OsVersion,
		_ => return None,
	})
}

fn grouping(name: &str) -> Result<Grouping, String> {
	Ok(match name {
		"time:hour" => Grouping::Time(Time::Hour),
		"time" | "time:day" => Grouping::Time(Time::Day),
		"time:week" => Grouping::Time(Time::Week),
		"time:month" => Grouping::Time(Time::Month),
		name => Grouping::Dim(
			dimension(name)
				.ok_or_else(|| format!("#/dimensions/0: Invalid dimension \"{name}\""))?,
		),
	})
}

fn filters(raw: &Value) -> Result<Vec<Filter>, String> {
	let invalid = || format!("#/filters: Invalid filter {raw}");
	let Value::Array(parts) = raw else {
		return Err(invalid());
	};
	match parts.as_slice() {
		[Value::String(op), Value::Array(inner)] if op == "and" => Ok(inner
			.iter()
			.map(filters)
			.collect::<Result<Vec<_>, _>>()?
			.concat()),
		[
			Value::String(op),
			Value::String(dim),
			Value::Array(clauses),
			..,
		] => {
			let op = match op.as_str() {
				"is" => Op::Is,
				"is_not" => Op::IsNot,
				"contains" => Op::Contains,
				"contains_not" => Op::ContainsNot,
				_ => return Err(format!("#/filters: Unsupported filter operator \"{op}\"")),
			};
			let dim = dimension(dim)
				.ok_or_else(|| format!("#/filters: Invalid filter dimension \"{dim}\""))?;
			let values = clauses
				.iter()
				.map(|clause| match clause {
					Value::String(text) => Ok(unlabel(dim, text.clone())),
					Value::Number(number) => Ok(number.to_string()),
					_ => Err(invalid()),
				})
				.collect::<Result<_, _>>()?;
			Ok(vec![Filter { dim, op, values }])
		}
		_ => Err(invalid()),
	}
}

fn date_range(raw: &Value, today: Date, created: Date, trim: bool) -> Result<(Date, Date), String> {
	let invalid = || format!("#/date_range: Invalid date range {raw}");
	let parse = |value: &Value| -> Result<Date, String> {
		let text = value.as_str().ok_or_else(invalid)?;
		text.get(..10)
			.unwrap_or(text)
			.parse()
			.map_err(|_| invalid())
	};
	let back = |span: Span| today.checked_sub(span).map_err(|err| err.to_string());
	let range = match raw {
		Value::Array(bounds) if bounds.len() == 2 => (parse(&bounds[0])?, parse(&bounds[1])?),
		Value::String(name) => match name.as_str() {
			"day" => (today, today),
			"month" => (
				today.first_of_month(),
				match trim {
					true => today,
					false => today.last_of_month(),
				},
			),
			"year" => (
				today.first_of_year(),
				match trim {
					true => today,
					false => today.last_of_year(),
				},
			),
			"all" => (created.min(today), today),
			name => {
				let (count, unit) = name
					.find(|c: char| !c.is_ascii_digit())
					.map(|i| name.split_at(i))
					.ok_or_else(invalid)?;
				let count: i64 = count.parse().map_err(|_| invalid())?;
				match unit {
					"d" if count > 0 => (back((count - 1).days())?, today),
					"mo" if count > 0 => (back((count - 1).months())?.first_of_month(), today),
					_ => return Err(invalid()),
				}
			}
		},
		_ => return Err(invalid()),
	};
	match range.0 <= range.1 {
		true => Ok(range),
		false => Err(invalid()),
	}
}

fn day_number(date: Date) -> i32 {
	Date::constant(1970, 1, 1)
		.until(date)
		.map_or(0, |span| span.get_days())
}

fn error(status: StatusCode, message: &str) -> Response {
	(status, Json(json!({ "error": message }))).into_response()
}
