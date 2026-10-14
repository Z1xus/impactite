use std::thread;

use crater::{Db, Dim, Event};
use jiff::Timestamp;
use jiff::tz::TimeZone;

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
	let tz = TimeZone::get(timezone).unwrap_or(TimeZone::UTC);
	let now = Timestamp::now();
	let offset = i64::from(tz.to_offset(now).seconds());
	let today = (now.as_second() + offset).div_euclid(86_400);
	let mut rng =
		Rng(blake3::hash(domain.as_bytes()).as_bytes()[0] as u64 * 0x9e37_79b9 + 0x1234_5678);
	let mut inserted = 0;
	let hours: Vec<(i64, u32)> = (0..24).zip(HOURS).collect();

	for day in today - days + 1..=today {
		let age = (today - day) as f64;
		let weekday = (day + 4).rem_euclid(7);
		let weekend = if weekday >= 5 { 0.62 } else { 1.0 };
		let growth = 1.0 - age / (days as f64 * 1.6);
		let spike = if today - day == 9 { 3.4 } else { 1.0 };
		let visits = (420.0 * weekend * growth * spike * (0.88 + rng.unit() * 0.24)) as u64;
		let mut visitors: Vec<u64> = Vec::new();

		for _ in 0..visits {
			let visitor = match visitors.is_empty() || rng.unit() > 0.12 {
				true => {
					let id = rng.next();
					visitors.push(id);
					id
				}
				false => visitors[rng.below(visitors.len() as u64) as usize],
			};
			let session = rng.next();
			let hour = rng.pick(&hours, |h| h.1).0;
			let mut time = day * 86_400 + hour * 3600 + rng.below(3600) as i64;
			let &(raw_referrer, utm, _) = rng.pick(SOURCES, |s| s.2);
			let &(device, browser, version, os, os_version, _) = rng.pick(AGENTS, |a| a.5);
			let country = rng.pick(COUNTRIES, |c| c.1).0;
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
				let mut names = vec![("pageview", page)];
				if page == "/pricing" && rng.unit() < 0.18 {
					names.push(("Signup", page));
				}
				for (name, path) in names {
					if time - offset > now.as_second() {
						continue;
					}
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
					] {
						event.dims[dim] = value.to_owned();
					}
					let mut event = Box::new(event);
					while let Err(back) = db.insert(event) {
						event = back;
						thread::yield_now();
					}
					inserted += 1;
					time += 4 + rng.below(40) as i64;
				}
				time += 15 + rng.below(160) as i64;
			}
		}
	}
	inserted
}
