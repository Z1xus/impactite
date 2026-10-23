use std::sync::Arc;

use jiff::civil::Date;
use jiff::tz::TimeZone;
use jiff::{Timestamp, ToSpan, Zoned};
use serde_json::json;

use crate::app::lock;
use crate::stats::run;
use crate::uptime::Uptime;
use crate::web::Web;

const WEEKS: i64 = 4;
const FLOOR: f64 = 5.0;
const LONGEST: i64 = 3 * 86_400;

pub type Span = (i64, i64, Option<i64>);

impl Uptime {
	fn remembered(&self, id: i64) -> Option<Option<u32>> {
		lock(&self.missed).get(&id).copied()
	}
}

fn hours(web: &Web, domain: &str, day: Date) -> Option<[f64; 24]> {
	let date = day.to_string();
	let answer = run(
		&web.stats,
		json!({
			"site_id": domain,
			"metrics": ["visits"],
			"date_range": [date, date],
			"dimensions": ["time:hour"],
		}),
	)
	.ok()?;
	let mut out = [0.0; 24];
	for (slot, row) in out.iter_mut().zip(answer["results"].as_array()?) {
		*slot = row["metrics"][0].as_f64().unwrap_or(0.0);
	}
	Some(out)
}

fn seconds(at: &Zoned) -> f64 {
	let time = at.time();
	f64::from(time.hour()) * 3600.0 + f64::from(time.minute()) * 60.0 + f64::from(time.second())
}

fn visits(web: &Web, domain: &str, from: &Zoned, to: &Zoned) -> Option<f64> {
	let mut total = 0.0;
	let mut day = from.date();
	while day <= to.date() {
		let table = hours(web, domain, day)?;
		let low = if day == from.date() {
			seconds(from)
		} else {
			0.0
		};
		let high = if day == to.date() {
			seconds(to)
		} else {
			86_400.0
		};
		for (hour, count) in table.iter().enumerate() {
			let start = hour as f64 * 3600.0;
			total += count * (high.min(start + 3600.0) - low.max(start)).max(0.0) / 3600.0;
		}
		day = day.tomorrow().ok()?;
	}
	Some(total)
}

fn friendly(n: f64) -> Option<u32> {
	let step = match n {
		..10.0 => 1.0,
		..100.0 => 5.0,
		..1000.0 => 10.0,
		_ => 100.0,
	};
	let rounded = (n / step).round() * step;
	(rounded >= 1.0).then_some(rounded as u32)
}

fn estimate(web: &Web, domain: &str, tz: &TimeZone, started: i64, ended: i64) -> Option<u32> {
	if ended - started > LONGEST {
		return None;
	}
	let from = Timestamp::from_second(started).ok()?.to_zoned(tz.clone());
	let to = Timestamp::from_second(ended).ok()?.to_zoned(tz.clone());
	let actual = visits(web, domain, &from, &to)?;
	let mut before = Vec::new();
	for week in 1..=WEEKS {
		let (low, high) = (
			from.checked_sub(week.weeks()).ok()?,
			to.checked_sub(week.weeks()).ok()?,
		);
		let count = visits(web, domain, &low, &high)?;
		if count <= 0.0 {
			return None;
		}
		before.push(count);
	}
	before.sort_by(f64::total_cmp);
	let expected = (before[1] + before[2]) / 2.0;
	if expected < FLOOR {
		return None;
	}
	friendly(expected - actual)
}

pub async fn visits_missed(
	web: &Arc<Web>,
	site: Option<i64>,
	spans: Vec<Span>,
) -> Vec<Option<u32>> {
	let none = vec![None; spans.len()];
	let Some(domain) = site.and_then(|id| web.app.monitor_domain(id)) else {
		return none;
	};
	let Ok(Some(found)) = web.app.site(&domain) else {
		return none;
	};
	let (web, at) = (web.clone(), crate::app::now());
	let work = tokio::task::spawn_blocking(move || {
		let tz = TimeZone::get(&found.timezone).unwrap_or(TimeZone::UTC);
		spans
			.into_iter()
			.map(|(id, started, ended)| {
				if let Some(known) = web.uptime.remembered(id) {
					return known;
				}
				let value = estimate(&web, &domain, &tz, started, ended.unwrap_or(at));
				if ended.is_some() {
					lock(&web.uptime.missed).insert(id, value);
				}
				value
			})
			.collect::<Vec<_>>()
	});
	work.await.unwrap_or(none)
}
