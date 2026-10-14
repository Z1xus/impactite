use std::time::Instant;

use anyhow::{Context, bail};
use crater::{Db, Dim, Event};
use jiff::Timestamp;
use jiff::tz::TimeZone;
use serde::Deserialize;
use serde_json::Value;

use crate::app::Site;
use crate::cli::say;

const COLUMNS: &str = "toUnixTimestamp(timestamp) AS ts, name, user_id, session_id, hostname, pathname, \
	referrer, referrer_source, utm_medium, utm_source, utm_campaign, utm_content, utm_term, \
	acquisition_channel, country_code, subdivision1_code, screen_size, browser, browser_version, \
	operating_system, operating_system_version, `meta.key` AS keys, `meta.value` AS values, \
	scroll_depth, engagement_time";

#[derive(Deserialize)]
struct Row {
	ts: i64,
	name: String,
	user_id: Value,
	session_id: Value,
	hostname: String,
	pathname: String,
	referrer: String,
	referrer_source: String,
	utm_medium: String,
	utm_source: String,
	utm_campaign: String,
	utm_content: String,
	utm_term: String,
	acquisition_channel: String,
	country_code: String,
	subdivision1_code: String,
	screen_size: String,
	browser: String,
	browser_version: String,
	operating_system: String,
	operating_system_version: String,
	keys: Vec<String>,
	values: Vec<String>,
	interactive: u8,
	scroll_depth: u8,
	engagement_time: u32,
}

pub async fn import(
	db: &Db,
	site: &Site,
	url: &str,
	plausible_site: u64,
) -> anyhow::Result<(u64, Option<i64>)> {
	let tz = TimeZone::get(&site.timezone).unwrap_or(TimeZone::UTC);
	// events_v2 drops the interactive flag, so it is recovered from bounced sessions
	let query = format!(
		"SELECT {COLUMNS}, name = 'pageview' OR session_id NOT IN (SELECT session_id FROM sessions_v2 \
		WHERE site_id = {plausible_site} GROUP BY session_id HAVING sum(sign * is_bounce) > 0) AS interactive \
		FROM events_v2 WHERE site_id = {plausible_site} ORDER BY timestamp FORMAT JSONEachRow"
	);
	let mut response = reqwest::Client::new()
		.post(url)
		.body(query)
		.send()
		.await
		.context("connect to clickhouse")?;
	if !response.status().is_success() {
		bail!(
			"clickhouse: {}",
			response.text().await.unwrap_or_default().trim()
		);
	}

	let started = Instant::now();
	let mut buffer = Vec::new();
	let mut count = 0u64;
	let mut first = None;
	while let Some(chunk) = response.chunk().await? {
		buffer.extend_from_slice(&chunk);
		let mut start = 0;
		while let Some(end) = buffer[start..].iter().position(|&b| b == b'\n') {
			let line = &buffer[start..start + end];
			start += end + 1;
			let row: Row = serde_json::from_slice(line).context("unexpected clickhouse row")?;
			first.get_or_insert(row.ts);
			insert(db, event(site.id, &tz, row)?);
			count += 1;
			if count.is_multiple_of(250_000) {
				say(
					'*',
					&format!("{count} events  |  {:.1?}", started.elapsed()),
				);
			}
		}
		buffer.drain(..start);
	}
	Ok((count, first))
}

fn event(site: u64, tz: &TimeZone, row: Row) -> anyhow::Result<Event> {
	let id = |value: &Value| match value {
		Value::String(text) => text.parse().ok(),
		Value::Number(number) => number.as_u64(),
		_ => None,
	};
	let offset = tz
		.to_offset(Timestamp::from_second(row.ts).context("bad timestamp")?)
		.seconds();
	let mut event = Event {
		site,
		ts: row.ts,
		offset,
		visitor: id(&row.user_id).context("bad user_id")?,
		session: id(&row.session_id).context("bad session_id")?,
		interactive: row.interactive == 1,
		scroll_depth: (row.scroll_depth <= 100).then_some(row.scroll_depth),
		engagement_ms: row.engagement_time,
		props: row.keys.into_iter().zip(row.values).collect(),
		..Event::default()
	};
	for (dim, value) in [
		(Dim::Name, row.name),
		(Dim::Hostname, row.hostname),
		(Dim::Page, row.pathname),
		(Dim::Referrer, row.referrer),
		(Dim::Source, row.referrer_source),
		(Dim::Channel, row.acquisition_channel),
		(Dim::UtmSource, row.utm_source),
		(Dim::UtmMedium, row.utm_medium),
		(Dim::UtmCampaign, row.utm_campaign),
		(Dim::UtmContent, row.utm_content),
		(Dim::UtmTerm, row.utm_term),
		(Dim::Country, row.country_code.trim_matches('\0').to_owned()),
		(Dim::Region, row.subdivision1_code),
		(Dim::Device, row.screen_size),
		(Dim::Browser, row.browser),
		(Dim::BrowserVersion, row.browser_version),
		(Dim::Os, row.operating_system),
		(Dim::OsVersion, row.operating_system_version),
	] {
		event.dims[dim] = value;
	}
	Ok(event)
}

fn insert(db: &Db, event: Event) {
	let mut event = Box::new(event);
	while let Err(back) = db.insert(event) {
		event = back;
		std::thread::yield_now();
	}
}
