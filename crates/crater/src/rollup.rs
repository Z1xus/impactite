use std::collections::{HashMap, HashSet};
use std::io;
use std::path::Path;

use crate::codec::{Reader, put_uvar};
use crate::dict::Dict;
use crate::event::Dim;
use crate::frame::{self, invalid};
use crate::query::{MISSING, Matcher, Slot};
use crate::segment::Columns;

const MAGIC: &[u8; 5] = b"CRRU\x04";
// older versions and how many metrics they stored, imported days still use them
const LEGACY: [(&[u8; 5], usize); 2] = [(b"CRRU\x03", 11), (b"CRRU\x02", 6)];
const FIELDS: usize = 12;
const PROP_KEYS: usize = 64;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Metrics {
	pub visitors: u64,
	pub visits: u64,
	pub pageviews: u64,
	pub events: u64,
	pub bounces: u64,
	pub duration: u64,
	pub exits: u64,
	pub engaged: u64,
	pub engaged_views: u64,
	pub scroll: u64,
	pub scrolled: u64,
	pub revenue: u64,
}

impl Metrics {
	pub fn merge(&mut self, other: &Metrics) {
		self.visitors += other.visitors;
		self.visits += other.visits;
		self.pageviews += other.pageviews;
		self.events += other.events;
		self.bounces += other.bounces;
		self.duration += other.duration;
		self.exits += other.exits;
		self.engaged += other.engaged;
		self.engaged_views += other.engaged_views;
		self.scroll += other.scroll;
		self.scrolled += other.scrolled;
		self.revenue += other.revenue;
	}

	fn engage(&mut self, (ms, scroll): (u64, u32)) {
		self.engaged += ms;
		self.engaged_views += u64::from(ms > 0);
		if scroll > 0 {
			self.scroll += u64::from(scroll - 1);
			self.scrolled += 1;
		}
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
			self.exits,
			self.engaged,
			self.engaged_views,
			self.scroll,
			self.scrolled,
			self.revenue,
		] {
			put_uvar(buf, value);
		}
	}

	fn read(r: &mut Reader, fields: usize) -> Option<Self> {
		let mut values = [0; FIELDS];
		for value in values.iter_mut().take(fields) {
			*value = r.uvar()?;
		}
		let [
			visitors,
			visits,
			pageviews,
			events,
			bounces,
			duration,
			exits,
			engaged,
			engaged_views,
			scroll,
			scrolled,
			revenue,
		] = values;
		Some(Self {
			visitors,
			visits,
			pageviews,
			events,
			bounces,
			duration,
			exits,
			engaged,
			engaged_views,
			scroll,
			scrolled,
			revenue,
		})
	}
}

pub(crate) struct Names {
	pageview: Option<u32>,
	engagement: Option<u32>,
	// names and prop keys starting with `~` belong to impactite, a query only sees them by name
	internal: HashSet<u32>,
}

impl Names {
	pub(crate) fn new(dict: &Dict) -> Self {
		Self {
			pageview: dict.find("pageview"),
			engagement: dict.find("engagement"),
			internal: dict.matching(|s| s.starts_with('~')),
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
	hit_revenue: u64,
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
	// pageviews per minute from `since`, only for windowed scans
	pub(crate) minutes: Vec<u64>,
}

struct Scan<'a> {
	parts: &'a [&'a Columns],
	offsets: Vec<Vec<u32>>,
	names: &'a Names,
	since: u32,
}

