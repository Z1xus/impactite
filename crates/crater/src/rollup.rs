use std::io;
use std::path::Path;
use std::sync::Arc;

use crate::codec::{Reader, put_uvar};
use crate::dict::Dict;
use crate::event::Dim;
use crate::frame::{self, invalid};
use crate::segment::Segment;

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
	fn add_visit(&mut self, visit: &Visit) {
		self.visits += 1;
		self.bounces += u64::from(visit.is_bounce());
		self.duration += visit.duration();
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

#[derive(Debug, Clone)]
pub struct Rollup {
	pub total: Metrics,
	pub hours: [Metrics; 24],
	dims: Vec<Vec<(Arc<str>, Metrics)>>,
}

impl Rollup {
	pub fn dim(&self, dim: Dim) -> &[(Arc<str>, Metrics)] {
		&self.dims[dim as usize]
	}
}

pub(crate) struct Counts {
	pub(crate) seq: u64,
	total: Metrics,
	hours: [Metrics; 24],
	dims: Vec<Vec<(u32, Metrics)>>,
}

struct Visit {
	visitor: u32,
	first: usize,
	start: u32,
	end: u32,
	pageviews: u64,
	events: u64,
	engaged: bool,
}

impl Visit {
	fn is_bounce(&self) -> bool {
		self.pageviews < 2 && !self.engaged
	}

	fn duration(&self) -> u64 {
		(self.end - self.start).into()
	}
}

impl Counts {
	pub(crate) fn compute(seg: &Segment, dict: &Dict) -> Self {
		let names = &seg.dims[Dim::Name as usize];
		let pageview = dict.find("pageview");
		let engagement = dict.find("engagement");
		let is_pageview = |row: usize| Some(names[row]) == pageview;
		let is_engagement = |row: usize| Some(names[row]) == engagement;

		let rows = seg.rows();
		let mut visits = Vec::new();
		let mut row_visit = vec![None; rows];
		let mut start = 0;
		while start < rows {
			let session = seg.session[start];
			let end = seg.session[start..]
				.iter()
				.position(|&s| s != session)
				.map_or(rows, |len| start + len);
			let mut first = None;
			let mut visit = Visit {
				visitor: seg.visitor[start],
				first: 0,
				start: seg.time[start],
				end: seg.time[end - 1],
				pageviews: 0,
				events: 0,
				engaged: false,
			};
			for row in (start..end).filter(|&row| !is_engagement(row)) {
				first.get_or_insert(row);
				visit.events += 1;
				if is_pageview(row) {
					visit.pageviews += 1;
				} else if seg.interactive[row] == 1 {
					visit.engaged = true;
				}
			}
			if let Some(first) = first {
				visit.first = first;
				row_visit[start..end].fill(Some(visits.len() as u32));
				visits.push(visit);
			}
			start = end;
		}

		let counted: Vec<(usize, u32)> = (0..rows)
			.filter(|&row| !is_engagement(row))
			.filter_map(|row| Some((row, row_visit[row]?)))
			.collect();

		let mut total = Metrics::default();
		let mut hours = [Metrics::default(); 24];
		for visit in &visits {
			total.add_visit(visit);
			total.pageviews += visit.pageviews;
			total.events += visit.events;
			hours[(visit.start / 3600) as usize].add_visit(visit);
		}
		total.visitors = distinct(visits.iter().map(|visit| visit.visitor).collect());

		let mut hour_visitors = Vec::with_capacity(counted.len());
		for &(row, _) in &counted {
			let hour = seg.time[row] / 3600;
			hours[hour as usize].events += 1;
			hours[hour as usize].pageviews += u64::from(is_pageview(row));
			hour_visitors.push((hour, seg.visitor[row]));
		}
		hour_visitors.sort_unstable();
		hour_visitors.dedup();
		for (hour, _) in hour_visitors {
			hours[hour as usize].visitors += 1;
		}

		let dims = Dim::ALL
			.into_iter()
			.map(|dim| {
				let column = &seg.dims[dim as usize];
				let mut entries = match dim.is_event() {
					true => event_breakdown(column, &counted, &visits, is_pageview),
					false => visit_breakdown(column, &visits),
				};
				entries.sort_unstable_by(|a, b| {
					b.1.visitors
						.cmp(&a.1.visitors)
						.then(b.1.pageviews.cmp(&a.1.pageviews))
						.then(a.0.cmp(&b.0))
				});
				entries
			})
			.collect();

		Self {
			seq: seg.seq,
			total,
			hours,
			dims,
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

	pub(crate) fn resolve(self, dict: &Dict) -> io::Result<Rollup> {
		let dims = self
			.dims
			.into_iter()
			.map(|entries| {
				entries
					.into_iter()
					.map(|(id, metrics)| Some((dict.get(id)?.clone(), metrics)))
					.collect::<Option<Vec<_>>>()
			})
			.collect::<Option<Vec<_>>>()
			.ok_or_else(|| invalid("unknown string id"))?;
		Ok(Rollup {
			total: self.total,
			hours: self.hours,
			dims,
		})
	}
}

fn visit_breakdown(column: &[u32], visits: &[Visit]) -> Vec<(u32, Metrics)> {
	let mut pairs: Vec<(u32, &Visit)> = visits
		.iter()
		.map(|visit| (column[visit.first], visit))
		.collect();
	pairs.sort_unstable_by_key(|&(value, _)| value);
	pairs
		.chunk_by(|a, b| a.0 == b.0)
		.map(|group| {
			let mut metrics = Metrics::default();
			for &(_, visit) in group {
				metrics.add_visit(visit);
				metrics.pageviews += visit.pageviews;
				metrics.events += visit.events;
			}
			metrics.visitors = distinct(group.iter().map(|(_, visit)| visit.visitor).collect());
			(group[0].0, metrics)
		})
		.collect()
}

fn event_breakdown(
	column: &[u32],
	counted: &[(usize, u32)],
	visits: &[Visit],
	is_pageview: impl Fn(usize) -> bool,
) -> Vec<(u32, Metrics)> {
	let mut rows: Vec<(u32, u32, bool)> = counted
		.iter()
		.map(|&(row, visit)| (column[row], visit, is_pageview(row)))
		.collect();
	rows.sort_unstable();
	rows.chunk_by(|a, b| a.0 == b.0)
		.map(|group| {
			let mut metrics = Metrics {
				events: group.len() as u64,
				pageviews: group.iter().filter(|row| row.2).count() as u64,
				..Metrics::default()
			};
			let mut visitors = Vec::new();
			for same in group.chunk_by(|a, b| a.1 == b.1) {
				let visit = &visits[same[0].1 as usize];
				metrics.add_visit(visit);
				visitors.push(visit.visitor);
			}
			metrics.visitors = distinct(visitors);
			(group[0].0, metrics)
		})
		.collect()
}

fn distinct(mut ids: Vec<u32>) -> u64 {
	ids.sort_unstable();
	ids.dedup();
	ids.len() as u64
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
