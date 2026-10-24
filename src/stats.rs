use std::collections::{BTreeMap, HashMap};
use std::ops::RangeInclusive;
use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use crater::{Db, Dim, Field, Filter, Metrics, Op, Query, Report};
use jiff::civil::{Date, DateTime};
use jiff::tz::TimeZone;
use jiff::{Span, Timestamp, ToSpan};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::app::{App, Site};
use crate::site::goals::Goal;
use crate::site::shares::Reader;
use crate::web::{Error, Reply, Web};

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
	ConversionRate,
	TimeOnPage,
	ScrollDepth,
	ExitRate,
	TotalRevenue,
	AverageRevenue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Time {
	Minute,
	Hour,
	Day,
	Week,
	Month,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Grouping {
	Total,
	Time(Time),
	Field(Field),
	Goal,
	TimeField(Time, Field),
}

struct Row {
	dimensions: Vec<Value>,
	metrics: Metrics,
}

#[derive(Serialize)]
pub(crate) struct Answer {
	pub results: Vec<Line>,
	meta: Value,
	query: Value,
}

#[derive(Serialize)]
pub(crate) struct Line {
	pub metrics: Vec<Value>,
	pub dimensions: Vec<Value>,
}

pub async fn query(
	State(stats): State<Arc<Stats>>,
	headers: HeaderMap,
	body: Json<Value>,
) -> Response {
	let domain = body.0["site_id"].as_str().unwrap_or_default();
	if !keyed(&stats.app, &headers, domain) {
		return unauthorized();
	}
	match answer(&stats, body.0) {
		Ok(answer) => Json(answer).into_response(),
		Err(message) => error(StatusCode::BAD_REQUEST, &message),
	}
}

pub(crate) fn keyed(app: &App, headers: &HeaderMap, domain: &str) -> bool {
	headers
		.get(header::AUTHORIZATION)
		.and_then(|value| value.to_str().ok())
		.and_then(|value| value.strip_prefix("Bearer "))
		.is_some_and(|key| key_reads(app, key, domain))
}

pub(crate) fn key_reads(app: &App, key: &str, domain: &str) -> bool {
	match app.key_owner(key).ok().flatten() {
		Some(Some(user)) => matches!(app.site_role(user, domain), Ok(Some(_))),
		Some(None) => true,
		None => false,
	}
}

pub(crate) fn unauthorized() -> Response {
	error(
		StatusCode::UNAUTHORIZED,
		"Missing or invalid API key. Please use a valid Bearer token.",
	)
}

pub async fn dashboard(State(web): State<Arc<Web>>, reader: Reader, body: Json<Value>) -> Reply {
	let domain = body.0["site_id"].as_str().unwrap_or_default();
	if !reader.reads(&web, domain) {
		return Err(Error::not_found("This site doesn't exist."));
	}
	Ok(Json(run(&web.stats, body.0).map_err(Error::bad)?))
}

#[derive(Deserialize)]
pub struct PropsRequest {
	site_id: String,
	date_range: Value,
}

pub async fn props(
	State(web): State<Arc<Web>>,
	reader: Reader,
	Json(body): Json<PropsRequest>,
) -> Reply {
	if !reader.reads(&web, &body.site_id) {
		return Err(Error::not_found("This site doesn't exist."));
	}
	let period = period(&web.stats, &body.site_id, &body.date_range, false).map_err(Error::bad)?;
	let keys = web
		.stats
		.db
		.prop_keys(
			period.site.id,
			day_number(period.from)..=day_number(period.to),
		)
		.map_err(|err| Error::bad(err.to_string()))?;
	Ok(Json(json!(
		keys.iter().map(|key| &**key).collect::<Vec<_>>()
	)))
}

pub(crate) struct Period {
	pub(crate) site: Site,
	// the rest of a team's sites when it is viewed as one, `team:<id>`
	pub(crate) others: Vec<u64>,
	tz: TimeZone,
	pub(crate) from: Date,
	pub(crate) to: Date,
	imported: Option<Date>,
}

pub(crate) fn period(
	stats: &Stats,
	site_id: &str,
	range: &Value,
	trim: bool,
) -> Result<Period, String> {
	let missing = || format!("#/site_id: Site {site_id} not found");
	let (site, others) = match site_id.strip_prefix("team:") {
		Some(team) => {
			let team = team.parse().map_err(|_| missing())?;
			let mut sites = stats.app.team_sites(team).map_err(|err| err.to_string())?;
			sites.sort_by_key(|site| site.id);
			let others = sites.iter().skip(1).map(|site| site.id).collect();
			(sites.into_iter().next().ok_or_else(missing)?, others)
		}
		None => (
			stats
				.app
				.site(site_id)
				.map_err(|err| err.to_string())?
				.ok_or_else(missing)?,
			Vec::new(),
		),
	};
	let tz = TimeZone::get(&site.timezone).unwrap_or(TimeZone::UTC);
	let today = Timestamp::now().to_zoned(tz.clone()).date();
	let (created, imported) = created(&stats.app, &site, &tz)?;
	let (from, to) = date_range(range, today, created, trim)?;
	let (from, to) = bound(from, to, created, today);
	Ok(Period {
		site,
		others,
		tz,
		from,
		to,
		imported,
	})
}

pub(crate) fn run(stats: &Stats, raw: Value) -> Result<Value, String> {
	serde_json::to_value(answer(stats, raw)?).map_err(|err| err.to_string())
}

type Window = (RangeInclusive<i32>, Option<u32>);

// every question runs once per window, realtime spans midnight in two
struct Scan<'a> {
	stats: &'a Stats,
	others: &'a [u64],
	site: u64,
	imports: bool,
	windows: Vec<Window>,
}

