use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Write};
use std::path::Path;

use crate::codec::{Reader, put_packed, put_uvar};
use crate::dict::Dict;
use crate::event::{Dim, Event};
use crate::frame::{self, invalid};
use crate::query::MISSING;

const MAGIC: &[u8; 5] = b"CRSG\x01";
const CHECKPOINT: &[u8; 5] = b"CRCP\x01";

// dims stored before revenue, later ones go after it
const LEGACY_DIMS: usize = 19;

// columns while a part is built, packed into `Columns` once it is done
#[derive(Clone)]
pub(crate) struct Raw {
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

impl Raw {
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

	pub(crate) fn pack(&self) -> Columns {
		Columns {
			time: Col::pack(&self.time),
			visitor: Col::pack(&self.visitor),
			session: Col::pack(&self.session),
			interactive: Col::pack(&self.interactive),
			scroll: Col::pack(&self.scroll),
			engagement: Col::pack(&self.engagement),
			dims: self.dims.iter().map(|dim| Col::pack(dim)).collect(),
			props: Col::pack(&self.props),
			keys: Col::pack(&self.keys),
			values: Col::pack(&self.values),
			revenue: Col::pack(&self.revenue),
			currency: Col::pack(&self.currency),
		}
	}

	pub(crate) fn keep(&self, keep: impl Fn(usize) -> bool) -> Self {
		let mut kept = Self::new();
		let mut prop = 0;
		for row in 0..self.rows() {
			let count = self.props[row] as usize;
			if keep(row) {
				for (kept, column) in [
					(&mut kept.time, &self.time),
					(&mut kept.visitor, &self.visitor),
					(&mut kept.session, &self.session),
					(&mut kept.interactive, &self.interactive),
					(&mut kept.scroll, &self.scroll),
					(&mut kept.engagement, &self.engagement),
					(&mut kept.props, &self.props),
					(&mut kept.revenue, &self.revenue),
					(&mut kept.currency, &self.currency),
				] {
					kept.push(column[row]);
				}
				for (kept, column) in kept.dims.iter_mut().zip(&self.dims) {
					kept.push(column[row]);
				}
				kept.keys.extend(&self.keys[prop..prop + count]);
				kept.values.extend(&self.values[prop..prop + count]);
			}
			prop += count;
		}
		kept
	}

	pub(crate) fn is_consistent(&self, visitors: usize, sessions: usize) -> bool {
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

// a column bit-packed at the width its values need above the smallest one, a column
// that never changes takes no space
#[derive(Clone)]
pub(crate) struct Col {
	base: u32,
	width: u32,
	len: usize,
	words: Vec<u64>,
}

impl Col {
	pub(crate) fn pack(values: &[u32]) -> Self {
		let base = values.iter().copied().min().unwrap_or(0);
		let width = values
			.iter()
			.map(|&value| value - base)
			.max()
			.map_or(0, |max| 32 - max.leading_zeros());
		let mut words = vec![0u64; (values.len() * width as usize).div_ceil(64)];
		if width > 0 {
			for (i, &value) in values.iter().enumerate() {
				let bit = i * width as usize;
				let (word, offset) = (bit / 64, bit % 64);
				let value = u64::from(value - base);
				words[word] |= value << offset;
				if offset + width as usize > 64 {
					words[word + 1] |= value >> (64 - offset);
				}
			}
		}
		Self {
			base,
			width,
			len: values.len(),
			words,
		}
	}

	#[inline]
	pub(crate) fn get(&self, i: usize) -> u32 {
		if self.width == 0 {
			return self.base;
		}
		let bit = i * self.width as usize;
		let (word, offset) = (bit / 64, bit % 64);
		let mut value = self.words[word] >> offset;
		if offset + self.width as usize > 64 {
			value |= self.words[word + 1] << (64 - offset);
		}
		self.base + (value & ((1 << self.width) - 1)) as u32
	}

	pub(crate) fn len(&self) -> usize {
		self.len
	}

	pub(crate) fn iter(&self) -> impl Iterator<Item = u32> + '_ {
		(0..self.len).map(|i| self.get(i))
	}

	fn unpack(&self) -> Vec<u32> {
		self.iter().collect()
	}

	fn put(&self, buf: &mut Vec<u8>) {
		put_uvar(buf, self.base.into());
		put_uvar(buf, self.width.into());
		put_uvar(buf, self.len as u64);
		for word in &self.words {
			buf.extend_from_slice(&word.to_le_bytes());
		}
	}

	fn read(r: &mut Reader) -> Option<Self> {
		let base = u32::try_from(r.uvar()?).ok()?;
		let width = u32::try_from(r.uvar()?).ok().filter(|&width| width <= 32)?;
		let len = usize::try_from(r.uvar()?).ok()?;
		let words = (0..len.checked_mul(width as usize)?.div_ceil(64))
			.map(|_| r.u64())
			.collect::<Option<_>>()?;
		Some(Self {
			base,
			width,
			len,
			words,
		})
	}
}

#[derive(Clone)]
pub(crate) struct Columns {
	pub(crate) time: Col,
	pub(crate) visitor: Col,
	pub(crate) session: Col,
	pub(crate) interactive: Col,
	pub(crate) scroll: Col,
	pub(crate) engagement: Col,
	pub(crate) dims: Vec<Col>,
	pub(crate) props: Col,
	pub(crate) keys: Col,
	pub(crate) values: Col,
	pub(crate) revenue: Col,
	pub(crate) currency: Col,
}

impl Columns {
	pub(crate) fn rows(&self) -> usize {
		self.time.len()
	}