impl<'a> Scan<'a> {
	fn new(parts: &'a [&'a Columns], names: &'a Names, since: u32) -> Self {
		// only days with props need where each row's props start
		let offsets = parts
			.iter()
			.map(|part| match part.keys.len() {
				0 => Vec::new(),
				_ => part
					.props
					.iter()
					.scan(0, |sum, n| {
						let at = *sum;
						*sum += n;
						Some(at)
					})
					.collect(),
			})
			.collect();
		Self {
			parts,
			offsets,
			names,
			since,
		}
	}

	fn rows(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
		self.parts.iter().enumerate().flat_map(move |(p, part)| {
			(0..part.rows())
				.filter(move |&row| part.time.get(row) >= self.since)
				.map(move |row| (p, row))
		})
	}

	fn name(&self, p: usize, row: usize) -> Option<u32> {
		Some(self.parts[p].dims[Dim::Name as usize].get(row))
	}

	fn is_pageview(&self, p: usize, row: usize) -> bool {
		self.name(p, row) == self.names.pageview
	}

	fn is_engagement(&self, p: usize, row: usize) -> bool {
		self.name(p, row) == self.names.engagement
	}

	fn is_internal(&self, p: usize, row: usize) -> bool {
		self.name(p, row)
			.is_some_and(|name| self.names.internal.contains(&name))
	}

	fn prop(&self, p: usize, row: usize, key: u32) -> u32 {
		let part = self.parts[p];
		let Some(&start) = self.offsets[p].get(row) else {
			return MISSING;
		};
		let start = start as usize;
		let end = start + part.props.get(row) as usize;
		(start..end)
			.find(|&i| part.keys.get(i) == key)
			.map_or(MISSING, |i| part.values.get(i))
	}

	fn event_value(&self, slot: Slot, p: usize, row: usize) -> u32 {
		match slot {
			Slot::Dim(dim) => self.parts[p].dims[dim as usize].get(row),
			Slot::Prop(key) => self.prop(p, row, key),
			Slot::Scroll => self.parts[p].scroll.get(row),
			Slot::Currency => self.parts[p].currency.get(row),
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
			minutes: Vec::new(),
		}
	}

	pub(crate) fn rollup_slots(parts: &[&Columns], names: &Names) -> Vec<Slot> {
		let mut keys: HashMap<u32, usize> = HashMap::new();
		for part in parts {
			for key in part.keys.iter().filter(|key| !names.internal.contains(key)) {
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
		since: Option<u32>,
	) -> Self {
		let scan = Scan::new(parts, names, since.unwrap_or(0));
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
				hit_revenue: 0,
			};
			sessions
		];
		for (p, row) in scan.rows() {
			let part = parts[p];
			let time = part.time.get(row);
			let visit = &mut visits[part.session.get(row) as usize];
			visit.visitor = part.visitor.get(row);
			visit.start = visit.start.min(time);
			visit.end = visit.end.max(time);
			if scan.is_engagement(p, row) || scan.is_internal(p, row) {
				continue;
			}
			if visit.first.is_none_or(|(_, _, first)| time < first) {
				visit.first = Some((p, row, time));
			}
			if scan.is_pageview(p, row) {
				visit.pageviews += 1;
				let page = part.dims[Dim::Page as usize].get(row);
				if time < visit.entry.0 || visit.entry.1 == MISSING {
					visit.entry = (time, page);
				}
				if time >= visit.exit.0 {
					visit.exit = (time, page);
				}
			} else if part.interactive.get(row) == 1 {
				visit.engaged = true;
			}
		}
		for visit in &mut visits {
			visit.selected = visit.first.is_some()
				&& visit_filters
					.iter()
					.all(|m| m.matches(scan.visit_value(m.slot, visit)));
		}

		// a scroll filter can only match engagement events, which carry the depth
		let scrolling = event_filters.iter().any(|m| m.slot == Slot::Scroll);
		let named = event_filters
			.iter()
			.any(|m| m.selects(Slot::Dim(Dim::Name)));
		let is_hit = |visits: &[Visit], p: usize, row: usize| {
			visits[parts[p].session.get(row) as usize].selected
				&& scan.is_engagement(p, row) == scrolling
				&& (named || !scan.is_internal(p, row))
				&& event_filters
					.iter()
					.all(|m| m.matches(scan.event_value(m.slot, p, row)))
		};
		let mut hours = [Metrics::default(); 24];
		let mut minutes = vec![0; since.map_or(0, |since| ((86_400 - since) / 60 + 1) as usize)];
		let mut visitor_hours = vec![0u32; visitors];
		for (p, row) in scan.rows() {
			if !is_hit(&visits, p, row) {
				continue;
			}
			let part = parts[p];
			let pageview = u64::from(scan.is_pageview(p, row));
			let hour = (part.time.get(row) / 3600) as usize;
			let visit = &mut visits[part.session.get(row) as usize];
			let revenue = u64::from(part.revenue.get(row));
			visit.hit_events += 1;
			visit.hit_pageviews += pageview;
			visit.hit_revenue += revenue;
			hours[hour].events += 1;
			hours[hour].pageviews += pageview;
			hours[hour].revenue += revenue;
			if let Some(since) = since {
				minutes[((part.time.get(row) - since) / 60) as usize] += pageview;
			}
			visitor_hours[part.visitor.get(row) as usize] |= 1 << hour;
		}
		for visit in &mut visits {
			visit.selected &= visit.hit_events > 0;
		}

		let mut total = Metrics::default();
		for visit in visits.iter().filter(|visit| visit.selected) {
			total.add_visit(visit);
			total.pageviews += visit.hit_pageviews;
			total.events += visit.hit_events;
			total.revenue += visit.hit_revenue;
			hours[(visit.start / 3600) as usize].add_visit(visit);
		}
		for mask in visitor_hours.into_iter().filter(|&mask| mask != 0) {
			total.visitors += 1;
			for (hour, metrics) in hours.iter_mut().enumerate() {
				metrics.visitors += u64::from(mask >> hour & 1);
			}
		}

		// max scroll and summed engagement per (page, session), only for pages and totals
		let mut engaged: HashMap<(u32, u32), (u64, u32)> = HashMap::new();
		for (p, row) in scan.rows() {
			let part = parts[p];
			let session = part.session.get(row);
			if !visits[session as usize].selected
				|| !scan.is_engagement(p, row)
				|| !event_filters
					.iter()
					.all(|m| m.matches(scan.event_value(m.slot, p, row)))
			{
				continue;
			}
			let page = part.dims[Dim::Page as usize].get(row);
			let entry = engaged.entry((page, session)).or_default();
			entry.0 += u64::from(part.engagement.get(row));
			entry.1 = entry.1.max(part.scroll.get(row));
		}
		for &pair in engaged.values() {
			total.engage(pair);
		}
		total.exits = total.visits;

		let mut fields = HashMap::new();
		for &slot in slots {
			let mut entries = if slot.is_event() {
				// value and session packed in one word keeps this small on big days
				let mut pairs = Vec::new();
				let mut sums: HashMap<u32, Metrics> = HashMap::new();
				for (p, row) in scan.rows().filter(|&(p, row)| is_hit(&visits, p, row)) {
					let value = scan.event_value(slot, p, row);
					if value == MISSING {
						continue;
					}
					let session = parts[p].session.get(row);
					pairs.push(u64::from(value) << 32 | u64::from(session));
					let sum = sums.entry(value).or_default();
					sum.events += 1;
					sum.pageviews += u64::from(scan.is_pageview(p, row));
					sum.revenue += u64::from(parts[p].revenue.get(row));
				}
				event_breakdown(pairs, &sums, &visits)
			} else {
				visit_breakdown(
					visits
						.iter()
						.filter(|visit| visit.selected)
						.map(|visit| (scan.visit_value(slot, visit), visit))
						.filter(|pair| pair.0 != MISSING)
						.collect(),
				)
			};
			if slot == Slot::Dim(Dim::Page) {
				let index: HashMap<u32, usize> = entries
					.iter()
					.enumerate()
					.map(|(i, &(id, _))| (id, i))
					.collect();
				for (&(page, _), &pair) in &engaged {
					if let Some(&i) = index.get(&page) {
						entries[i].1.engage(pair);
					}
				}
				for visit in visits.iter().filter(|visit| visit.selected) {
					if let Some(&i) = index.get(&visit.exit.1) {
						entries[i].1.exits += 1;
					}
				}
			}
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
			minutes,
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
					Slot::Scroll => buf.push(4),
					Slot::Currency => buf.push(5),
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
			.and_then(|buf| decode(buf, FIELDS))
			.ok_or_else(|| invalid("bad rollup"))
	}

	pub(crate) fn read_any(path: &Path) -> io::Result<Self> {
		let mut read = Self::read(path);
		for (magic, fields) in LEGACY {
			read = match read {
				Err(err) if err.kind() == io::ErrorKind::InvalidData => {
					frame::read_file(path, magic, 1)?
						.first()
						.and_then(|buf| decode(buf, fields))
						.ok_or_else(|| invalid("bad rollup"))
				}
				read => return read,
			};
		}
		read
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
				metrics.revenue += visit.hit_revenue;
			}
			metrics.visitors = group.chunk_by(|a, b| a.1.visitor == b.1.visitor).count() as u64;
			(group[0].0, metrics)
		})
		.collect()
}

