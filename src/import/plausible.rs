use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use anyhow::Context;
use crater::{Db, Dim, Field, Metrics};
use jiff::civil::Date;

use crate::app::Site;
use crate::ingest::referrer::{self, Traffic};
use crate::stats::day_number;

#[derive(Default)]
pub(super) struct Day {
	total: Metrics,
	rows: Vec<(Field, String, Metrics)>,
}

impl Day {
	fn add<'a>(&mut self, metrics: Metrics, values: impl IntoIterator<Item = (Field, &'a str)>) {
		for (field, value) in values {
			if !(matches!(field, Field::Prop(_) | Field::Entry | Field::Exit) && value.is_empty()) {
				self.rows.push((field, value.to_owned(), metrics));
			}
		}
	}
}

pub fn import(
	db: &Db,
	site: &Site,
	path: &Path,
	before: Date,
) -> anyhow::Result<Option<(Date, Date)>> {
	let mut archive = zip::ZipArchive::new(File::open(path).context("open export")?)?;
	let mut days: BTreeMap<Date, Day> = BTreeMap::new();
	for i in 0..archive.len() {
		let file = archive.by_index(i)?;
		let Some(table) = table(&file.name()?).map(str::to_owned) else {
			continue;
		};
		let mut csv = csv::Reader::from_reader(file.take(super::LIMIT));
		let headers = csv.headers()?.clone();
		for record in csv.records() {
			let record = record?;
			let get = |name: &str| {
				headers
					.iter()
					.position(|header| header == name)
					.and_then(|i| record.get(i))
					.unwrap_or_default()
					.trim_matches('\0')
			};
			let date: Date = get("date")
				.parse()
				.with_context(|| format!("bad date in {table}"))?;
			if date < before {
				add(days.entry(date).or_default(), &table, &get);
			}
		}
	}
	write(db, site, &days)
}

pub(super) fn write(
	db: &Db,
	site: &Site,
	days: &BTreeMap<Date, Day>,
) -> anyhow::Result<Option<(Date, Date)>> {
	for (&date, day) in days {
		db.import(site.id, day_number(date), day.total, &day.rows)?;
	}
	Ok(days
		.first_key_value()
		.zip(days.last_key_value())
		.map(|((&first, _), (&last, _))| (first, last)))
}

pub(super) fn add<'a>(day: &mut Day, table: &str, get: &dyn Fn(&str) -> &'a str) {
	let num = |name: &str| get(name).parse::<u64>().unwrap_or_default();
	let visit = Metrics {
		visitors: num("visitors"),
		visits: num("visits"),
		pageviews: num("pageviews"),
		events: num("pageviews"),
		bounces: num("bounces"),
		duration: num("visit_duration"),
		..Metrics::default()
	};
	let dim = Field::Dim;
	match table {
		"visitors" => {
			day.total.merge(&Metrics {
				exits: visit.visits,
				..visit
			});
			day.add(visit, [(dim(Dim::Name), "pageview")]);
		}
		"custom_events" => {
			let metrics = Metrics {
				visitors: num("visitors"),
				events: num("events"),
				..Metrics::default()
			};
			day.total.events += metrics.events;
			day.add(
				metrics,
				[
					(dim(Dim::Name), get("name")),
					(Field::Prop("url".into()), get("link_url")),
					(Field::Prop("path".into()), get("path")),
				],
			);
		}
		"sources" => {
			let channel = match get("channel") {
				"" => referrer::channel(&Traffic {
					referrer_host: "",
					utm_source: get("source"),
					utm_medium: get("utm_medium"),
					click_id: false,
				}),
				channel => channel,
			};
			day.add(
				visit,
				[
					(dim(Dim::Source), get("source")),
					(dim(Dim::Referrer), get("referrer")),
					(dim(Dim::Channel), channel),
					(dim(Dim::UtmSource), get("utm_source")),
					(dim(Dim::UtmMedium), get("utm_medium")),
					(dim(Dim::UtmCampaign), get("utm_campaign")),
					(dim(Dim::UtmContent), get("utm_content")),
					(dim(Dim::UtmTerm), get("utm_term")),
				],
			);
		}
		"pages" => {
			// plausible exports time on page in seconds
			let page = Metrics {
				engaged: num("total_time_on_page") * 1000,
				engaged_views: num("total_time_on_page_visits"),
				scroll: num("total_scroll_depth"),
				scrolled: num("total_scroll_depth_visits"),
				..visit
			};
			day.add(page, [(dim(Dim::Page), get("page"))]);
			day.add(visit, [(dim(Dim::Hostname), get("hostname"))]);
		}
		"entry_pages" => day.add(
			Metrics {
				visits: num("entrances"),
				..visit
			},
			[(Field::Entry, get("entry_page"))],
		),
		"exit_pages" => {
			day.add(
				Metrics {
					visits: num("exits"),
					..visit
				},
				[(Field::Exit, get("exit_page"))],
			);
			if !get("exit_page").is_empty() {
				day.add(
					Metrics {
						exits: num("exits"),
						..Metrics::default()
					},
					[(dim(Dim::Page), get("exit_page"))],
				);
			}
		}
		"locations" => day.add(
			visit,
			[
				(dim(Dim::Country), get("country")),
				(dim(Dim::Region), get("region")),
			],
		),
		"devices" => day.add(visit, [(dim(Dim::Device), get("device"))]),
		"browsers" => day.add(
			visit,
			[
				(dim(Dim::Browser), get("browser")),
				(dim(Dim::BrowserVersion), get("browser_version")),
			],
		),
		"operating_systems" => day.add(
			visit,
			[
				(dim(Dim::Os), get("operating_system")),
				(dim(Dim::OsVersion), get("operating_system_version")),
			],
		),
		_ => {}
	}
}

fn table(name: &str) -> Option<&str> {
	let name = name
		.rsplit('/')
		.next()?
		.strip_prefix("imported_")?
		.strip_suffix(".csv")?;
	name.rsplitn(3, '_').nth(2)
}