	pub(crate) fn raw(&self) -> Raw {
		Raw {
			time: self.time.unpack(),
			visitor: self.visitor.unpack(),
			session: self.session.unpack(),
			interactive: self.interactive.unpack(),
			scroll: self.scroll.unpack(),
			engagement: self.engagement.unpack(),
			dims: self.dims.iter().map(Col::unpack).collect(),
			props: self.props.unpack(),
			keys: self.keys.unpack(),
			values: self.values.unpack(),
			revenue: self.revenue.unpack(),
			currency: self.currency.unpack(),
		}
	}

	pub(crate) fn concat<'a>(parts: impl IntoIterator<Item = &'a Columns>) -> Self {
		let mut all = Raw::new();
		for part in parts {
			let part = part.raw();
			all.time.extend(part.time);
			all.visitor.extend(part.visitor);
			all.session.extend(part.session);
			all.interactive.extend(part.interactive);
			all.scroll.extend(part.scroll);
			all.engagement.extend(part.engagement);
			for (all, part) in all.dims.iter_mut().zip(part.dims) {
				all.extend(part);
			}
			all.props.extend(part.props);
			all.keys.extend(part.keys);
			all.values.extend(part.values);
			all.revenue.extend(part.revenue);
			all.currency.extend(part.currency);
		}
		all.pack()
	}

	pub(crate) fn keep(&self, keep: impl Fn(usize) -> bool) -> Self {
		self.raw().keep(keep).pack()
	}

	fn put(&self, buf: &mut Vec<u8>) {
		for col in [
			&self.time,
			&self.visitor,
			&self.session,
			&self.interactive,
			&self.scroll,
			&self.engagement,
			&self.props,
			&self.keys,
			&self.values,
			&self.revenue,
			&self.currency,
		] {
			col.put(buf);
		}
		put_uvar(buf, self.dims.len() as u64);
		for dim in &self.dims {
			dim.put(buf);
		}
	}