fn event_breakdown(
	mut pairs: Vec<u64>,
	sums: &HashMap<u32, Metrics>,
	visits: &[Visit],
) -> Vec<(u32, Metrics)> {
	pairs.sort_unstable();
	pairs.dedup();
	pairs
		.chunk_by(|a, b| a >> 32 == b >> 32)
		.map(|group| {
			let value = (group[0] >> 32) as u32;
			let mut metrics = sums.get(&value).copied().unwrap_or_default();
			let mut visitors = Vec::new();
			for &pair in group {
				let visit = &visits[(pair & u64::from(u32::MAX)) as usize];
				metrics.add_visit(visit);
				visitors.push(visit.visitor);
			}
			visitors.sort_unstable();
			visitors.dedup();
			metrics.visitors = visitors.len() as u64;
			(value, metrics)
		})
		.collect()
}

fn decode(buf: &[u8], width: usize) -> Option<Counts> {
	let mut r = Reader::new(buf);
	let seq = r.uvar()?;
	let total = Metrics::read(&mut r, width)?;
	let mut hours = [Metrics::default(); 24];
	for hour in &mut hours {
		*hour = Metrics::read(&mut r, width)?;
	}
	let mut fields = HashMap::new();
	for _ in 0..r.uvar()? {
		let slot = match r.u8()? {
			0 => Slot::Dim(*Dim::ALL.get(usize::from(r.u8()?))?),
			1 => Slot::Entry,
			2 => Slot::Exit,
			3 => Slot::Prop(u32::try_from(r.uvar()?).ok()?),
			4 => Slot::Scroll,
			5 => Slot::Currency,
			_ => return None,
		};
		let entries = (0..r.uvar()?)
			.map(|_| {
				Some((
					u32::try_from(r.uvar()?).ok()?,
					Metrics::read(&mut r, width)?,
				))
			})
			.collect::<Option<Vec<_>>>()?;
		fields.insert(slot, entries);
	}
	r.is_empty().then_some(Counts {
		seq,
		total,
		hours,
		fields,
		minutes: Vec::new(),
	})
}

