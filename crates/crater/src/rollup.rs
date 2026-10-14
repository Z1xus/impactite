use std::collections::HashMap;
use std::io;
use std::path::Path;

use crate::codec::{Reader, put_uvar};
use crate::dict::Dict;
use crate::event::Dim;
use crate::frame::{self, invalid};
use crate::query::{MISSING, Matcher, Slot};
use crate::segment::Columns;

const MAGIC: &[u8; 5] = b"CRRU\x02";
const PROP_KEYS: usize = 64;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Metrics {
	pub visitors: u64,
	pub visits: u64,
	pub pageviews: u64,
	pub events: u64,
	pub bounces: u64,
	pub duration: u64,
}

impl Metrics {
	pub(crate) fn merge(&mut self, other: &Metrics) {
		self.visitors += other.visitors;
		self.visits += other.visits;
		self.pageviews += other.pageviews;
		self.events += other.events;
		self.bounces += other.bounces;
		self.duration += other.duration;
	}

	fn add_visit(&mut self, visit: &Visit) {
		self.visits += 1;
		self.bounces += u64::from(visit.is_bounce());
		self.duration += u64::from(visit.end - visit.start);
	}

	fn put(&self, buf: &mut Vec<u8>) {
		for value in [
			self.visitors,
			self.visits,
			self.pageviews,
			self.events,
			self.bounces,
			self.duration,
		] {
			put_uvar(buf, value);
		}
	}

	fn read(r: &mut Reader) -> Option<Self> {
		Some(Self {
			visitors: r.uvar()?,
			visits: r.uvar()?,
			pageviews: r.uvar()?,
			events: r.uvar()?,
			bounces: r.uvar()?,
			duration: r.uvar()?,
		})
	}
}

pub(crate) struct Names {
	pageview: Option<u32>,
	engagement: Option<u32>,
}

impl Names {
	pub(crate) fn new(dict: &Dict) -> Self {
		Self {
			pageview: dict.find("pageview"),
			engagement: dict.find("engagement"),
		}
	}
}

#[derive(Clone, Copy)]
struct Visit {
	visitor: u32,
	first: Option<(usize, usize, u32)>,
	entry: (u32, u32),
	exit: (u32, u32),
	start: u32,
	end: u32,
	pageviews: u32,
	engaged: bool,
	selected: bool,
	hit_pageviews: u64,
	hit_events: u64,
}

impl Visit {
	fn is_bounce(&self) -> bool {
		self.pageviews < 2 && !self.engaged
	}
}

pub(crate) struct Counts {
	pub(crate) seq: u64,
	pub(crate) total: Metrics,
	pub(crate) hours: [Metrics; 24],
	pub(crate) fields: HashMap<Slot, Vec<(u32, Metrics)>>,
}

struct Scan<'a> {
	parts: &'a [&'a Columns],
	offsets: Vec<Vec<u32>>,
	names: &'a Names,
}

impl Scan<'_> {
	fn rows(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
		self.parts
			.iter()
			.enumerate()
			.flat_map(|(p, part)| (0..part.rows()).map(move |row| (p, row)))
	}

	fn name(&self, p: usize, row: usize) -> Option<u32> {
		Some(self.parts[p].dims[Dim::Name as usize][row])
	}

	fn is_pageview(&self, p: usize, row: usize) -> bool {
		self.name(p, row) == self.names.pageview
	}

	fn is_engagement(&self, p: usize, row: usize) -> bool {
		self.name(p, row) == self.names.engagement
	}

	fn prop(&self, p: usize, row: usize, key: u32) -> u32 {
		let part = self.parts[p];
		let start = self.offsets[p][row] as usize;
		let end = start + part.props[row] as usize;
		(start..end)
			.find(|&i| part.keys[i] == key)
			.map_or(MISSING, |i| part.values[i])
	}

	fn event_value(&self, slot: Slot, p: usize, row: usize) -> u32 {
		match slot {
			Slot::Dim(dim) => self.parts[p].dims[dim as usize][row],
			Slot::Prop(key) => self.prop(p, row, key),
			Slot::Entry | Slot::Exit => MISSING,
		}
	}

	fn visit_value(&self, slot: Slot, visit: &Visit) -> u32 {
		match slot {
			Slot::Entry => visit.entry.1,
			Slot::Exit => visit.exit.1,
			slot => visit
				.first
				.map_or(MISSING, |(p, row, _)| self.event_value(slot, p, row)),
		}
	}
}

impl Counts {
	pub(crate) fn empty() -> Self {
		Self {
			seq: 0,
			total: Metrics::default(),
			hours: [Metrics::default(); 24],
			fields: HashMap::new(),
		}
	}