	fn read(r: &mut Reader) -> Option<Self> {
		let mut col = || Col::read(r);
		let (time, visitor, session, interactive, scroll, engagement) =
			(col()?, col()?, col()?, col()?, col()?, col()?);
		let (props, keys, values, revenue, currency) = (col()?, col()?, col()?, col()?, col()?);
		let count = usize::try_from(r.uvar()?)
			.ok()
			.filter(|&count| count <= Dim::COUNT)?;
		let mut dims = (0..count)
			.map(|_| Col::read(r))
			.collect::<Option<Vec<_>>>()?;
		// dims added after this checkpoint was written
		dims.resize(Dim::COUNT, Col::pack(&vec![MISSING; time.len()]));
		Some(Self {
			time,
			visitor,
			session,
			interactive,
			scroll,
			engagement,
			dims,
			props,
			keys,
			values,
			revenue,
			currency,
		})
	}
}

// what an open day packed since its last checkpoint, with the wal seq it covers and the
// visitors and sessions it added
pub(crate) struct Checkpoint {
	pub(crate) seq: u64,
	pub(crate) visitors: Vec<u64>,
	pub(crate) sessions: Vec<u64>,
	pub(crate) parts: Vec<Columns>,
}

impl Checkpoint {
	pub(crate) fn read_all(path: &Path) -> io::Result<Vec<Self>> {
		if !path.try_exists()? {
			return Ok(Vec::new());
		}
		let mut all = Vec::new();
		let mut bad = false;
		frame::open_log(path, CHECKPOINT, |payload| {
			match decode_checkpoint(payload) {
				Some(checkpoint) => {
					all.push(checkpoint);
					true
				}
				None => {
					bad = true;
					false
				}
			}
		})?;
		match bad {
			true => Err(invalid("bad checkpoint")),
			false => Ok(all),
		}
	}

	pub(crate) fn append(&self, path: &Path) -> io::Result<()> {
		let mut file: File = frame::open_log(path, CHECKPOINT, |_| true)?;
		let mut buf = Vec::new();
		frame::put(&mut buf, |buf| {
			put_uvar(buf, self.seq);
			for ids in [&self.visitors, &self.sessions] {
				put_uvar(buf, ids.len() as u64);
				for id in ids {
					buf.extend_from_slice(&id.to_le_bytes());
				}
			}
			put_uvar(buf, self.parts.len() as u64);
			for part in &self.parts {
				part.put(buf);
			}
		});
		file.write_all(&buf)?;
		file.sync_data()
	}
}

fn decode_checkpoint(buf: &[u8]) -> Option<Checkpoint> {
	let mut r = Reader::new(buf);
	let seq = r.uvar()?;
	let mut ids = || -> Option<Vec<u64>> { (0..r.uvar()?).map(|_| r.u64()).collect() };
	let (visitors, sessions) = (ids()?, ids()?);
	let parts = (0..r.uvar()?)
		.map(|_| Columns::read(&mut r))
		.collect::<Option<_>>()?;
	r.is_empty().then_some(Checkpoint {
		seq,
		visitors,
		sessions,
		parts,
	})
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

	pub(crate) fn extend(&mut self, visitors: Vec<u64>, sessions: Vec<u64>) {
		for id in visitors {
			self.visitor(id);
		}
		for id in sessions {
			self.session(id);
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
		let cols = &self.cols.raw();
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
			for column in cols.dims[..LEGACY_DIMS]
				.iter()
				.chain([&cols.keys, &cols.values])
			{
				put_strings(buf, column);
			}
			put_packed(buf, &cols.revenue);
			put_packed(buf, &cols.currency);
			for column in &cols.dims[LEGACY_DIMS..] {
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
		decode(data, seq, rows).ok_or_else(|| invalid("bad segment data"))
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

fn decode(buf: &[u8], seq: u64, rows: usize) -> Option<Segment> {
	let mut r = Reader::new(buf);
	let mut ids = || -> Option<Vec<u64>> { (0..r.uvar()?).map(|_| r.u64()).collect() };
	let visitors = ids()?;
	let sessions = ids()?;
	let mut cols = Raw {
		time: r.packed()?,
		visitor: r.packed()?,
		session: r.packed()?,
		interactive: r.packed()?,
		scroll: r.packed()?,
		engagement: r.packed()?,
		props: r.packed()?,
		dims: (0..LEGACY_DIMS)
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
	// dims added later follow, older segments lack them
	for _ in LEGACY_DIMS..Dim::COUNT {
		let column = match r.is_empty() {
			true => vec![MISSING; cols.rows()],
			false => strings(&mut r)?,
		};
		cols.dims.push(column);
	}
	(r.is_empty() && cols.rows() == rows && cols.is_consistent(visitors.len(), sessions.len()))
		.then(|| Segment {
			seq,
			cols: cols.pack(),
			visitors,
			sessions,
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