// visitors reaching each step in order within a session, filtered by visit level matchers
pub(crate) fn funnel(
	parts: &[&Columns],
	names: &Names,
	filters: &[Matcher],
	steps: &[Vec<Matcher>],
) -> Vec<u64> {
	let scan = Scan::new(parts, names, 0);
	let mut rows: Vec<(u32, u32, usize, usize)> = scan
		.rows()
		.map(|(p, row)| (parts[p].session.get(row), parts[p].time.get(row), p, row))
		.collect();
	rows.sort_unstable();
	let matches = |matchers: &[Matcher], p: usize, row: usize| {
		let scrolling = matchers.iter().any(|m| m.slot == Slot::Scroll);
		scan.is_engagement(p, row) == scrolling
			&& !scan.is_internal(p, row)
			&& matchers
				.iter()
				.all(|m| m.matches(scan.event_value(m.slot, p, row)))
	};
	let mut reached: Vec<Vec<u32>> = vec![Vec::new(); steps.len()];
	for session in rows.chunk_by(|a, b| a.0 == b.0) {
		let first = session
			.iter()
			.find(|&&(_, _, p, row)| !scan.is_engagement(p, row) && !scan.is_internal(p, row));
		let Some(&(_, _, p, row)) = first else {
			continue;
		};
		if !filters
			.iter()
			.all(|m| m.matches(scan.event_value(m.slot, p, row)))
		{
			continue;
		}
		let mut progress = 0;
		for &(_, _, p, row) in session {
			if progress < steps.len() && matches(&steps[progress], p, row) {
				progress += 1;
			}
		}
		for step in &mut reached[..progress] {
			step.push(parts[p].visitor.get(row));
		}
	}
	reached
		.into_iter()
		.map(|mut visitors| {
			visitors.sort_unstable();
			visitors.dedup();
			visitors.len() as u64
		})
		.collect()
}
