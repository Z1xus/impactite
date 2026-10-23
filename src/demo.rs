use std::sync::Arc;
use std::thread;

use crater::{Db, Dim, Event};
use jiff::Timestamp;
use jiff::tz::TimeZone;

use crate::app::App;
use crate::ingest::referrer;

const SOURCES: &[(&str, &str, u32)] = &[
	("https://www.google.com/", "", 38),
	("", "", 25),
	("https://news.ycombinator.com/item?id=41", "", 6),
	("https://www.reddit.com/r/selfhosted/", "", 5),
	("https://t.co/x1", "", 5),
	("https://github.com/impactite", "", 4),
	("https://chatgpt.com/", "", 4),
	("https://duckduckgo.com/", "", 3),
	("https://www.bing.com/", "", 3),
	("https://www.linkedin.com/", "", 2),
	("", "newsletter", 3),
	("https://www.perplexity.ai/", "", 2),
];
const PAGES: &[(&str, u32)] = &[
	("/", 30),
	("/pricing", 12),
	("/blog/crater", 9),
	("/blog/microseconds", 7),
	("/blog/dithering", 5),
	("/blog/self-hosting", 4),
	("/docs", 8),
	("/docs/install", 7),
	("/docs/import", 5),
	("/docs/api", 4),
	("/changelog", 5),
	("/about", 4),
];
const COUNTRIES: &[(&str, u32)] = &[
	("US", 28),
	("DE", 11),
	("GB", 7),
	("IN", 7),
	("FR", 5),
	("NL", 4),
	("CA", 4),
	("UA", 4),
	("PL", 3),
	("BR", 3),
	("JP", 3),
	("SE", 2),
	("ES", 2),
	("AU", 2),
	("CH", 2),
	("KR", 2),
];
const AGENTS: &[(&str, &str, &str, &str, &str, u32)] = &[
	("Desktop", "Chrome", "141.0", "Windows", "10", 26),
	("Desktop", "Chrome", "141.0", "Mac", "10.15", 10),
	("Desktop", "Firefox", "144.0", "GNU/Linux", "", 7),
	("Desktop", "Safari", "26.0", "Mac", "10.15", 8),
	("Desktop", "Microsoft Edge", "141.0", "Windows", "10", 6),
	("Mobile", "Safari", "26.0", "iOS", "26.0", 19),
	("Mobile", "Chrome", "141.0", "Android", "15", 16),
	("Mobile", "Samsung Internet", "28.0", "Android", "14", 3),
	("Tablet", "Safari", "26.0", "iPadOS", "26.0", 3),
	("Tablet", "Chrome", "141.0", "Android", "14", 2),
];
const HOURS: [u32; 24] = [
	3, 2, 2, 1, 1, 2, 4, 6, 9, 11, 12, 12, 11, 12, 12, 12, 11, 10, 10, 11, 10, 8, 6, 4,
];

struct Rng(u64);

impl Rng {
	fn next(&mut self) -> u64 {
		self.0 ^= self.0 << 13;
		self.0 ^= self.0 >> 7;
		self.0 ^= self.0 << 17;
		self.0
	}

	fn unit(&mut self) -> f64 {
		(self.next() >> 11) as f64 / (1u64 << 53) as f64
	}

	fn below(&mut self, n: u64) -> u64 {
		self.next() % n.max(1)
	}

	fn pick<'a, T>(&mut self, items: &'a [T], weight: impl Fn(&T) -> u32) -> &'a T {
		let total: u64 = items.iter().map(|item| u64::from(weight(item))).sum();
		let mut roll = self.below(total);
		for item in items {
			let w = u64::from(weight(item));
			if roll < w {
				return item;
			}
			roll -= w;
		}
		&items[0]
	}
}

pub fn seed(db: &Db, site: u64, domain: &str, timezone: &str, days: i64) -> usize {
	let now = Timestamp::now().as_second();
	let today = local_day(timezone, now);
	(today - days + 1..=today)
		.map(|day| insert(db, generate(site, domain, timezone, day), 0, now))
		.sum()
}

// keeps a demo site current: the events of each day arrive as their time comes, a restart goes on where it stopped
pub async fn live(db: Arc<Db>, app: Arc<App>, site: u64, domain: String, timezone: String) {
	let key = format!("demo:{site}");
	let mut last = app
		.setting(&key)
		.and_then(|value| value.parse().ok())
		.unwrap_or_else(|| Timestamp::now().as_second());
	let mut tick = tokio::time::interval(std::time::Duration::from_secs(30));
	loop {
		tick.tick().await;
		let now = Timestamp::now().as_second();
		for day in local_day(&timezone, last)..=local_day(&timezone, now) {
			insert(&db, generate(site, &domain, &timezone, day), last, now);
		}
		last = now;
		if let Err(err) = app.set_setting(&key, &now.to_string()) {
			tracing::warn!(%err, "demo");
		}
	}
}

fn local_day(timezone: &str, at: i64) -> i64 {
	let tz = TimeZone::get(timezone).unwrap_or(TimeZone::UTC);
	let offset = Timestamp::from_second(at).map_or(0, |ts| i64::from(tz.to_offset(ts).seconds()));
	(at + offset).div_euclid(86_400)
}

