use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use jiff::Timestamp;
use jiff::tz::TimeZone;
use serde_json::json;

use crate::accounts::Web;
use crate::app::lock;
use crate::stats::run;

const FRESH: Duration = Duration::from_secs(300);

type Counts = HashMap<(u64, String, String), (Instant, u64)>;

static COUNTS: LazyLock<Mutex<Counts>> = LazyLock::new(Mutex::default);

pub async fn counter(
	State(web): State<Arc<Web>>,
	Path(file): Path<String>,
	Query(params): Query<HashMap<String, String>>,
	headers: HeaderMap,
) -> Response {
	let missing = |message: &str| (StatusCode::NOT_FOUND, message.to_owned()).into_response();
	let Some((path, kind)) = file.rsplit_once('.') else {
		return missing("Use /counter/PATH.json, .svg or .html");
	};
	let page = [header::ORIGIN, header::REFERER]
		.into_iter()
		.filter_map(|name| headers.get(name)?.to_str().ok())
		.find_map(|value| url::Url::parse(value).ok()?.host_str().map(str::to_owned))
		.unwrap_or_default();
	let domain = params.get("site").cloned().unwrap_or(page);
	let site = web
		.app
		.site(domain.strip_prefix("www.").unwrap_or(&domain))
		.ok()
		.flatten()
		.filter(|site| web.app.counter(site.id).unwrap_or(false));
	let Some(site) = site else {
		return missing("This site has no public visitor counter.");
	};

	let tz = TimeZone::get(&site.timezone).unwrap_or(TimeZone::UTC);
	let today = Timestamp::now().to_zoned(tz).date().to_string();
	let range = match params.get("start").map(String::as_str) {
		None => json!("all"),
		Some("week") => json!("7d"),
		Some("month") => json!("30d"),
		Some("year") => json!("365d"),
		Some(start) => json!([start, params.get("end").unwrap_or(&today)]),
	};
	let filters = match path {
		"TOTAL" => json!([]),
		path => json!([
			["is", "event:name", ["pageview"]],
			[
				"is",
				"event:page",
				[format!("/{}", path.trim_start_matches('/'))]
			],
		]),
	};
	let key = (site.id, path.to_owned(), range.to_string());
	let cached = lock(&COUNTS)
		.get(&key)
		.filter(|(at, _)| at.elapsed() < FRESH)
		.map(|&(_, count)| count);
	let count = match cached {
		Some(count) => count,
		None => {
			let result = run(
				&web.stats,
				json!({
					"site_id": site.domain,
					"metrics": ["visitors"],
					"date_range": range,
					"filters": filters,
				}),
			);
			let count = match result {
				Ok(result) => result["results"][0]["metrics"][0]
					.as_u64()
					.unwrap_or_default(),
				Err(message) => return (StatusCode::BAD_REQUEST, message).into_response(),
			};
			let mut counts = lock(&COUNTS);
			counts.retain(|_, (at, _)| at.elapsed() < FRESH);
			counts.insert(key, (Instant::now(), count));
			count
		}
	};

	let shown = grouped(count);
	let branding = !params.contains_key("no_branding");
	let style = params.get("style").map_or("", String::as_str);
	let mut response = match kind {
		"json" => Json(json!({ "count": shown, "count_unique": shown })).into_response(),
		"svg" => (
			[(header::CONTENT_TYPE, "image/svg+xml")],
			svg(&shown, branding),
		)
			.into_response(),
		"html" => (
			[(header::CONTENT_TYPE, "text/html; charset=utf-8")],
			html(&shown, branding, style),
		)
			.into_response(),
		_ => return missing("Use /counter/PATH.json, .svg or .html"),
	};
	let headers = response.headers_mut();
	headers.insert(
		header::ACCESS_CONTROL_ALLOW_ORIGIN,
		HeaderValue::from_static("*"),
	);
	headers.insert(
		header::CACHE_CONTROL,
		HeaderValue::from_static("public, max-age=300"),
	);
	response
}

fn grouped(count: u64) -> String {
	let digits = count.to_string();
	let mut out = String::new();
	for (i, digit) in digits.chars().enumerate() {
		if i > 0 && (digits.len() - i).is_multiple_of(3) {
			out.push(',');
		}
		out.push(digit);
	}
	out
}

fn svg(count: &str, branding: bool) -> String {
	let height = if branding { 66 } else { 48 };
	let by = match branding {
		true => r##"<text x="100" y="58" font-size="10" fill="#8a96a3">stats by impactite</text>"##,
		false => "",
	};
	format!(
		r##"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="{height}" font-family="ui-monospace,monospace" text-anchor="middle"><rect x="0.5" y="0.5" width="199" height="{}" rx="2" fill="#0b0f13" stroke="#1d262e"/><text x="100" y="18" font-size="11" fill="#8a96a3">Views for this page</text><text x="100" y="40" font-size="20" fill="#e6edf3">{count}</text>{by}</svg>"##,
		height - 1
	)
}

fn html(count: &str, branding: bool, style: &str) -> String {
	let by = match branding {
		true => {
			r#"<div id="gcvc-by">stats by <a href="https://impactite.dev" target="_blank" rel="noopener">impactite</a></div>"#
		}
		false => "",
	};
	// ids match goatcounter so existing counter css keeps working
	format!(
		r#"<!DOCTYPE html><meta charset="utf-8"><style>body{{margin:0;font:13px ui-monospace,monospace;color:#e6edf3;background:#0b0f13;text-align:center}}#gcvc-for,#gcvc-by{{color:#8a96a3;font-size:11px}}#gcvc-views{{font-size:20px;margin:2px 0}}a{{color:inherit}}{}</style><div id="gcvc"><div id="gcvc-for">Views for this page</div><div id="gcvc-views">{count}</div>{by}</div>"#,
		style.replace('<', "")
	)
}
