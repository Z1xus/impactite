use std::collections::{BTreeMap, HashMap};
use std::time::Duration;

use anyhow::{Context, bail};
use crater::Db;
use jiff::civil::Date;
use serde_json::{Value, json};

use super::plausible::{Day, add, write};
use crate::app::Site;
use crate::ingest::referrer;

const API: &str = "https://analyticsdata.googleapis.com/v1beta";
const PAGE: u64 = 100_000;
// google analytics 4 has no data before this day
const FIRST: &str = "2015-08-14";

const VISIT: [&str; 5] = [
	"screenPageViews",
	"totalUsers",
	"sessions",
	"bounces",
	"userEngagementDuration",
];

// the reports plausible pulls, each lands in the plausible table of the same name
const REPORTS: [(&str, &[&str], &[&str]); 9] = [
	("visitors", &["date"], &VISIT),
	(
		"sources",
		&[
			"date",
			"sessionSource",
			"sessionDefaultChannelGroup",
			"sessionMedium",
			"sessionCampaignName",
			"sessionManualAdContent",
			"sessionGoogleAdsKeyword",
		],
		&VISIT,
	),
	(
		"pages",
		&["date", "hostName", "pagePath"],
		&[
			"totalUsers",
			"screenPageViews",
			"sessions",
			"userEngagementDuration",
		],
	),
	("entry_pages", &["date", "landingPage"], &VISIT),
	(
		"custom_events",
		&["date", "eventName", "linkUrl"],
		&["totalUsers", "eventCount"],
	),
	("locations", &["date", "countryId", "region"], &VISIT),
	("devices", &["date", "deviceCategory"], &VISIT),
	("browsers", &["date", "browser"], &VISIT),
	(
		"operating_systems",
		&["date", "operatingSystem", "operatingSystemVersion"],
		&VISIT,
	),
];

// pulls a property's history up to `before`, the day the site started tracking
pub async fn import(
	http: &reqwest::Client,
	token: &str,
	property: &str,
	db: &Db,
	site: &Site,
	before: Date,
) -> anyhow::Result<Option<(Date, Date)>> {
	let last = before.yesterday()?;
	let mut days: BTreeMap<Date, Day> = BTreeMap::new();
	for (table, dimensions, metrics) in REPORTS {
		let mut offset = 0;
		loop {
			let report = fetch(
				http, token, property, table, dimensions, metrics, last, offset,
			)
			.await?;
			let rows = report["rows"].as_array().cloned().unwrap_or_default();
			for row in &rows {
				let values = row_values(table, dimensions, metrics, row);
				let Some(date) = values
					.get("date")
					.and_then(|date| date.parse::<Date>().ok())
				else {
					continue;
				};
				add(days.entry(date).or_default(), table, &|name| {
					values.get(name).map_or("", String::as_str)
				});
			}
			offset += PAGE;
			if rows.is_empty() || offset >= report["rowCount"].as_u64().unwrap_or(0) {
				break;
			}
		}
	}
	write(db, site, &days)
}

#[allow(clippy::too_many_arguments)]
async fn fetch(
	http: &reqwest::Client,
	token: &str,
	property: &str,
	table: &str,
	dimensions: &[&str],
	metrics: &[&str],
	last: Date,
	offset: u64,
) -> anyhow::Result<Value> {
	let body = json!({
		"dateRanges": [{ "startDate": FIRST, "endDate": last.to_string() }],
		"dimensions": dimensions.iter().map(|name| json!({ "name": name })).collect::<Vec<_>>(),
		"metrics": metrics.iter().map(|&name| match name {
			"bounces" => json!({ "name": "bounces", "expression": "sessions - engagedSessions" }),
			name => json!({ "name": name }),
		}).collect::<Vec<_>>(),
		"dimensionFilter": (table == "custom_events").then(|| json!({
			"notExpression": { "filter": { "fieldName": "eventName", "inListFilter": {
				"values": ["page_view", "session_start", "first_visit", "user_engagement"],
				"caseSensitive": true,
			}}},
		})),
		"limit": PAGE,
		"offset": offset,
	});
	// google limits how fast reports come, so a busy minute waits and tries again
	for attempt in 0..6 {
		let response = http
			.post(format!("{API}/{property}:runReport"))
			.bearer_auth(token)
			.json(&body)
			.send()
			.await?;
		if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS
			|| response.status().is_server_error()
		{
			tokio::time::sleep(Duration::from_secs(30 << attempt)).await;
			continue;
		}
		let status = response.status();
		let report: Value = response.json().await.context("read the report")?;
		if !status.is_success() {
			bail!(
				"Google Analytics said: {}",
				report["error"]["message"].as_str().unwrap_or("no")
			);
		}
		return Ok(report);
	}
	bail!("Google Analytics kept limiting the import. Try again later.")
}