fn insert(db: &Db, events: Vec<Event>, after: i64, until: i64) -> usize {
	let mut inserted = 0;
	for event in events
		.into_iter()
		.filter(|event| (after + 1..=until).contains(&event.ts))
	{
		let mut event = Box::new(event);
		while let Err(back) = db.insert(event) {
			event = back;
			thread::yield_now();
		}
		inserted += 1;
	}
	inserted
}

// one day of the demo site, the same every time it is made
fn generate(site: u64, domain: &str, timezone: &str, day: i64) -> Vec<Event> {
	let tz = TimeZone::get(timezone).unwrap_or(TimeZone::UTC);
	let noon = Timestamp::from_second(day * 86_400 + 43_200).unwrap_or_else(|_| Timestamp::now());
	let offset = i64::from(tz.to_offset(noon).seconds());
	let mut rng = Rng(u64::from_le_bytes(
		blake3::hash(format!("{domain}:{day}").as_bytes()).as_bytes()[..8]
			.try_into()
			.unwrap_or_default(),
	) | 1);
	let mut events = Vec::new();
	let hours: Vec<(i64, u32)> = (0..24).zip(HOURS).collect();
	let weekday = (day + 4).rem_euclid(7);
	let weekend = if weekday >= 5 { 0.62 } else { 1.0 };
	let wave = 0.85 + 0.15 * (day as f64 / 30.0).sin();
	// a front page day now and then
	let spike = if day.rem_euclid(41) == 0 { 3.4 } else { 1.0 };
	let visits = (420.0 * weekend * wave * spike * (0.88 + rng.unit() * 0.24)) as u64;
	let mut visitors: Vec<u64> = Vec::new();
	for _ in 0..visits {
		let visitor = if visitors.is_empty() || rng.unit() > 0.12 {
			let id = rng.next();
			visitors.push(id);
			id
		} else {
			visitors[rng.below(visitors.len() as u64) as usize]
		};
		let session = rng.next();
		let hour = rng.pick(&hours, |h| h.1).0;
		let mut time = day * 86_400 + hour * 3600 + rng.below(3600) as i64;
		let &(raw_referrer, utm, _) = rng.pick(SOURCES, |s| s.2);
		let &(device, browser, version, os, os_version, _) = rng.pick(AGENTS, |a| a.5);
		let country = rng.pick(COUNTRIES, |c| c.1).0;
		let language = match country {
			"DE" => "de",
			"FR" => "fr",
			"NL" => "nl",
			"UA" => "uk",
			"PL" => "pl",
			"BR" => "pt",
			"JP" => "ja",
			"SE" => "sv",
			"ES" => "es",
			_ => "en",
		};
		let host = raw_referrer
			.split("//")
			.nth(1)
			.and_then(|rest| rest.split('/').next())
			.map(|host| host.trim_start_matches("www."))
			.unwrap_or_default();
		let source = referrer::source(host, utm);
		let channel = referrer::channel(&referrer::Traffic {
			referrer_host: host,
			utm_source: utm,
			utm_medium: if utm.is_empty() { "" } else { "email" },
			click_id: false,
		});
		let pages = match rng.unit() {
			x if x < 0.55 => 1,
			x if x < 0.75 => 2,
			_ => 3 + rng.below(4),
		};
		let entry = match host {
			"news.ycombinator.com" => "/blog/crater",
			_ => rng.pick(PAGES, |p| p.1).0,
		};

		for n in 0..pages {
			let page = if n == 0 {
				entry
			} else {
				rng.pick(PAGES, |p| p.1).0
			};
			let mut names = vec![("pageview", page), ("engagement", page)];
			if page == "/pricing" && rng.unit() < 0.18 {
				names.push(("Signup", page));
			}
			for (name, path) in names {
				let mut event = Event {
					site,
					ts: time - offset,
					offset: offset as i32,
					visitor,
					session,
					interactive: true,
					..Event::default()
				};
				for (dim, value) in [
					(Dim::Name, name),
					(Dim::Hostname, domain),
					(Dim::Page, path),
					(Dim::Referrer, if n == 0 { host } else { "" }),
					(Dim::Source, source.as_str()),
					(Dim::Channel, channel),
					(Dim::UtmSource, utm),
					(Dim::Country, country),
					(Dim::Device, device),
					(Dim::Browser, browser),
					(Dim::BrowserVersion, version),
					(Dim::Os, os),
					(Dim::OsVersion, os_version),
					(Dim::Language, language),
				] {
					event.dims[dim] = value.to_owned();
				}
				if name == "engagement" {
					event.interactive = false;
					event.engagement_ms = 2_000 + rng.below(90_000) as u32;
					event.scroll_depth = Some(10 + rng.below(91) as u8);
				}
				if name == "Signup" {
					let plan = rng.pick(&[("free", 6), ("pro", 3), ("team", 1)], |p| p.1).0;
					event.props = vec![("plan".into(), plan.into())];
					event.revenue = match plan {
						"pro" => Some((1900, "EUR".into())),
						"team" => Some((4900, "EUR".into())),
						_ => None,
					};
				}
				events.push(event);
				time += 4 + rng.below(40) as i64;
			}
			time += 15 + rng.below(160) as i64;
		}
	}
	events
}
