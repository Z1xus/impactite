use std::fs::File;
use std::io::{self, Write};
use std::path::Path;

use crate::codec::{Reader, put_ivar, put_str, put_uvar};
use crate::event::{Dim, Event};
use crate::frame;

const MAGIC: &[u8; 5] = b"CRWL\x01";

pub(crate) struct Wal {
	file: File,
	buf: Vec<u8>,
	pub(crate) size: u64,
}

impl Wal {
	pub(crate) fn open(path: &Path) -> io::Result<(Self, Vec<Event>)> {
		let mut events = Vec::new();
		let file = frame::open_log(path, MAGIC, |payload| {
			decode(payload).map(|event| events.push(event)).is_some()
		})?;
		let size = file.metadata()?.len();
		let wal = Self {
			file,
			buf: Vec::new(),
			size,
		};
		Ok((wal, events))
	}

	pub(crate) fn append(&mut self, events: &[Event]) -> io::Result<()> {
		self.buf.clear();
		for event in events {
			frame::put(&mut self.buf, |buf| encode(buf, event));
		}
		self.file.write_all(&self.buf)?;
		self.size += self.buf.len() as u64;
		self.file.sync_data()
	}
}

fn encode(buf: &mut Vec<u8>, event: &Event) {
	put_uvar(buf, event.site);
	put_ivar(buf, event.ts);
	put_ivar(buf, event.offset.into());
	buf.extend_from_slice(&event.visitor.to_le_bytes());
	buf.extend_from_slice(&event.session.to_le_bytes());
	buf.push(u8::from(event.interactive) | u8::from(event.scroll_depth.is_some()) << 1);
	if let Some(depth) = event.scroll_depth {
		buf.push(depth);
	}
	put_uvar(buf, event.engagement_ms.into());

	let mask = Dim::ALL
		.iter()
		.enumerate()
		.filter(|&(_, &dim)| !event.dims[dim].is_empty())
		.fold(0u64, |mask, (i, _)| mask | 1 << i);
	put_uvar(buf, mask);
	for dim in Dim::ALL {
		if !event.dims[dim].is_empty() {
			put_str(buf, &event.dims[dim]);
		}
	}

	put_uvar(buf, event.props.len() as u64);
	for (key, value) in &event.props {
		put_str(buf, key);
		put_str(buf, value);
	}
}

fn decode(buf: &[u8]) -> Option<Event> {
	let mut r = Reader::new(buf);
	let mut event = Event {
		site: r.uvar()?,
		ts: r.ivar()?,
		offset: i32::try_from(r.ivar()?).ok()?,
		visitor: r.u64()?,
		session: r.u64()?,
		..Event::default()
	};

	let flags = r.u8()?;
	event.interactive = flags & 1 != 0;
	if flags & 2 != 0 {
		event.scroll_depth = Some(r.u8()?);
	}
	event.engagement_ms = u32::try_from(r.uvar()?).ok()?;

	let mask = r.uvar()?;
	for (i, dim) in Dim::ALL.into_iter().enumerate() {
		if mask >> i & 1 != 0 {
			event.dims[dim] = r.str()?;
		}
	}

	for _ in 0..r.uvar()? {
		event.props.push((r.str()?, r.str()?));
	}

	r.is_empty().then_some(event)
}

#[cfg(test)]
mod tests {
	use std::fs::{self, OpenOptions};

	use super::*;
	use crate::testing::{TempDir, event};

	#[test]
	fn torn_tail() {
		let dir = TempDir::new("wal-torn-tail").unwrap();
		let path = dir.path().join("events.wal");
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
}