	pub(crate) fn rollup_slots(parts: &[&Columns]) -> Vec<Slot> {
		let mut keys: HashMap<u32, usize> = HashMap::new();
		for part in parts {
			for &key in &part.keys {
				*keys.entry(key).or_default() += 1;
			}
		}
		let mut keys: Vec<(u32, usize)> = keys.into_iter().collect();
		keys.sort_unstable_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
		Dim::ALL
			.into_iter()
			.map(Slot::Dim)
			.chain([Slot::Entry, Slot::Exit])
			.chain(
				keys.into_iter()
					.take(PROP_KEYS)
					.map(|(key, _)| Slot::Prop(key)),
			)
			.collect()
	}

	pub(crate) fn scan(
		parts: &[&Columns],
		visitors: usize,
		sessions: usize,
		names: &Names,
		filters: &[Matcher],
		slots: &[Slot],
	) -> Self {
		let scan = Scan {
			parts,
			offsets: parts
				.iter()
				.map(|part| {
					part.props
						.iter()
						.scan(0, |sum, &n| {
							let at = *sum;
							*sum += n;
							Some(at)
						})
						.collect()
				})
				.collect(),
			names,
		};
		let (event_filters, visit_filters): (Vec<&Matcher>, Vec<&Matcher>) =
			filters.iter().partition(|m| m.slot.is_event());

		let mut visits = vec![
			Visit {
				visitor: 0,
				first: None,
				entry: (u32::MAX, MISSING),
				exit: (0, MISSING),
				start: u32::MAX,
				end: 0,
				pageviews: 0,
				engaged: false,
				selected: false,
				hit_pageviews: 0,
				hit_events: 0,
			};
			sessions
		];
		for (p, row) in scan.rows() {
			let part = parts[p];
			let time = part.time[row];
			let visit = &mut visits[part.session[row] as usize];
			visit.visitor = part.visitor[row];
			visit.start = visit.start.min(time);
			visit.end = visit.end.max(time);
			if scan.is_engagement(p, row) {
				continue;
			}
			if visit.first.is_none_or(|(_, _, first)| time < first) {
				visit.first = Some((p, row, time));
			}
			if scan.is_pageview(p, row) {
				visit.pageviews += 1;
				let page = part.dims[Dim::Page as usize][row];
				if time < visit.entry.0 || visit.entry.1 == MISSING {
					visit.entry = (time, page);
				}
				if time >= visit.exit.0 {
					visit.exit = (time, page);
				}
			} else if part.interactive[row] == 1 {
				visit.engaged = true;
			}
		}
		for visit in &mut visits {
			visit.selected = visit.first.is_some()
				&& visit_filters
					.iter()
					.all(|m| m.matches(scan.visit_value(m.slot, visit)));
		}

		let is_hit = |visits: &[Visit], p: usize, row: usize| {
			visits[parts[p].session[row] as usize].selected
				&& !scan.is_engagement(p, row)
				&& event_filters
					.iter()
					.all(|m| m.matches(scan.event_value(m.slot, p, row)))
		};
		let mut hours = [Metrics::default(); 24];
		let mut visitor_hours = vec![0u32; visitors];
		for (p, row) in scan.rows() {
			if !is_hit(&visits, p, row) {
				continue;
			}
			let part = parts[p];
			let pageview = u64::from(scan.is_pageview(p, row));
			let hour = (part.time[row] / 3600) as usize;
			let visit = &mut visits[part.session[row] as usize];
			visit.hit_events += 1;
			visit.hit_pageviews += pageview;
			hours[hour].events += 1;
			hours[hour].pageviews += pageview;
			visitor_hours[part.visitor[row] as usize] |= 1 << hour;
		}
		for visit in &mut visits {
			visit.selected &= visit.hit_events > 0;
		}

		let mut total = Metrics::default();
		for visit in visits.iter().filter(|visit| visit.selected) {
			total.add_visit(visit);
			total.pageviews += visit.hit_pageviews;
			total.events += visit.hit_events;
			hours[(visit.start / 3600) as usize].add_visit(visit);
		}
		for mask in visitor_hours.into_iter().filter(|&mask| mask != 0) {
			total.visitors += 1;
			for (hour, metrics) in hours.iter_mut().enumerate() {
				metrics.visitors += u64::from(mask >> hour & 1);
			}
		}

		let mut fields = HashMap::new();
		for &slot in slots {
			let mut entries = match slot.is_event() {
				true => event_breakdown(
					scan.rows()
						.filter(|&(p, row)| is_hit(&visits, p, row))
						.map(|(p, row)| {
							let session = parts[p].session[row];
							(
								scan.event_value(slot, p, row),
								session,
								scan.is_pageview(p, row),
							)
						})
						.filter(|hit| hit.0 != MISSING)
						.collect(),
					&visits,
				),
				false => visit_breakdown(
					visits
						.iter()
						.filter(|visit| visit.selected)
						.map(|visit| (scan.visit_value(slot, visit), visit))
						.filter(|pair| pair.0 != MISSING)
						.collect(),
				),
			};
			entries.sort_unstable_by(|a, b| {
				b.1.visitors
					.cmp(&a.1.visitors)
					.then(b.1.pageviews.cmp(&a.1.pageviews))
					.then(a.0.cmp(&b.0))
			});
			fields.insert(slot, entries);
		}

		Self {
			seq: 0,
			total,
			hours,
			fields,
		}
	}

