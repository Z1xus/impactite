use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::event::{Dim, Event};

pub(crate) struct TempDir(PathBuf);

impl TempDir {
	pub(crate) fn new(name: &str) -> io::Result<Self> {
		let path = std::env::temp_dir().join(format!("crater-{name}-{}", std::process::id()));
		fs::create_dir_all(&path)?;
		Ok(Self(path))
	}

	pub(crate) fn path(&self) -> &Path {
		&self.0
	}
}

impl Drop for TempDir {
	fn drop(&mut self) {
		let _ = fs::remove_dir_all(&self.0);
	}
}

pub(crate) fn event(i: u64) -> Event {
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
