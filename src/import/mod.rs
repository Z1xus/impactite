use std::path::Path;

use anyhow::bail;
use crater::{Db, Event};
use jiff::Timestamp;
use jiff::tz::TimeZone;

use crate::app::{App, Site};

pub mod clickhouse;
pub mod goatcounter;
pub mod plausible;

fn insert(db: &Db, event: Event) {
	let mut event = Box::new(event);
	while let Err(back) = db.insert(event) {
		event = back;
		std::thread::yield_now();
	}
}

pub fn plausible_zip(db: &Db, app: &App, site: &Site, path: &Path) -> anyhow::Result<u64> {
	let tz = TimeZone::get(&site.timezone)?;
	let before = Timestamp::from_second(site.created_at)?.to_zoned(tz).date();
	let Some((first, last)) = plausible::import(db, site, path, before)? else {
		bail!("No days before {before} in this export.");
	};
	app.add_import(site.id, "plausible", &first.to_string(), &last.to_string())?;
	Ok(first.until(last)?.get_days() as u64 + 1)
}

pub fn goatcounter_csv(db: &Db, app: &App, site: &Site, path: &Path) -> anyhow::Result<u64> {
	let (count, first) = goatcounter::import(db, site, path)?;
	if let Some(first) = first {
		app.backdate(site.id, first)?;
	}
	Ok(count)
}
