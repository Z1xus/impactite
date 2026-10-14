use std::io;
use std::path::Path;

use crate::codec::{Reader, put_uvar};
use crate::dict::Dict;
use crate::event::Dim;
use crate::frame::{self, invalid};
use crate::query::Matcher;
use crate::segment::Columns;

const MAGIC: &[u8; 5] = b"CRRU\x01";

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
	pub(crate) dims: Vec<Vec<(u32, Metrics)>>,
}

impl Counts {
	pub(crate) fn empty() -> Self {
		Self {
			seq: 0,
			total: Metrics::default(),
			hours: [Metrics::default(); 24],
			dims: vec![Vec::new(); Dim::COUNT],
		}
	}

	pub(crate) fn scan(
		parts: &[&Columns],
		visitors: usize,
		sessions: usize,
		names: &Names,
		filters: &[Matcher],
		dims: &[Dim],
	) -> Self {
		let rows = || {
			parts
				.iter()
				.enumerate()
				.flat_map(|(p, part)| (0..part.rows()).map(move |row| (p, row)))
		};
		let name = |p: usize, row: usize| Some(parts[p].dims[Dim::Name as usize][row]);
		let (event_filters, visit_filters): (Vec<&Matcher>, Vec<&Matcher>) =
			filters.iter().partition(|m| m.dim.is_event());

		let mut visits = vec![
			Visit {
				visitor: 0,
				first: None,
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
		for (p, row) in rows() {
			let part = parts[p];
			let time = part.time[row];
			let visit = &mut visits[part.session[row] as usize];
			visit.visitor = part.visitor[row];
			visit.start = visit.start.min(time);
			visit.end = visit.end.max(time);
			if name(p, row) == names.engagement {
				continue;
			}
			if visit.first.is_none_or(|(_, _, first)| time < first) {
				visit.first = Some((p, row, time));
			}
			if name(p, row) == names.pageview {
				visit.pageviews += 1;
			} else if part.interactive[row] == 1 {
				visit.engaged = true;
			}
		}
		for visit in &mut visits {
			visit.selected = visit.first.is_some_and(|(p, row, _)| {
				visit_filters
					.iter()
					.all(|m| m.matches(parts[p].dims[m.dim as usize][row]))
			});
		}

		let is_hit = |visits: &[Visit], p: usize, row: usize| {
			let part = parts[p];
			visits[part.session[row] as usize].selected
				&& name(p, row) != names.engagement
				&& event_filters
					.iter()
					.all(|m| m.matches(part.dims[m.dim as usize][row]))
		};
		let mut hours = [Metrics::default(); 24];
		let mut visitor_hours = vec![0u32; visitors];
		for (p, row) in rows() {
			if !is_hit(&visits, p, row) {
				continue;
			}
			let part = parts[p];
			let pageview = u64::from(name(p, row) == names.pageview);
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

		let mut breakdowns = vec![Vec::new(); Dim::COUNT];
		for &dim in dims {
			let column = |p: usize, row: usize| parts[p].dims[dim as usize][row];
			let mut entries = match dim.is_event() {
				true => {
					let hits = rows()
						.filter(|&(p, row)| is_hit(&visits, p, row))
						.map(|(p, row)| {
							let session = parts[p].session[row];
							(column(p, row), session, name(p, row) == names.pageview)
						})
						.collect();
					event_breakdown(hits, &visits)
				}
				false => visit_breakdown(
					visits
						.iter()
						.filter(|visit| visit.selected)
						.filter_map(|visit| {
							let (p, row, _) = visit.first?;
							Some((column(p, row), visit))
						})
						.collect(),
				),
			};
			entries.sort_unstable_by(|a, b| {
				b.1.visitors
					.cmp(&a.1.visitors)
					.then(b.1.pageviews.cmp(&a.1.pageviews))
					.then(a.0.cmp(&b.0))
			});
			breakdowns[dim as usize] = entries;
		}

		Self {
			seq: 0,
			total,
			hours,
			dims: breakdowns,
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
			for entries in &self.dims {
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
	let dims = (0..Dim::COUNT)
		.map(|_| {
			(0..r.uvar()?)
				.map(|_| Some((u32::try_from(r.uvar()?).ok()?, Metrics::read(&mut r)?)))
				.collect::<Option<Vec<_>>>()
		})
		.collect::<Option<Vec<_>>>()?;
	r.is_empty().then_some(Counts {
		seq,
		total,
		hours,
		dims,
	})
}