impl Scan<'_> {
	fn run(
		&self,
		filters: Vec<Filter>,
		breakdown: Option<Field>,
		daily: bool,
	) -> Result<Report, String> {
		let mut merged: Option<Report> = None;
		for (days, since) in &self.windows {
			let next = self.ask(
				days.clone(),
				*since,
				filters.clone(),
				breakdown.clone(),
				daily,
			)?;
			merged = Some(match merged {
				Some(report) => combine(report, next),
				None => next,
			});
		}
		merged.ok_or_else(|| "empty date range".to_string())
	}

	fn ask(
		&self,
		days: RangeInclusive<i32>,
		since: Option<u32>,
		filters: Vec<Filter>,
		breakdown: Option<Field>,
		daily: bool,
	) -> Result<Report, String> {
		query_sites(
			self.stats,
			self.others,
			Query {
				site: self.site,
				days,
				filters,
				breakdown,
				limit: usize::MAX,
				daily,
				imports: self.imports,
				since,
			},
		)
	}
}

pub(crate) fn answer(stats: &Stats, raw: Value) -> Result<Answer, String> {
	let request: Request = serde_json::from_value(raw).map_err(|err| format!("#: {err}"))?;
	let realtime = request.date_range == json!("30m");
	let day = json!("day");
	let Period {
		site,
		others,
		tz,
		from,
		to,
		imported,
	} = period(
		stats,
		&request.site_id,
		if realtime { &day } else { &request.date_range },
		request.include.trim_relative_date_range,
	)?;
	let (windows, start) = windows(from, to, &tz, realtime)?;

	let metrics = request
		.metrics
		.iter()
		.map(|name| metric(name).ok_or_else(|| format!("#/metrics: Invalid metric \"{name}\"")))
		.collect::<Result<Vec<_>, _>>()?;
	if metrics.is_empty() {
		return Err("#/metrics: Expected a minimum of 1 items".into());
	}
	let grouping = grouping(&request.dimensions)?;
	check_grouping(&grouping, from == to, realtime)?;

	let goals = if others.is_empty() {
		stats.app.goals(site.id).map_err(|err| err.to_string())?
	} else {
		Vec::new()
	};
	let mut base_filters = Vec::new();
	let mut goal_filters = Vec::new();
	for raw in &request.filters {
		for (filter, from_goal) in filters(raw, &goals)? {
			if from_goal {
				goal_filters.push(filter)
			} else {
				base_filters.push(filter)
			}
		}
	}
	let skip = match imported {
		None => "no_imported_data",
		Some(_) => imports_skip(&grouping, !request.filters.is_empty()).unwrap_or_default(),
	};
	let imports = request.include.imports && skip.is_empty();
	let scan = Scan {
		stats,
		others: &others,
		site: site.id,
		imports,
		windows,
	};
	let filtered = || {
		base_filters
			.iter()
			.chain(&goal_filters)
			.cloned()
			.collect::<Vec<_>>()
	};

	let (mut rows, base) = match &grouping {
		Grouping::Goal => {
			let base = scan.run(filtered(), None, false)?.total.visitors;
			let rows = goals
				.iter()
				.map(|goal| {
					let mut filters = filtered();
					filters.extend(goal.filters());
					Ok(Row {
						dimensions: vec![json!(goal.name)],
						metrics: scan.run(filters, None, false)?.total,
					})
				})
				.collect::<Result<Vec<_>, String>>()?
				.into_iter()
				.filter(|row| row.metrics.visitors > 0)
				.collect();
			(rows, base)
		}
		grouping => {
			let (field, daily) = match grouping {
				Grouping::Field(field) => (Some(field.clone()), false),
				Grouping::TimeField(_, field) => (Some(field.clone()), true),
				_ => (None, false),
			};
			let report = scan.run(filtered(), field, daily)?;
			let base = if goal_filters.is_empty() {
				report.total.visitors
			} else {
				scan.run(base_filters.clone(), None, false)?.total.visitors
			};
			let rows = match grouping {
				// reports keep hours for one day, so longer ranges ask day by day
				Grouping::Time(Time::Hour) if from != to => (day_number(from)..=day_number(to))
					.zip(from.series(1.day()))
					.map(|(number, day)| {
						let report = scan.ask(number..=number, None, filtered(), None, false)?;
						Ok(rows(&report, grouping, day, start))
					})
					.collect::<Result<Vec<_>, String>>()?
					.into_iter()
					.flatten()
					.collect(),
				_ => rows(&report, grouping, from, start),
			};
			(rows, base)
		}
	};

	sort(&mut rows, &request, &grouping, &metrics, base);
	let total_rows = rows.len();
	let mut time_labels: Vec<Value> = match grouping {
		Grouping::Time(_) | Grouping::TimeField(..) => rows
			.iter()
			.filter_map(|row| row.dimensions.first().cloned())
			.collect(),
		_ => Vec::new(),
	};
	time_labels.dedup();
	let results = rows
		.into_iter()
		.skip(request.pagination.offset)
		.take(request.pagination.limit)
		.map(|row| Line {
			metrics: metrics
				.iter()
				.map(|&m| value(m, &row.metrics, base))
				.collect(),
			dimensions: row.dimensions,
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
		meta["imports_included"] = json!(imports);
		if !imports {
			meta["imports_skip_reason"] = json!(skip);
		}
	}
	Ok(Answer {
		results,
		meta,
		query: echo(&request, &site, &tz, from, to),
	})
}

fn windows(
	from: Date,
	to: Date,
	tz: &TimeZone,
	realtime: bool,
) -> Result<(Vec<Window>, Option<DateTime>), String> {
	let first = day_number(from);
	if !realtime {
		return Ok((vec![(first..=day_number(to), None)], None));
	}
	let now = Timestamp::now().to_zoned(tz.clone()).datetime();
	let start = (now - 30.minutes())
		.with()
		.second(0)
		.subsec_nanosecond(0)
		.build()
		.map_err(|err| err.to_string())?;
	let since = start.time().hour() as u32 * 3600 + start.time().minute() as u32 * 60;
	let windows = if start.date() == now.date() {
		vec![(first..=first, Some(since))]
	} else {
		vec![
			(first - 1..=first - 1, Some(since)),
			(first..=first, Some(0)),
		]
	};
	Ok((windows, Some(start)))
}

fn check_grouping(grouping: &Grouping, single_day: bool, realtime: bool) -> Result<(), String> {
	if matches!(grouping, Grouping::TimeField(Time::Hour, _)) && !single_day {
		return Err(
			"#/dimensions: time:hour with another dimension needs a single day date_range".into(),
		);
	}
	if matches!(
		grouping,
		Grouping::Time(Time::Hour) | Grouping::TimeField(Time::Hour, _)
	) && realtime
	{
		return Err("#/dimensions: time:hour does not go with the 30m date_range".into());
	}
	if matches!(grouping, Grouping::Time(Time::Minute)) != realtime
		&& matches!(grouping, Grouping::Time(_) | Grouping::TimeField(..))
	{
		return Err("#/dimensions: time:minute goes with the 30m date_range only".into());
	}
	Ok(())
}

// the first order wins, with no order the biggest first and time in its own order
fn sort(rows: &mut [Row], request: &Request, grouping: &Grouping, metrics: &[Metric], base: u64) {
	let compare = |m: Metric, a: &Row, b: &Row| {
		value(m, &a.metrics, base)
			.as_f64()
			.partial_cmp(&value(m, &b.metrics, base).as_f64())
			.unwrap_or(std::cmp::Ordering::Equal)
	};
	match request.order_by.first() {
		Some((name, direction)) => {
			let by_metric = if request.dimensions.iter().any(|dim| dim == name) {
				None
			} else {
				request.metrics.iter().position(|metric| metric == name)
			};
			rows.sort_by(|a, b| {
				let ordering = match by_metric {
					Some(i) => compare(metrics[i], a, b),
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
		None if !matches!(grouping, Grouping::Time(_) | Grouping::TimeField(..)) => {
			rows.sort_by(|a, b| compare(metrics[0], b, a))
		}
		None => {}
	}
}

fn echo(request: &Request, site: &Site, tz: &TimeZone, from: Date, to: Date) -> Value {
	json!({
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
	})
}

fn query_sites(stats: &Stats, others: &[u64], query: Query) -> Result<Report, String> {
	let mut report = stats.db.query(&query).map_err(|err| err.to_string())?;
	for &other in others {
		let next = stats
			.db
			.query(&Query {
				site: other,
				..query.clone()
			})
			.map_err(|err| err.to_string())?;
		report.total.merge(&next.total);
		for (day, other) in report.days.iter_mut().zip(&next.days) {
			day.merge(other);
		}
		for (hour, other) in report.hours.iter_mut().zip(&next.hours) {
			hour.merge(other);
		}
		for (minute, other) in report.minutes.iter_mut().zip(&next.minutes) {
			*minute += other;
		}
		report.rows = add_rows(std::mem::take(&mut report.rows), next.rows);
		report.daily = std::mem::take(&mut report.daily)
			.into_iter()
			.zip(next.daily)
			.map(|(rows, other)| add_rows(rows, other))
			.collect();
	}
	Ok(report)
}

fn add_rows(
	rows: Vec<(Arc<str>, Metrics)>,
	other: Vec<(Arc<str>, Metrics)>,
) -> Vec<(Arc<str>, Metrics)> {
	let mut all: HashMap<Arc<str>, Metrics> = rows.into_iter().collect();
	for (value, metrics) in other {
		all.entry(value).or_default().merge(&metrics);
	}
	let mut all: Vec<_> = all.into_iter().collect();
	all.sort_unstable_by(|a, b| b.1.visitors.cmp(&a.1.visitors).then(a.0.cmp(&b.0)));
	all
}

fn combine(mut report: Report, next: Report) -> Report {
	report.total.merge(&next.total);
	report.minutes.extend(next.minutes);
	let mut rows: HashMap<Arc<str>, Metrics> = report.rows.into_iter().collect();
	for (value, metrics) in next.rows {
		rows.entry(value).or_default().merge(&metrics);
	}
	report.rows = rows.into_iter().collect();
	report
		.rows
		.sort_unstable_by(|a, b| b.1.visitors.cmp(&a.1.visitors).then(a.0.cmp(&b.0)));
	report
}

fn rows(report: &Report, grouping: &Grouping, from: Date, start: Option<DateTime>) -> Vec<Row> {
	let ranked = |field: &Field, rows: &[(Arc<str>, Metrics)]| -> Vec<(String, Metrics)> {
		rows.iter()
			.map(|(value, metrics)| (label(field, value).to_owned(), *metrics))
			.collect()
	};
	match grouping {
		Grouping::Total | Grouping::Goal => vec![Row {
			dimensions: Vec::new(),
			metrics: report.total,
		}],
		Grouping::Field(field) => ranked(field, &report.rows)
			.into_iter()
			.map(|(value, metrics)| Row {
				dimensions: vec![json!(value)],
				metrics,
			})
			.collect(),
		Grouping::Time(Time::Minute) => (0..=30)
			.filter_map(|minute| {
				let at = start?.checked_add(minute.minutes()).ok()?;
				Some(Row {
					dimensions: vec![json!(at.strftime("%Y-%m-%d %H:%M:00").to_string())],
					metrics: Metrics {
						pageviews: report.minutes.get(minute as usize).copied().unwrap_or(0),
						..Metrics::default()
					},
				})
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
			let mut buckets: BTreeMap<String, Metrics> = BTreeMap::new();
			for (offset, metrics) in report.days.iter().enumerate() {
				merge(
					buckets.entry(bucket(*unit, from, offset)).or_default(),
					metrics,
				);
			}
			buckets
				.into_iter()
				.map(|(label, metrics)| Row {
					dimensions: vec![json!(label)],
					metrics,
				})
				.collect()
		}
		Grouping::TimeField(unit, field) => {
			let mut buckets: BTreeMap<(String, String), Metrics> = BTreeMap::new();
			for (offset, day) in report.daily.iter().enumerate() {
				let time = bucket(*unit, from, offset);
				for (value, metrics) in ranked(field, day) {
					merge(buckets.entry((time.clone(), value)).or_default(), &metrics);
				}
			}
			buckets
				.into_iter()
				.map(|((time, value), metrics)| Row {
					dimensions: vec![json!(time), json!(value)],
					metrics,
				})
				.collect()
		}
	}
}

fn bucket(unit: Time, from: Date, offset: usize) -> String {
	let day = from.checked_add((offset as i64).days()).unwrap_or(from);
	match unit {
		Time::Week => day
			.checked_sub(i64::from(day.weekday().to_monday_zero_offset()).days())
			.unwrap_or(day)
			.to_string(),
		Time::Month => day.first_of_month().to_string(),
		Time::Minute | Time::Day | Time::Hour => day.to_string(),
	}
}

fn label<'a>(field: &Field, value: &'a str) -> &'a str {
	match (value, field) {
		("", Field::Dim(Dim::Source)) => "Direct / None",
		("", _) => "(none)",
		(value, _) => value,
	}
}

fn unlabel(field: &Field, value: String) -> String {
	if label(field, "") == value {
		String::new()
	} else {
		value
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

fn value(metric: Metric, metrics: &Metrics, base: u64) -> Value {
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
		Metric::Percentage => json!((ratio(metrics.visitors, base) * 1000.0).round() / 10.0),
		Metric::ConversionRate => json!((ratio(metrics.visitors, base) * 10000.0).round() / 100.0),
		Metric::TimeOnPage => match metrics.engaged_views {
			0 => Value::Null,
			views => json!((metrics.engaged as f64 / views as f64 / 1000.0).round() as u64),
		},
		Metric::ScrollDepth => match metrics.scrolled {
			0 => Value::Null,
			views => json!((metrics.scroll as f64 / views as f64).round() as u64),
		},
		Metric::ExitRate => json!((ratio(metrics.exits, metrics.visits) * 1000.0).round() / 10.0),
		Metric::TotalRevenue | Metric::AverageRevenue if metrics.revenue == 0 => Value::Null,
		Metric::TotalRevenue => json!(metrics.revenue as f64 / 100.0),
		Metric::AverageRevenue => json!((ratio(metrics.revenue, metrics.events)).round() / 100.0),
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
		"conversion_rate" => Metric::ConversionRate,
		"time_on_page" => Metric::TimeOnPage,
		"scroll_depth" => Metric::ScrollDepth,
		"exit_rate" => Metric::ExitRate,
		"total_revenue" => Metric::TotalRevenue,
		"average_revenue" => Metric::AverageRevenue,
		_ => return None,
	})
}

fn dimension(name: &str) -> Option<Field> {
	if let Some(key) = name.strip_prefix("event:props:") {
		return Some(Field::Prop(key.to_owned()));
	}
	Some(Field::Dim(match name {
		"visit:entry_page" => return Some(Field::Entry),
		"visit:exit_page" => return Some(Field::Exit),
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
		"visit:language" => Dim::Language,
		_ => return None,
	}))
}

fn time(name: &str) -> Option<Time> {
	Some(match name {
		"time:minute" => Time::Minute,
		"time:hour" => Time::Hour,
		"time" | "time:day" => Time::Day,
		"time:week" => Time::Week,
		"time:month" => Time::Month,
		_ => return None,
	})
}

fn grouping(names: &[String]) -> Result<Grouping, String> {
	let field = |name: &str| {
		dimension(name).ok_or_else(|| format!("#/dimensions: Invalid dimension \"{name}\""))
	};
	Ok(match names {
		[] => Grouping::Total,
		[name] if name == "event:goal" => Grouping::Goal,
		[name] => match time(name) {
			Some(unit) => Grouping::Time(unit),
			None => Grouping::Field(field(name)?),
		},
		[a, b] => match (time(a), time(b)) {
			(Some(unit), None) => Grouping::TimeField(unit, field(b)?),
			(None, Some(unit)) => Grouping::TimeField(unit, field(a)?),
			_ => {
				return Err(
					"#/dimensions: Combine one time dimension with one other dimension".into(),
				);
			}
		},
		_ => return Err("#/dimensions: At most two dimensions are supported".into()),
	})
}

pub(crate) fn filters(raw: &Value, goals: &[Goal]) -> Result<Vec<(Filter, bool)>, String> {
	let invalid = || format!("#/filters: Invalid filter {raw}");
	let Value::Array(parts) = raw else {
		return Err(invalid());
	};
	match parts.as_slice() {
		[Value::String(op), Value::Array(inner)] if op == "and" => Ok(inner
			.iter()
			.map(|filter| filters(filter, goals))
			.collect::<Result<Vec<_>, _>>()?
			.concat()),
		[
			Value::String(op),
			Value::String(name),
			Value::Array(clauses),
			..,
		] => {
			let values: Vec<String> = clauses
				.iter()
				.map(|clause| match clause {
					Value::String(text) => Ok(text.clone()),
					Value::Number(number) => Ok(number.to_string()),
					_ => Err(invalid()),
				})
				.collect::<Result<_, _>>()?;
			if name == "event:goal" {
				let [value] = values.as_slice() else {
					return Err("#/filters: Filter by one goal at a time".into());
				};
				let goal = goals
					.iter()
					.find(|goal| &goal.name == value)
					.ok_or_else(|| format!("#/filters: Unknown goal \"{value}\""))?;
				if op != "is" {
					return Err("#/filters: Goals only support \"is\"".into());
				}
				return Ok(goal
					.filters()
					.into_iter()
					.map(|filter| (filter, true))
					.collect());
			}
			let op = match op.as_str() {
				"is" => Op::Is,
				"is_not" => Op::IsNot,
				"contains" => Op::Contains,
				"contains_not" => Op::ContainsNot,
				"matches_wildcard" => Op::Matches,
				"matches_wildcard_not" => Op::MatchesNot,
				_ => return Err(format!("#/filters: Unsupported filter operator \"{op}\"")),
			};
			let field = dimension(name)
				.ok_or_else(|| format!("#/filters: Invalid filter dimension \"{name}\""))?;
			let values = values
				.into_iter()
				.map(|value| unlabel(&field, value))
				.collect();
			Ok(vec![(Filter { field, op, values }, false)])
		}
		_ => Err(invalid()),
	}
}

fn imports_skip(grouping: &Grouping, filtered: bool) -> Option<&'static str> {
	match grouping {
		Grouping::Time(Time::Hour) => Some("unsupported_interval"),
		Grouping::Goal | Grouping::TimeField(..) => Some("unsupported_query"),
		_ if filtered => Some("unsupported_query"),
		Grouping::Field(Field::Dim(Dim::City)) => Some("unsupported_query"),
		Grouping::Field(Field::Prop(key)) if !matches!(key.as_str(), "url" | "path") => {
			Some("unsupported_query")
		}
		_ => None,
	}
}

// the first day with data, counting imports from before the site was added, and the first import
pub(crate) fn created(
	app: &App,
	site: &Site,
	tz: &TimeZone,
) -> Result<(Date, Option<Date>), String> {
	let imported = app
		.first_import(site.id)
		.map_err(|err| err.to_string())?
		.and_then(|day| day.parse::<Date>().ok());
	let created = Timestamp::from_second(site.created_at)
		.map_err(|err| err.to_string())?
		.to_zoned(tz.clone())
		.date();
	Ok((imported.map_or(created, |day| day.min(created)), imported))
}

// queries walk every day in range, so a range stops where data can be
pub(crate) fn bound(from: Date, to: Date, created: Date, today: Date) -> (Date, Date) {
	let first = created.min(today).saturating_sub(5.years());
	let last = today.last_of_year();
	(from.clamp(first, last), to.clamp(first, last))
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
				if trim { today } else { today.last_of_month() },
			),
			"year" => (
				today.first_of_year(),
				if trim { today } else { today.last_of_year() },
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
	if range.0 <= range.1 {
		Ok(range)
	} else {
		Err(invalid())
	}
}

pub(crate) fn day_number(date: Date) -> i32 {
	Date::constant(1970, 1, 1)
		.until(date)
		.map_or(0, |span| span.get_days())
}

pub(crate) fn error(status: StatusCode, message: &str) -> Response {
	(status, Json(json!({ "error": message }))).into_response()
}
