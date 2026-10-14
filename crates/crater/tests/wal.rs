use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use crater::{Dim, Event, Wal};

struct TempDir(PathBuf);

impl TempDir {
	fn new(name: &str) -> std::io::Result<Self> {
		let path = std::env::temp_dir().join(format!("crater-{name}-{}", std::process::id()));
		fs::create_dir_all(&path)?;
		Ok(Self(path))
	}
}

impl Drop for TempDir {
	fn drop(&mut self) {
		let _ = fs::remove_dir_all(&self.0);
	}
}

fn event(i: u64) -> Event {
	let mut event = Event {
		site: 1 + i % 3,
		ts: 1_792_000_000 + i as i64,
		offset: 7200,
		visitor: i.wrapping_mul(0x9e37_79b9_7f4a_7c15),
		session: i / 5,
		interactive: i.is_multiple_of(2),
		scroll_depth: i.is_multiple_of(3).then_some((i % 101) as u8),
		engagement_ms: (i % 60_000) as u32,
		..Event::default()
	};
	event.dims[Dim::Name] = "pageview".into();
	event.dims[Dim::Page] = format!("/post/{}", i % 500);
	event.dims[Dim::Country] = "DE".into();
	if i.is_multiple_of(4) {
		event.dims[Dim::Source] = "Google".into();
		event.props.push(("plan".into(), "pro".into()));
	}
	event
}

#[test]
fn round_trip() {
	let dir = TempDir::new("round-trip").unwrap();
	let path = dir.0.join("events.wal");
	let events: Vec<Event> = (0..100_000).map(event).collect();

	let (mut wal, replayed) = Wal::open(&path).unwrap();
	assert!(replayed.is_empty());
	for chunk in events.chunks(4096) {
		wal.append(chunk).unwrap();
	}
	drop(wal);

	let (_, replayed) = Wal::open(&path).unwrap();
	assert_eq!(replayed, events);
}

#[test]
fn torn_tail() {
	let dir = TempDir::new("torn-tail").unwrap();
	let path = dir.0.join("events.wal");
	let events: Vec<Event> = (0..1000).map(event).collect();

	let (mut wal, _) = Wal::open(&path).unwrap();
	wal.append(&events).unwrap();
	drop(wal);
	let intact = fs::metadata(&path).unwrap().len();

	let mut file = OpenOptions::new().append(true).open(&path).unwrap();
	file.write_all(&[200, 0, 0, 0, 1, 2, 3]).unwrap();
	drop(file);

	let (mut wal, replayed) = Wal::open(&path).unwrap();
	assert_eq!(replayed, events);
	assert_eq!(fs::metadata(&path).unwrap().len(), intact);
	wal.append(&[event(1000)]).unwrap();
	drop(wal);

	let (_, replayed) = Wal::open(&path).unwrap();
	assert_eq!(replayed.len(), 1001);
}
