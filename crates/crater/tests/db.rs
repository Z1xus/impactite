use std::fs;
use std::path::PathBuf;

use crater::{Db, Dim, Event, Metrics};

const SITE: u64 = 7;
const DAY: i32 = 20_000;

struct TempDir(PathBuf);

impl TempDir {
	fn new(name: &str) -> std::io::Result<Self> {
		let path = std::env::temp_dir().join(format!("crater-db-{name}-{}", std::process::id()));
		let _ = fs::remove_dir_all(&path);
		fs::create_dir_all(&path)?;
		Ok(Self(path))
	}
}

impl Drop for TempDir {
	fn drop(&mut self) {
		let _ = fs::remove_dir_all(&self.0);
	}
}

fn event(session: u64, visitor: u64, time: i64, name: &str, page: &str, source: &str) -> Event {
	let mut event = Event {
		site: SITE,
		ts: i64::from(DAY) * 86_400 + time - 3600,
		offset: 3600,
		visitor,
		session,
		interactive: true,
		..Event::default()
	};
	event.dims[Dim::Name] = name.into();
	event.dims[Dim::Page] = page.into();
	event.dims[Dim::Source] = source.into();
	event
}

fn day() -> Vec<Event> {
	vec![
		event(1, 10, 100, "pageview", "/a", "Google"),
		event(1, 10, 200, "pageview", "/b", "Google"),
		event(2, 20, 3700, "pageview", "/a", ""),
		event(3, 10, 7300, "pageview", "/a", "Twitter"),
		event(3, 10, 7400, "Signup", "/a", "Twitter"),
		event(4, 30, 10_900, "pageview", "/b", ""),
		event(4, 30, 11_000, "engagement", "/b", ""),
	]
}

fn metrics(
	visitors: u64,
	visits: u64,
	pageviews: u64,
	events: u64,
	bounces: u64,
	duration: u64,
) -> Metrics {
	Metrics {
		visitors,
		visits,
		pageviews,
		events,
		bounces,
		duration,
	}
}

fn find(rollup: &crater::Rollup, dim: Dim, value: &str) -> Option<Metrics> {
	rollup
		.dim(dim)
		.iter()
		.find(|(name, _)| &**name == value)
		.map(|&(_, metrics)| metrics)
}

#[test]
fn seals_past_days() {
	let dir = TempDir::new("seal").unwrap();
	let db = Db::open(&dir.0).unwrap();
	for event in day() {
		assert!(db.insert(event));
	}
	db.sync().unwrap();

	let rollup = db.rollup(SITE, DAY).unwrap().unwrap();
	assert_eq!(rollup.total, metrics(3, 4, 5, 6, 2, 300));
	assert_eq!(rollup.hours[0], metrics(1, 1, 2, 2, 0, 100));
	assert_eq!(rollup.hours[3], metrics(1, 1, 1, 1, 1, 100));

	assert_eq!(
		find(&rollup, Dim::Page, "/a"),
		Some(metrics(2, 3, 3, 4, 1, 200))
	);
	assert_eq!(
		find(&rollup, Dim::Page, "/b"),
		Some(metrics(2, 2, 2, 2, 1, 200))
	);
	assert_eq!(
		find(&rollup, Dim::Source, "Google"),
		Some(metrics(1, 1, 2, 2, 0, 100))
	);
	assert_eq!(
		find(&rollup, Dim::Source, ""),
		Some(metrics(2, 2, 2, 2, 2, 100))
	);
	assert_eq!(
		find(&rollup, Dim::Source, "Twitter"),
		Some(metrics(1, 1, 1, 2, 0, 100))
	);
	assert_eq!(find(&rollup, Dim::Name, "engagement"), None);
	db.close().unwrap();
}

#[test]
fn merges_late_events_and_reopens() {
	let dir = TempDir::new("late").unwrap();
	let db = Db::open(&dir.0).unwrap();
	for event in day() {
		assert!(db.insert(event));
	}
	db.sync().unwrap();
	assert!(db.insert(event(2, 20, 3800, "pageview", "/c", "")));
	db.sync().unwrap();

	let rollup = db.rollup(SITE, DAY).unwrap().unwrap();
	assert_eq!(rollup.total, metrics(3, 4, 6, 7, 1, 400));
	assert_eq!(
		find(&rollup, Dim::Page, "/c"),
		Some(metrics(1, 1, 1, 1, 0, 100))
	);
	db.close().unwrap();

	let db = Db::open(&dir.0).unwrap();
	assert_eq!(db.rollup(SITE, DAY).unwrap().unwrap().total, rollup.total);
	assert_eq!(fs::read_dir(dir.0.join("wal")).unwrap().count(), 1);
	db.close().unwrap();
}
