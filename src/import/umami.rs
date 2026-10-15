use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use anyhow::{Context, bail};
use crater::{Db, Dim, Event};
use jiff::Timestamp;
use jiff::tz::TimeZone;

use super::goatcounter::timestamp;
use super::insert;
use crate::app::Site;
use crate::ingest::referrer::{self, Traffic};

// a csv of umami's website_event joined with its session, one row per event
pub fn import(
	db: &Db,
	site: &Site,
	path: &Path,
	titles: &mut HashMap<String, String>,
) -> anyhow::Result<(u64, Option<i64>)> {
	let mut csv =
		csv::Reader::from_reader(BufReader::new(File::open(path).context("open export")?));
	let headers = csv.headers()?.clone();
	let column = |name: &str| headers.iter().position(|header| header == name);
	let (Some(created), Some(session)) = (column("created_at"), column("session_id")) else {
		bail!("This isn't an Umami export. It needs the created_at and session_id columns.");
	};
	let tz = TimeZone::get(&site.timezone).unwrap_or(TimeZone::UTC);
	let hash = |text: &str| -> anyhow::Result<u64> {
		Ok(u64::from_le_bytes(
			blake3::hash(text.as_bytes()).as_bytes()[..8].try_into()?,
		))
	};
	let (mut count, mut first) = (0, None);
	for record in csv.records() {
		let record = record?;
		let get = |name: &str| {
			column(name)
				.and_then(|i| record.get(i))
				.unwrap_or_default()
				.trim()
		};
		let raw = record.get(created).unwrap_or_default();
		let ts = timestamp(raw).with_context(|| format!("bad date {raw:?}"))?;
		first = Some(first.unwrap_or(ts).min(ts));
		let visitor = hash(record.get(session).unwrap_or_default())?;
		let mut event = Event {
			site: site.id,
			ts,
			offset: tz.to_offset(Timestamp::from_second(ts)?).seconds(),
			visitor,
			session: match get("visit_id") {
				"" => visitor,
				visit => hash(visit)?,
			},
			interactive: true,
			..Event::default()
		};

		let host = get("referrer_domain");
		let host = host.strip_prefix("www.").unwrap_or(host);
		let referrer = match host {
			"" => String::new(),
			host => format!("https://{host}{}", get("referrer_path")),
		};
		let (tagged, medium) = (get("utm_source"), get("utm_medium"));
		let channel = referrer::channel(&Traffic {
			referrer_host: host,
			utm_source: tagged,
			utm_medium: medium,
			click_id: false,
		});
		let custom = get("event_type") == "2";
		let page = get("url_path");
		if !custom && !page.is_empty() && !get("page_title").is_empty() {
			titles.insert(page.to_owned(), get("page_title").to_owned());
		}
		let (os, os_version) = match get("os") {
			"Mac OS" => ("Mac", ""),
			"Android OS" => ("Android", ""),
			"Linux" => ("GNU/Linux", ""),
			os => match os.strip_prefix("Windows ") {
				Some(version) => ("Windows", version),
				None => (os, ""),
			},
		};
		let browser = match get("browser") {
			"chrome" | "crios" => "Chrome",
			"firefox" | "fxios" => "Firefox",
			"safari" | "ios" => "Safari",
			"edge" | "edge-chromium" | "edge-ios" => "Microsoft Edge",
			"opera" | "opera-mini" => "Opera",
			"samsung" => "Samsung Internet",
			"yandexbrowser" => "Yandex Browser",
			"ios-webview" | "chromium-webview" | "facebook" | "instagram" => "Mobile App",
			other => other,
		};
		let device = match get("device") {
			"desktop" | "laptop" => "Desktop",
			"mobile" => "Mobile",
			"tablet" => "Tablet",
			_ => "",
		};
		let host_name = get("hostname");
		for (dim, value) in [
			(
				Dim::Name,
				if custom {
					get("event_name")
				} else {
					"pageview"
				},
			),
			(
				Dim::Hostname,
				if host_name.is_empty() {
					site.domain.as_str()
				} else {
					host_name
				},
			),
			(Dim::Page, page),
			(Dim::Referrer, referrer.as_str()),
			(Dim::Source, referrer::source(host, tagged).as_str()),
			(Dim::Channel, channel),
			(Dim::UtmSource, tagged),
			(Dim::UtmMedium, medium),
			(Dim::UtmCampaign, get("utm_campaign")),
			(Dim::UtmContent, get("utm_content")),
			(Dim::UtmTerm, get("utm_term")),
			(Dim::Country, get("country")),
			(Dim::Region, get("region")),
			(Dim::City, get("city")),
			(Dim::Device, device),
			(Dim::Browser, browser),
			(Dim::Os, os),
			(Dim::OsVersion, os_version),
			(Dim::Language, get("language")),
		] {
			event.dims[dim] = value.to_owned();
		}
		insert(db, event);
		count += 1;
	}
	Ok((count, first))
}
