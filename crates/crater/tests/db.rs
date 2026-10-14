use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crater::{Db, Dim, Event, Field, Filter, Metrics, Op, Query, Report};

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
		{
			let mut signup = event(3, 10, 7400, "Signup", "/a", "Twitter");
			signup.props.push(("plan".into(), "pro".into()));
			signup
		},
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

fn query(db: &Db, filters: Vec<Filter>, breakdown: Option<Field>) -> std::io::Result<Report> {
	db.query(&Query {
		site: SITE,
		days: DAY - 1..=DAY + 1,
		filters,
		breakdown,
		limit: 10,
		daily: false,
	})
}

fn row(report: &Report, value: &str) -> Option<Metrics> {
	report
		.rows
		.iter()
		.find(|(name, _)| &**name == value)
		.map(|&(_, metrics)| metrics)
}

fn filter(dim: Dim, op: Op, value: &str) -> Filter {
	Filter {
		field: Field::Dim(dim),
		op,
		values: vec![value.into()],
	}
}

#[test]
fn seals_past_days() {
	let dir = TempDir::new("seal").unwrap();
	let db = Db::open(&dir.0).unwrap();
	for event in day() {
		db.insert(Box::new(event)).unwrap();
	}
	db.sync().unwrap();

	let report = query(&db, Vec::new(), Some(Field::Dim(Dim::Page))).unwrap();
	assert_eq!(report.total, metrics(3, 4, 5, 6, 2, 300));
	assert_eq!(report.days.len(), 3);
	assert_eq!(report.days[1], report.total);
	assert_eq!(report.hours[0], metrics(1, 1, 2, 2, 0, 100));
	assert_eq!(report.hours[3], metrics(1, 1, 1, 1, 1, 100));
	assert_eq!(row(&report, "/a"), Some(metrics(2, 3, 3, 4, 1, 200)));
	assert_eq!(row(&report, "/b"), Some(metrics(2, 2, 2, 2, 1, 200)));

	let report = query(&db, Vec::new(), Some(Field::Dim(Dim::Source))).unwrap();
	assert_eq!(row(&report, "Google"), Some(metrics(1, 1, 2, 2, 0, 100)));
	assert_eq!(row(&report, ""), Some(metrics(2, 2, 2, 2, 2, 100)));
	assert_eq!(row(&report, "Twitter"), Some(metrics(1, 1, 1, 2, 0, 100)));

	let report = query(&db, Vec::new(), Some(Field::Dim(Dim::Name))).unwrap();
	assert_eq!(row(&report, "engagement"), None);

	let report = query(&db, Vec::new(), Some(Field::Entry)).unwrap();
	assert_eq!(row(&report, "/a"), Some(metrics(2, 3, 4, 5, 1, 200)));
	assert_eq!(row(&report, "/b"), Some(metrics(1, 1, 1, 1, 1, 100)));

	let report = query(&db, Vec::new(), Some(Field::Exit)).unwrap();
	assert_eq!(row(&report, "/a"), Some(metrics(2, 2, 2, 3, 1, 100)));
	assert_eq!(row(&report, "/b"), Some(metrics(2, 2, 3, 3, 1, 200)));

	let report = query(&db, Vec::new(), Some(Field::Prop("plan".into()))).unwrap();
	assert_eq!(report.rows.len(), 1);
	assert_eq!(row(&report, "pro"), Some(metrics(1, 1, 0, 1, 0, 100)));
	db.close().unwrap();
}

#[test]
fn filters() {
	let dir = TempDir::new("filters").unwrap();
	let db = Db::open(&dir.0).unwrap();
	for event in day() {
		db.insert(Box::new(event)).unwrap();
	}
	db.sync().unwrap();

	let google = vec![filter(Dim::Source, Op::Is, "Google")];
	assert_eq!(
		query(&db, google, None).unwrap().total,
		metrics(1, 1, 2, 2, 0, 100)
	);

	let page_a = vec![filter(Dim::Page, Op::Is, "/a")];
	let report = query(&db, page_a, Some(Field::Dim(Dim::Source))).unwrap();
	assert_eq!(report.total, metrics(2, 3, 3, 4, 1, 200));
	assert_eq!(row(&report, "Google"), Some(metrics(1, 1, 1, 1, 0, 100)));
	assert_eq!(row(&report, ""), Some(metrics(1, 1, 1, 1, 1, 0)));
	assert_eq!(row(&report, "Twitter"), Some(metrics(1, 1, 1, 2, 0, 100)));

	let not_a = vec![filter(Dim::Page, Op::IsNot, "/a")];
	let contains_b = vec![filter(Dim::Page, Op::Contains, "b")];
	assert_eq!(
		query(&db, not_a, None).unwrap().total,
		metrics(2, 2, 2, 2, 1, 200)
	);
	assert_eq!(
		query(&db, contains_b, None).unwrap().total,
		metrics(2, 2, 2, 2, 1, 200)
	);

	let glob = vec![filter(Dim::Page, Op::Matches, "/a*")];
	assert_eq!(
		query(&db, glob, None).unwrap().total,
		metrics(2, 3, 3, 4, 1, 200)
	);
	db.close().unwrap();
}

#[test]
fn merges_late_events_and_reopens() {
	let dir = TempDir::new("late").unwrap();
	let db = Db::open(&dir.0).unwrap();
	for event in day() {
		db.insert(Box::new(event)).unwrap();
	}
	db.sync().unwrap();
	db.insert(Box::new(event(2, 20, 3800, "pageview", "/c", "")))
		.unwrap();
	db.sync().unwrap();

	let report = query(&db, Vec::new(), Some(Field::Dim(Dim::Page))).unwrap();
	assert_eq!(report.total, metrics(3, 4, 6, 7, 1, 400));
	assert_eq!(row(&report, "/c"), Some(metrics(1, 1, 1, 1, 0, 100)));
	db.close().unwrap();

	let db = Db::open(&dir.0).unwrap();
	assert_eq!(query(&db, Vec::new(), None).unwrap().total, report.total);
	assert_eq!(fs::read_dir(dir.0.join("wal")).unwrap().count(), 1);
	db.close().unwrap();
}

#[test]
fn serves_today_live_and_after_restart() {
	let dir = TempDir::new("live").unwrap();
	let now = SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.unwrap()
		.as_secs() as i64;
	let today = now.div_euclid(86_400) as i32;
	let live = |session: u64, ts: i64| {
		let mut event = event(session, session, 0, "pageview", "/", "");
		event.ts = ts;
		event.offset = 0;
		event
	};
	let today_query = Query {
		site: SITE,
		days: today..=today,
		filters: Vec::new(),
		breakdown: None,
		limit: 10,
		daily: false,
	};

	let db = Db::open(&dir.0).unwrap();
	db.insert(Box::new(live(1, now - 60))).unwrap();
	db.insert(Box::new(live(1, now - 30))).unwrap();
	db.insert(Box::new(live(2, now - 10))).unwrap();
	db.sync().unwrap();
	let expected = metrics(2, 2, 3, 3, 1, 30);
	assert_eq!(db.query(&today_query).unwrap().total, expected);
	db.close().unwrap();

	let db = Db::open(&dir.0).unwrap();
	assert_eq!(db.query(&today_query).unwrap().total, expected);
	db.close().unwrap();
}
