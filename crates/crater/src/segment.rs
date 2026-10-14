use std::collections::HashMap;
use std::io;
use std::path::Path;

use crate::codec::{Reader, put_packed, put_uvar};
use crate::dict::Dict;
use crate::event::{Dim, Event};
use crate::frame::{self, invalid};
use crate::query::MISSING;

const MAGIC: &[u8; 5] = b"CRSG\x01";

#[derive(Clone)]
pub(crate) struct Columns {
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
	pub(crate) revenue: Vec<u32>,
	// dict ids, MISSING without revenue
	pub(crate) currency: Vec<u32>,
}

impl Columns {
	pub(crate) fn new() -> Self {
		Self {
			time: Vec::new(),
			visitor: Vec::new(),
			session: Vec::new(),
			interactive: Vec::new(),
			scroll: Vec::new(),
			engagement: Vec::new(),
			dims: vec![Vec::new(); Dim::COUNT],
			props: Vec::new(),
			keys: Vec::new(),
			values: Vec::new(),
			revenue: Vec::new(),
			currency: Vec::new(),
		}
	}

	pub(crate) fn rows(&self) -> usize {
		self.time.len()
	}

	pub(crate) fn push(&mut self, event: &Event, dense: &mut Dense, dict: &mut Dict) {
		self.time
			.push((event.ts + i64::from(event.offset)).rem_euclid(86_400) as u32);
		self.visitor.push(dense.visitor(event.visitor));
		self.session.push(dense.session(event.session));
		self.interactive.push(event.interactive.into());
		self.scroll
			.push(event.scroll_depth.map_or(0, |depth| u32::from(depth) + 1));
		self.engagement.push(event.engagement_ms);
		for dim in Dim::ALL {
			self.dims[dim as usize].push(dict.id(&event.dims[dim]));
		}
		let (amount, currency) = match &event.revenue {
			Some((amount, currency)) => (*amount, dict.id(currency)),
			None => (0, MISSING),
		};
		self.revenue.push(amount);
		self.currency.push(currency);
		self.props.push(event.props.len() as u32);
		for (key, value) in &event.props {
			self.keys.push(dict.id(key));
			self.values.push(dict.id(value));
		}
	}

	pub(crate) fn concat<'a>(parts: impl IntoIterator<Item = &'a Columns>) -> Self {
		let mut all = Self::new();
		for part in parts {
			all.time.extend(&part.time);
			all.visitor.extend(&part.visitor);
			all.session.extend(&part.session);
			all.interactive.extend(&part.interactive);
			all.scroll.extend(&part.scroll);
			all.engagement.extend(&part.engagement);
			for (all, part) in all.dims.iter_mut().zip(&part.dims) {
				all.extend(part);
			}
			all.props.extend(&part.props);
			all.keys.extend(&part.keys);
			all.values.extend(&part.values);
			all.revenue.extend(&part.revenue);
			all.currency.extend(&part.currency);
		}
		all
	}

	fn is_consistent(&self, visitors: usize, sessions: usize) -> bool {
		let rows = self.rows();
		let columns = [
			&self.visitor,
			&self.session,
			&self.interactive,
			&self.scroll,
			&self.engagement,
			&self.props,
			&self.revenue,
			&self.currency,
		];
		columns
			.into_iter()
			.chain(&self.dims)
			.all(|column| column.len() == rows)
			&& self.keys.len() == self.values.len()
			&& self.props.iter().map(|&n| n as usize).sum::<usize>() == self.keys.len()
			&& self.visitor.iter().all(|&id| (id as usize) < visitors)
			&& self.session.iter().all(|&id| (id as usize) < sessions)
			&& self.time.iter().all(|&time| time < 86_400)
	}
}

#[derive(Default)]
pub(crate) struct Dense {
	pub(crate) visitors: Vec<u64>,
	pub(crate) sessions: Vec<u64>,
	visitor_ids: HashMap<u64, u32>,
	session_ids: HashMap<u64, u32>,
}

impl Dense {
	pub(crate) fn from_lists(visitors: Vec<u64>, sessions: Vec<u64>) -> Self {
		let index = |ids: &[u64]| -> HashMap<u64, u32> {
			ids.iter()
				.enumerate()
				.map(|(i, &id)| (id, i as u32))
				.collect()
		};
		Self {
			visitor_ids: index(&visitors),
			session_ids: index(&sessions),
			visitors,
			sessions,
		}
	}

	fn visitor(&mut self, id: u64) -> u32 {
		dense(&mut self.visitor_ids, &mut self.visitors, id)
	}

	fn session(&mut self, id: u64) -> u32 {
		dense(&mut self.session_ids, &mut self.sessions, id)
	}
}

fn dense(ids: &mut HashMap<u64, u32>, list: &mut Vec<u64>, key: u64) -> u32 {
	*ids.entry(key).or_insert_with(|| {
		list.push(key);
		list.len() as u32 - 1
	})
}

pub(crate) struct Segment {
	pub(crate) seq: u64,
	pub(crate) visitors: Vec<u64>,
	pub(crate) sessions: Vec<u64>,
	pub(crate) cols: Columns,
}

impl Segment {
	pub(crate) fn write(&self, path: &Path) -> io::Result<()> {
		let cols = &self.cols;
		let mut buf = Vec::new();
		frame::put(&mut buf, |buf| {
			put_uvar(buf, self.seq);
			put_uvar(buf, cols.rows() as u64);
		});
		frame::put(&mut buf, |buf| {
			for ids in [&self.visitors, &self.sessions] {
				put_uvar(buf, ids.len() as u64);
				for id in ids {
					buf.extend_from_slice(&id.to_le_bytes());
				}
			}
			for column in [
				&cols.time,
				&cols.visitor,
				&cols.session,
				&cols.interactive,
				&cols.scroll,
				&cols.engagement,
				&cols.props,
			] {
				put_packed(buf, column);
			}
			for column in cols.dims.iter().chain([&cols.keys, &cols.values]) {
				put_strings(buf, column);
			}
			put_packed(buf, &cols.revenue);
			put_packed(buf, &cols.currency);
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
			.filter(|seg| {
				seg.cols.rows() == rows
					&& seg
						.cols
						.is_consistent(seg.visitors.len(), seg.sessions.len())
			})
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

fn header(buf: &[u8]) -> Option<(u64, usize)> {
	let mut r = Reader::new(buf);
	Some((r.uvar()?, usize::try_from(r.uvar()?).ok()?))
}

fn decode(buf: &[u8], seq: u64) -> Option<Segment> {
	let mut r = Reader::new(buf);
	let mut ids = || -> Option<Vec<u64>> { (0..r.uvar()?).map(|_| r.u64()).collect() };
	let visitors = ids()?;
	let sessions = ids()?;
	let mut cols = Columns {
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
		revenue: Vec::new(),
		currency: Vec::new(),
	};
	// segments written before revenue end here
	match r.is_empty() {
		true => {
			cols.revenue = vec![0; cols.rows()];
			cols.currency = vec![MISSING; cols.rows()];
		}
		false => {
			cols.revenue = r.packed()?;
			cols.currency = r.packed()?;
		}
	}
	r.is_empty().then_some(Segment {
		seq,
		visitors,
		sessions,
		cols,
	})
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
