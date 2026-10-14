use std::collections::HashMap;
use std::io;
use std::path::Path;

use crate::codec::{Reader, put_packed, put_uvar};
use crate::dict::Dict;
use crate::event::{Dim, Event};
use crate::frame::{self, invalid};

const MAGIC: &[u8; 5] = b"CRSG\x01";

pub(crate) struct Segment {
	pub(crate) seq: u64,
	pub(crate) visitors: Vec<u64>,
	pub(crate) sessions: Vec<u64>,
	pub(crate) time: Vec<u32>,
	pub(crate) visitor: Vec<u32>,
	pub(crate) session: Vec<u32>,
	pub(crate) interactive: Vec<u32>,
	// 0 = no scroll, else depth + 1
	pub(crate) scroll: Vec<u32>,
	pub(crate) engagement: Vec<u32>,
	pub(crate) dims: Vec<Vec<u32>>,
	pub(crate) props: Vec<u32>,
	pub(crate) keys: Vec<u32>,
	pub(crate) values: Vec<u32>,
}

impl Segment {
	pub(crate) fn build(mut events: Vec<Event>, dict: &mut Dict, seq: u64) -> Self {
		events.sort_unstable_by_key(|event| (event.session, event.ts + i64::from(event.offset)));
		let rows = events.len();
		let mut visitors = Vec::new();
		let mut sessions = Vec::new();
		let mut visitor_ids = HashMap::new();
		let mut session_ids = HashMap::new();
		let mut seg = Self {
			seq,
			visitors: Vec::new(),
			sessions: Vec::new(),
			time: Vec::with_capacity(rows),
			visitor: Vec::with_capacity(rows),
			session: Vec::with_capacity(rows),
			interactive: Vec::with_capacity(rows),
			scroll: Vec::with_capacity(rows),
			engagement: Vec::with_capacity(rows),
			dims: (0..Dim::COUNT).map(|_| Vec::with_capacity(rows)).collect(),
			props: Vec::with_capacity(rows),
			keys: Vec::new(),
			values: Vec::new(),
		};

		for event in &events {
			seg.time
				.push((event.ts + i64::from(event.offset)).rem_euclid(86_400) as u32);
			seg.visitor
				.push(dense(&mut visitor_ids, &mut visitors, event.visitor));
			seg.session
				.push(dense(&mut session_ids, &mut sessions, event.session));
			seg.interactive.push(event.interactive.into());
			seg.scroll
				.push(event.scroll_depth.map_or(0, |depth| u32::from(depth) + 1));
			seg.engagement.push(event.engagement_ms);
			for dim in Dim::ALL {
				seg.dims[dim as usize].push(dict.id(&event.dims[dim]));
			}
			seg.props.push(event.props.len() as u32);
			for (key, value) in &event.props {
				seg.keys.push(dict.id(key));
				seg.values.push(dict.id(value));
			}
		}

		seg.visitors = visitors;
		seg.sessions = sessions;
		seg
	}

	pub(crate) fn rows(&self) -> usize {
		self.time.len()
	}

	fn is_consistent(&self) -> bool {
		let rows = self.rows();
		let columns = [
			&self.visitor,
			&self.session,
			&self.interactive,
			&self.scroll,
			&self.engagement,
			&self.props,
		];
		columns
			.into_iter()
			.chain(&self.dims)
			.all(|column| column.len() == rows)
			&& self.keys.len() == self.values.len()
			&& self.props.iter().map(|&n| n as usize).sum::<usize>() == self.keys.len()
			&& self
				.visitor
				.iter()
				.all(|&id| (id as usize) < self.visitors.len())
			&& self
				.session
				.iter()
				.all(|&id| (id as usize) < self.sessions.len())
	}

	pub(crate) fn events(&self, dict: &Dict, site: u64, day: i32) -> io::Result<Vec<Event>> {
		let string = |id: u32| {
			dict.get(id)
				.map(|s| s.to_string())
				.ok_or_else(|| invalid("unknown string id"))
		};
		let mut prop = 0;
		let mut events = Vec::with_capacity(self.rows());
		for row in 0..self.rows() {
			let mut event = Event {
				site,
				ts: i64::from(day) * 86_400 + i64::from(self.time[row]),
				offset: 0,
				visitor: self.visitors[self.visitor[row] as usize],
				session: self.sessions[self.session[row] as usize],
				interactive: self.interactive[row] == 1,
				scroll_depth: self.scroll[row].checked_sub(1).map(|depth| depth as u8),
				engagement_ms: self.engagement[row],
				..Event::default()
			};
			for dim in Dim::ALL {
				event.dims[dim] = string(self.dims[dim as usize][row])?;
			}
			for _ in 0..self.props[row] {
				event
					.props
					.push((string(self.keys[prop])?, string(self.values[prop])?));
				prop += 1;
			}
			events.push(event);
		}
		Ok(events)
	}