	pub(crate) fn write(&self, path: &Path) -> io::Result<()> {
		let mut buf = Vec::new();
		frame::put(&mut buf, |buf| {
			put_uvar(buf, self.seq);
			self.total.put(buf);
			for hour in &self.hours {
				hour.put(buf);
			}
			put_uvar(buf, self.fields.len() as u64);
			for (slot, entries) in &self.fields {
				match *slot {
					Slot::Dim(dim) => buf.extend([0, dim as u8]),
					Slot::Entry => buf.push(1),
					Slot::Exit => buf.push(2),
					Slot::Prop(key) => {
						buf.push(3);
						put_uvar(buf, key.into());
					}
				}
				put_uvar(buf, entries.len() as u64);
				for (id, metrics) in entries {
					put_uvar(buf, (*id).into());
					metrics.put(buf);
				}
			}
		});
		frame::write_file(path, MAGIC, &buf)
	}

	pub(crate) fn read(path: &Path) -> io::Result<Self> {
		let frames = frame::read_file(path, MAGIC, 1)?;
		frames
			.first()
			.and_then(|buf| decode(buf))
			.ok_or_else(|| invalid("bad rollup"))
	}
}

fn visit_breakdown(mut pairs: Vec<(u32, &Visit)>) -> Vec<(u32, Metrics)> {
	pairs.sort_unstable_by_key(|&(value, visit)| (value, visit.visitor));
	pairs
		.chunk_by(|a, b| a.0 == b.0)
		.map(|group| {
			let mut metrics = Metrics::default();
			for &(_, visit) in group {
				metrics.add_visit(visit);
				metrics.pageviews += visit.hit_pageviews;
				metrics.events += visit.hit_events;
			}
			metrics.visitors = group.chunk_by(|a, b| a.1.visitor == b.1.visitor).count() as u64;
			(group[0].0, metrics)
		})
		.collect()
}

fn event_breakdown(mut hits: Vec<(u32, u32, bool)>, visits: &[Visit]) -> Vec<(u32, Metrics)> {
	hits.sort_unstable();
	hits.chunk_by(|a, b| a.0 == b.0)
		.map(|group| {
			let mut metrics = Metrics {
				events: group.len() as u64,
				pageviews: group.iter().filter(|hit| hit.2).count() as u64,
				..Metrics::default()
			};
			let mut visitors = Vec::new();
			for same in group.chunk_by(|a, b| a.1 == b.1) {
				let visit = &visits[same[0].1 as usize];
				metrics.add_visit(visit);
				visitors.push(visit.visitor);
			}
			visitors.sort_unstable();
			visitors.dedup();
			metrics.visitors = visitors.len() as u64;
			(group[0].0, metrics)
		})
		.collect()
}

fn decode(buf: &[u8]) -> Option<Counts> {
	let mut r = Reader::new(buf);
	let seq = r.uvar()?;
	let total = Metrics::read(&mut r)?;
	let mut hours = [Metrics::default(); 24];
	for hour in &mut hours {
		*hour = Metrics::read(&mut r)?;
	}
	let mut fields = HashMap::new();
	for _ in 0..r.uvar()? {
		let slot = match r.u8()? {
			0 => Slot::Dim(*Dim::ALL.get(usize::from(r.u8()?))?),
			1 => Slot::Entry,
			2 => Slot::Exit,
			3 => Slot::Prop(u32::try_from(r.uvar()?).ok()?),
			_ => return None,
		};
		let entries = (0..r.uvar()?)
			.map(|_| Some((u32::try_from(r.uvar()?).ok()?, Metrics::read(&mut r)?)))
			.collect::<Option<Vec<_>>>()?;
		fields.insert(slot, entries);
	}
	r.is_empty().then_some(Counts {
		seq,
		total,
		hours,
		fields,
	})
}
