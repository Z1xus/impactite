use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use anyhow::{Context, bail};
use crater::{Db, Dim, Event};
use flate2::read::MultiGzDecoder;
use jiff::Timestamp;
use jiff::civil::DateTime;
use jiff::tz::TimeZone;

use super::insert;
use crate::app::Site;
use crate::ingest::referrer::{self, Traffic};

pub fn import(
	db: &Db,
	site: &Site,
	path: &Path,
	titles: &mut HashMap<String, String>,
) -> anyhow::Result<(u64, Option<i64>)> {
	let file = BufReader::new(File::open(path).context("open export")?);
	let reader: Box<dyn Read> = match path.extension().is_some_and(|ext| ext == "gz") {
		true => Box::new(MultiGzDecoder::new(file)),
		false => Box::new(file),
	};
	let mut csv = csv::Reader::from_reader(reader);
	if !csv
		.headers()?
		.get(0)
		.is_some_and(|first| first.starts_with('2'))
	{
		bail!("only goatcounter csv export version 2 is supported");
	}
	let tz = TimeZone::get(&site.timezone).unwrap_or(TimeZone::UTC);
	let (mut count, mut first) = (0, None);
	for record in csv.records() {
		let record = record?;
		let field = |i| record.get(i).unwrap_or_default();
		if !matches!(field(7), "" | "0") {
			continue;
		}
		let ts = timestamp(field(13)).with_context(|| format!("bad date {:?}", field(13)))?;
		first = Some(first.unwrap_or(ts).min(ts));
		let session =
			u64::from_le_bytes(blake3::hash(field(6).as_bytes()).as_bytes()[..8].try_into()?);
		let mut event = Event {
			site: site.id,
			ts,
			offset: tz.to_offset(Timestamp::from_second(ts)?).seconds(),
			visitor: session,
			session,
			interactive: true,
			..Event::default()
		};

		let (referrer, host, tagged) = match field(9) {
			"h" => {
				let host = field(8).split('/').next().unwrap_or_default();
				(
					format!("https://{}", field(8)),
					host.strip_prefix("www.").unwrap_or(host),
					"",
				)
			}
			"o" => (field(8).to_owned(), "", field(8)),
			"g" | "c" => (String::new(), "", field(8)),
			_ => (String::new(), "", ""),
		};
		let channel = referrer::channel(&Traffic {
			referrer_host: host,
			utm_source: tagged,
			utm_medium: "",
			click_id: false,
		});
		let device = match field(10)
			.split(',')
			.next()
			.and_then(|w| w.parse::<u32>().ok())
		{
			None | Some(0) => "",
			Some(..576) => "Mobile",
			Some(..992) => "Tablet",
			Some(_) => "Desktop",
		};
		let (browser, browser_version) = split(field(4));
		let (os, os_version) = split(field(5));
		let location = field(11);
		let (name, page) = match matches!(field(2), "true" | "1") {
			true => (field(0), ""),
			false => ("pageview", field(0)),
		};
		if !page.is_empty() && !field(1).is_empty() {
			titles.insert(page.to_owned(), field(1).to_owned());
		}
		for (dim, value) in [
			(Dim::Name, name),
			(Dim::Hostname, site.domain.as_str()),
			(Dim::Page, page),
			(Dim::Referrer, referrer.as_str()),
			(Dim::Source, referrer::source(host, tagged).as_str()),
			(Dim::Channel, channel),
			(Dim::UtmSource, tagged),
			(Dim::Country, location.get(..2).unwrap_or_default()),
			(
				Dim::Region,
				if location.contains('-') { location } else { "" },
			),
			(Dim::Device, device),
			(Dim::Browser, browser),
			(Dim::BrowserVersion, browser_version),
			(Dim::Os, os),
			(Dim::OsVersion, os_version),
		] {
			event.dims[dim] = value.to_owned();
		}
		insert(db, event);
		count += 1;
	}
	Ok((count, first))
}

pub(super) fn timestamp(raw: &str) -> Option<i64> {
	let ts = raw.parse::<Timestamp>().ok().or_else(|| {
		let civil: DateTime = raw
			.trim_end_matches("+00")
			.trim_end_matches('Z')
			.parse()
			.ok()?;
		civil
			.to_zoned(TimeZone::UTC)
			.ok()
			.map(|zoned| zoned.timestamp())
	})?;
	Some(ts.as_second())
}

fn split(value: &str) -> (&str, &str) {
	let value = value.trim();
	let (name, version) = value
		.rsplit_once(' ')
		.filter(|(_, version)| version.starts_with(|c: char| c.is_ascii_digit()))
		.unwrap_or((value, ""));
	let name = match name {
		"Edge" => "Microsoft Edge",
		"macOS" => "Mac",
		"Linux" => "GNU/Linux",
		name => name,
	};
	(name, version)
}