	pub(crate) fn write(&self, path: &Path) -> io::Result<()> {
		let mut buf = Vec::new();
		frame::put(&mut buf, |buf| {
			put_uvar(buf, self.seq);
			put_uvar(buf, self.rows() as u64);
		});
		frame::put(&mut buf, |buf| {
			for ids in [&self.visitors, &self.sessions] {
				put_uvar(buf, ids.len() as u64);
				for id in ids {
					buf.extend_from_slice(&id.to_le_bytes());
				}
			}
			for column in [
				&self.time,
				&self.visitor,
				&self.session,
				&self.interactive,
				&self.scroll,
				&self.engagement,
				&self.props,
			] {
				put_packed(buf, column);
			}
			for column in self.dims.iter().chain([&self.keys, &self.values]) {
				put_strings(buf, column);
			}
		});
		frame::write_file(path, MAGIC, &buf)
	}

	pub(crate) fn read(path: &Path) -> io::Result<Self> {
		let frames = frame::read_file(path, MAGIC, 2)?;
		let (Some(head), Some(data)) = (frames.first(), frames.get(1)) else {
			return Err(invalid("truncated segment"));
		};
		let (seq, rows) = header(head).ok_or_else(|| invalid("bad segment header"))?;
		decode(data, seq)
			.filter(|seg| seg.rows() == rows && seg.is_consistent())
			.ok_or_else(|| invalid("bad segment data"))
	}

	pub(crate) fn read_seq(path: &Path) -> io::Result<u64> {
		let frames = frame::read_file(path, MAGIC, 1)?;
		frames
			.first()
			.and_then(|head| header(head))
			.map(|(seq, _)| seq)
			.ok_or_else(|| invalid("bad segment header"))
	}
}

fn dense(ids: &mut HashMap<u64, u32>, list: &mut Vec<u64>, key: u64) -> u32 {
	*ids.entry(key).or_insert_with(|| {
		list.push(key);
		list.len() as u32 - 1
	})
}

fn header(buf: &[u8]) -> Option<(u64, usize)> {
	let mut r = Reader::new(buf);
	Some((r.uvar()?, usize::try_from(r.uvar()?).ok()?))
}

fn decode(buf: &[u8], seq: u64) -> Option<Segment> {
	let mut r = Reader::new(buf);
	let mut ids = || -> Option<Vec<u64>> { (0..r.uvar()?).map(|_| r.u64()).collect() };
	let visitors = ids()?;
	let sessions = ids()?;
	let seg = Segment {
		seq,
		visitors,
		sessions,
		time: r.packed()?,
		visitor: r.packed()?,
		session: r.packed()?,
		interactive: r.packed()?,
		scroll: r.packed()?,
		engagement: r.packed()?,
		props: r.packed()?,
		dims: (0..Dim::COUNT)
			.map(|_| strings(&mut r))
			.collect::<Option<_>>()?,
		keys: strings(&mut r)?,
		values: strings(&mut r)?,
	};
	r.is_empty().then_some(seg)
}

fn put_strings(buf: &mut Vec<u8>, ids: &[u32]) {
	let mut local = ids.to_vec();
	local.sort_unstable();
	local.dedup();
	put_uvar(buf, local.len() as u64);
	let mut prev = 0;
	for &id in &local {
		put_uvar(buf, (id - prev).into());
		prev = id;
	}
	let packed: Vec<u32> = ids
		.iter()
		.map(|id| local.binary_search(id).unwrap_or_default() as u32)
		.collect();
	put_packed(buf, &packed);
}

fn strings(r: &mut Reader) -> Option<Vec<u32>> {
	let mut prev = 0u32;
	let local = (0..r.uvar()?)
		.map(|_| {
			prev = prev.checked_add(u32::try_from(r.uvar()?).ok()?)?;
			Some(prev)
		})
		.collect::<Option<Vec<u32>>>()?;
	r.packed()?
		.into_iter()
		.map(|i| local.get(i as usize).copied())
		.collect()
}