// one report row as plausible's column names and values
fn row_values(
	table: &str,
	dimensions: &[&str],
	metrics: &[&str],
	row: &Value,
) -> HashMap<&'static str, String> {
	let read = |list: &str, i: usize| row[list][i]["value"].as_str().unwrap_or_default();
	let dimension = |name: &str| {
		let value = dimensions
			.iter()
			.position(|&dim| dim == name)
			.map_or("", |i| read("dimensionValues", i));
		match value {
			"(none)" | "(not set)" | "(not provided)" | "(other)" => "",
			value => value,
		}
	};
	let metric = |name: &str| {
		let value = metrics
			.iter()
			.position(|&metric| metric == name)
			.map_or("", |i| read("metricValues", i));
		value
			.parse::<f64>()
			.map_or(0, |value| value.round() as u64)
			.to_string()
	};
	let date = dimension("date");
	let mut values = HashMap::from([
		(
			"date",
			match date.len() {
				8 => format!("{}-{}-{}", &date[..4], &date[4..6], &date[6..]),
				_ => String::new(),
			},
		),
		("visitors", metric("totalUsers")),
		("visits", metric("sessions")),
		("pageviews", metric("screenPageViews")),
		("bounces", metric("bounces")),
		("visit_duration", metric("userEngagementDuration")),
	]);
	let mut set = |name: &'static str, value: String| {
		values.insert(name, value);
	};
	match table {
		"sources" => {
			let source = dimension("sessionSource");
			set(
				"source",
				match source {
					"(direct)" | "" => String::new(),
					source => referrer::source("", source),
				},
			);
			set("channel", dimension("sessionDefaultChannelGroup").into());
			set("utm_medium", dimension("sessionMedium").into());
			set("utm_campaign", dimension("sessionCampaignName").into());
			set("utm_content", dimension("sessionManualAdContent").into());
			set("utm_term", dimension("sessionGoogleAdsKeyword").into());
		}
		"pages" => {
			let host = dimension("hostName");
			set("hostname", host.strip_prefix("www.").unwrap_or(host).into());
			let path = dimension("pagePath");
			set("page", path.split(['?', '#']).next().unwrap_or(path).into());
			set("total_time_on_page", metric("userEngagementDuration"));
			set("total_time_on_page_visits", metric("sessions"));
		}
		"entry_pages" => {
			set("entry_page", dimension("landingPage").into());
			set("entrances", metric("sessions"));
		}
		"custom_events" => {
			let name = match dimension("eventName") {
				"file_download" => "File Download",
				"click" => "Outbound Link: Click",
				name => name,
			};
			set("name", name.into());
			set("link_url", dimension("linkUrl").into());
			set("events", metric("eventCount"));
		}
		"locations" => {
			set("country", dimension("countryId").into());
			set("region", dimension("region").into());
		}
		"devices" => {
			let device = dimension("deviceCategory");
			let mut letters = device.chars();
			set(
				"device",
				letters
					.next()
					.map(|first| first.to_uppercase().chain(letters).collect())
					.unwrap_or_default(),
			);
		}
		"browsers" => {
			let browser = match dimension("browser") {
				"User-Agent:Opera" => "Opera",
				"Mozilla Compatible Agent"
				| "Android Webview"
				| "Android Browser"
				| "Safari (in-app)" => "Mobile App",
				"User-Agent: Mozilla" => "Firefox",
				browser => browser,
			};
			set("browser", browser.into());
		}
		"operating_systems" => {
			let os = match dimension("operatingSystem") {
				"Macintosh" => "Mac",
				"Linux" => "GNU/Linux",
				os => os,
			};
			set("operating_system", os.into());
			set(
				"operating_system_version",
				dimension("operatingSystemVersion").into(),
			);
		}
		_ => {}
	}
	values
}
