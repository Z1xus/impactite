use std::collections::{BTreeMap, HashMap};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::mem;
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::app::lock;
use crate::uptime::State;

const RAW_MAGIC: &[u8; 4] = b"IMUP";
const HOUR_MAGIC: &[u8; 4] = b"IMUH";
const VERSION: u8 = 1;
const HEAD: u64 = 5;
const CHECK_LEN: u64 = 8;
const HOUR_LEN: u64 = 46;
const BUCKETS: usize = 16;
const KEEP_DAYS: i64 = 90;
const OVER: u16 = u16::MAX;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Check {
	pub at: i64,
	// 0 means the check has no latency, like a heartbeat without a start ping
	pub ms: u32,
	pub state: State,
	pub code: u8,
}

impl Check {
	fn encode(self) -> [u8; 8] {
		let mut out = [0; 8];
		out[..4].copy_from_slice(&(self.at as u32).to_le_bytes());
		out[4..6].copy_from_slice(&(self.ms.min(u32::from(OVER)) as u16).to_le_bytes());
		out[6] = self.state as u8;
		out[7] = self.code;
		out
	}

	fn decode(bytes: &[u8; 8]) -> Self {
		Self {
			at: i64::from(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])),
			ms: u32::from(u16::from_le_bytes([bytes[4], bytes[5]])),
			state: State::from_u8(bytes[6]),
			code: bytes[7],
		}
	}
}

#[derive(Clone, Debug, PartialEq)]
pub struct Hour {
	pub hour: u32,
	pub checks: u16,
	pub failed: u16,
	pub slow: u16,
	pub buckets: [u16; BUCKETS],
	pub min: u16,
	pub max: u16,
}

impl Hour {
	fn new(hour: u32) -> Self {
		Self {
			hour,
			checks: 0,
			failed: 0,
			slow: 0,
			buckets: [0; BUCKETS],
			min: u16::MAX,
			max: 0,
		}
	}

	pub fn rolled(hour: u32, up: u16, down: u16, avg: u32, min: u32, max: u32) -> Self {
		let mut rolled = Self::new(hour);
		rolled.checks = up.saturating_add(down);
		rolled.failed = down;
		if up > 0 && avg > 0 {
			let (avg, min, max) = (
				avg.min(65_534) as u16,
				min.min(65_534) as u16,
				max.min(65_534) as u16,
			);
			rolled.buckets[(avg.ilog2() as usize).min(BUCKETS - 1)] = up;
			rolled.min = min.min(avg);
			rolled.max = max.max(avg);
		}
		rolled
	}

	fn add(&mut self, check: Check) {
		self.checks = self.checks.saturating_add(1);
		match check.state {
			State::Down => self.failed = self.failed.saturating_add(1),
			State::Slow => self.slow = self.slow.saturating_add(1),
			_ => {}
		}
		if check.ms > 0 {
			let ms = check.ms.min(u32::from(OVER)) as u16;
			let bucket = (ms.ilog2() as usize).min(BUCKETS - 1);
			self.buckets[bucket] = self.buckets[bucket].saturating_add(1);
			self.min = self.min.min(ms);
			self.max = self.max.max(ms);
		}
	}

	pub fn worst(&self) -> State {
		if self.failed > 0 {
			State::Down
		} else if self.slow > 0 {
			State::Slow
		} else {
			State::Up
		}
	}

	fn encode(&self) -> [u8; HOUR_LEN as usize] {
		let mut out = [0; HOUR_LEN as usize];
		out[..4].copy_from_slice(&self.hour.to_le_bytes());
		out[4..6].copy_from_slice(&self.checks.to_le_bytes());
		out[6..8].copy_from_slice(&self.failed.to_le_bytes());
		out[8..10].copy_from_slice(&self.slow.to_le_bytes());
		for (i, count) in self.buckets.iter().enumerate() {
			out[10 + i * 2..12 + i * 2].copy_from_slice(&count.to_le_bytes());
		}
		out[42..44].copy_from_slice(&self.min.to_le_bytes());
		out[44..46].copy_from_slice(&self.max.to_le_bytes());
		out
	}

	fn decode(bytes: &[u8; 46]) -> Self {
		let word = |at: usize| u16::from_le_bytes([bytes[at], bytes[at + 1]]);
		let mut buckets = [0; BUCKETS];
		for (i, slot) in buckets.iter_mut().enumerate() {
			*slot = word(10 + i * 2);
		}
		Self {
			hour: u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
			checks: word(4),
			failed: word(6),
			slow: word(8),
			buckets,
			min: word(42),
			max: word(44),
		}
	}
}

pub fn percentile(hours: &[Hour], q: f64) -> Option<u32> {
	let total: u64 = hours
		.iter()
		.flat_map(|hour| hour.buckets)
		.map(u64::from)
		.sum();
	if total == 0 {
		return None;
	}
	let rank = ((q * total as f64).ceil() as u64).clamp(1, total);
	let min = hours.iter().map(|hour| hour.min).min()?;
	let max = hours.iter().map(|hour| hour.max).max()?;
	let mut seen = 0;
	for bucket in 0..BUCKETS {
		let count: u64 = hours
			.iter()
			.map(|hour| u64::from(hour.buckets[bucket]))
			.sum();
		if seen + count >= rank {
			let fraction = (rank - seen) as f64 / count as f64;
			let value = f64::from(1u32 << bucket) * 2f64.powf(fraction);
			return Some((value as u32).clamp(u32::from(min), u32::from(max)));
		}
		seen += count;
	}
	None
}

#[derive(Default)]
struct Pending {
	raw: Vec<Check>,
	done: Vec<Hour>,
	current: Option<Hour>,
	dirty: bool,
}

pub struct Log {
	dir: PathBuf,
	pending: Mutex<HashMap<i64, Pending>>,
	// one writer at a time keeps records in order and a removed monitor removed
	writing: Mutex<()>,
}

impl Log {
	pub fn open(dir: PathBuf) -> io::Result<Self> {
		fs::create_dir_all(&dir)?;
		Ok(Self {
			dir,
			pending: Mutex::default(),
			writing: Mutex::default(),
		})
	}

	fn folder(&self, monitor: i64) -> PathBuf {
		self.dir.join(monitor.to_string())
	}

	pub fn push(&self, monitor: i64, check: Check) {
		let hour = (check.at / 3600) as u32;
		let mut all = lock(&self.pending);
		let pending = all.entry(monitor).or_default();
		pending.raw.push(check);
		if pending
			.current
			.as_ref()
			.is_none_or(|current| current.hour != hour)
		{
			let fresh = match pending.current.take() {
				Some(done) => {
					pending.done.push(done);
					Hour::new(hour)
				}
				None => self
					.stored_hour(monitor, hour)
					.unwrap_or_else(|| Hour::new(hour)),
			};
			pending.current = Some(fresh);
		}
		if let Some(current) = &mut pending.current {
			current.add(check);
		}
		pending.dirty = true;
	}

	// the hour a restart cut in half goes on counting where it stopped
	fn stored_hour(&self, monitor: i64, hour: u32) -> Option<Hour> {
		let mut file = File::open(self.folder(monitor).join("hours.log")).ok()?;
		let found = last_hour(&mut file).ok()??;
		(found.hour == hour).then_some(found)
	}

	pub fn flush(&self) -> io::Result<()> {
		let ids: Vec<i64> = lock(&self.pending).keys().copied().collect();
		let mut result = Ok(());
		for id in ids {
			if let Err(err) = self.flush_one(id) {
				result = result.and(Err(err));
			}
		}
		result
	}

	pub fn flush_one(&self, monitor: i64) -> io::Result<()> {
		let _writing = lock(&self.writing);
		let (raw, hours) = {
			let mut all = lock(&self.pending);
			let Some(pending) = all.get_mut(&monitor) else {
				return Ok(());
			};
			let mut hours = mem::take(&mut pending.done);
			if mem::take(&mut pending.dirty)
				&& let Some(current) = &pending.current
			{
				hours.push(current.clone());
			}
			(mem::take(&mut pending.raw), hours)
		};
		if raw.is_empty() && hours.is_empty() {
			return Ok(());
		}
		let dir = self.folder(monitor);
		fs::create_dir_all(&dir)?;
		let mut days: Vec<(i64, Vec<u8>)> = Vec::new();
		for check in raw {
			let day = check.at / 86_400;
			if days.last().is_none_or(|(last, _)| *last != day) {
				days.push((day, Vec::new()));
			}
			if let Some((_, bytes)) = days.last_mut() {
				bytes.extend_from_slice(&check.encode());
			}
		}
		for (day, bytes) in days {
			append(&dir.join(format!("{day}.log")), RAW_MAGIC, &bytes)?;
		}
		hours
			.iter()
			.try_for_each(|hour| store_hour(&dir.join("hours.log"), hour))
	}

	pub fn checks(&self, monitor: i64, from: i64, to: i64) -> io::Result<Vec<Check>> {
		self.flush_one(monitor)?;
		let mut out = Vec::new();
		for day in from / 86_400..=to / 86_400 {
			let path = self.folder(monitor).join(format!("{day}.log"));
			let Some(body) = read_body(&path, RAW_MAGIC)? else {
				continue;
			};
			out.extend(
				body.as_chunks::<8>()
					.0
					.iter()
					.map(Check::decode)
					.filter(|check| (from..=to).contains(&check.at)),
			);
		}
		Ok(out)
	}

	pub fn last(&self, monitor: i64, now: i64) -> Option<Check> {
		(0..2).find_map(|back| {
			let path = self
				.folder(monitor)
				.join(format!("{}.log", now / 86_400 - back));
			let body = read_body(&path, RAW_MAGIC).ok()??;
			body.as_chunks::<8>().0.last().map(Check::decode)
		})
	}

	pub fn hours(&self, monitor: i64, from: i64, to: i64) -> io::Result<Vec<Hour>> {
		self.flush_one(monitor)?;
		let mut file = match File::open(self.folder(monitor).join("hours.log")) {
			Ok(file) => file,
			Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
			Err(err) => return Err(err),
		};
		let records = records(&file, HOUR_LEN)?;
		let (first, last) = ((from / 3600).max(0) as u32, (to / 3600).max(0) as u32);
		let (mut low, mut high) = (0, records);
		while low < high {
			let middle = low + (high - low) / 2;
			if hour_at(&mut file, middle)?.hour < first {
				low = middle + 1;
			} else {
				high = middle;
			}
		}
		file.seek(SeekFrom::Start(HEAD + low * HOUR_LEN))?;
		let mut bytes = Vec::new();
		file.read_to_end(&mut bytes)?;
		Ok(bytes
			.as_chunks::<46>()
			.0
			.iter()
			.map(Hour::decode)
			.take_while(|hour| hour.hour <= last)
			.collect())
	}

	pub fn backfill(
		&self,
		monitor: i64,
		checks: &[Check],
		rolled: Vec<Hour>,
		now: i64,
	) -> io::Result<()> {
		let mut hours: BTreeMap<u32, Hour> =
			rolled.into_iter().map(|hour| (hour.hour, hour)).collect();
		let mut built: BTreeMap<u32, Hour> = BTreeMap::new();
		let oldest = now / 86_400 - KEEP_DAYS;
		let mut days: Vec<(i64, Vec<u8>)> = Vec::new();
		for check in checks {
			let hour = (check.at / 3600) as u32;
			built
				.entry(hour)
				.or_insert_with(|| Hour::new(hour))
				.add(*check);
			let day = check.at / 86_400;
			if day < oldest {
				continue;
			}
			if days.last().is_none_or(|(last, _)| *last != day) {
				days.push((day, Vec::new()));
			}
			if let Some((_, bytes)) = days.last_mut() {
				bytes.extend_from_slice(&check.encode());
			}
		}
		hours.extend(built);
		let dir = self.folder(monitor);
		fs::create_dir_all(&dir)?;
		for (day, bytes) in days {
			append(&dir.join(format!("{day}.log")), RAW_MAGIC, &bytes)?;
		}
		hours
			.values()
			.try_for_each(|hour| store_hour(&dir.join("hours.log"), hour))
	}

	pub fn remove(&self, monitor: i64) {
		let _writing = lock(&self.writing);
		lock(&self.pending).remove(&monitor);
		let _ = fs::remove_dir_all(self.folder(monitor));
	}

	pub fn prune(&self, now: i64) {
		let oldest = now / 86_400 - KEEP_DAYS;
		let Ok(dirs) = fs::read_dir(&self.dir) else {
			return;
		};
		for dir in dirs.flatten() {
			let Ok(files) = fs::read_dir(dir.path()) else {
				continue;
			};
			for file in files.flatten() {
				let day = file
					.file_name()
					.to_string_lossy()
					.strip_suffix(".log")
					.and_then(|name| name.parse::<i64>().ok());
				if day.is_some_and(|day| day < oldest) {
					let _ = fs::remove_file(file.path());
				}
			}
		}
	}
}

fn header(magic: &[u8; 4]) -> [u8; 5] {
	[magic[0], magic[1], magic[2], magic[3], VERSION]
}

fn open_checked(path: &Path, magic: &[u8; 4], width: u64) -> io::Result<File> {
	let mut file = OpenOptions::new()
		.read(true)
		.write(true)
		.create(true)
		.truncate(false)
		.open(path)?;
	let len = file.metadata()?.len();
	if len < HEAD {
		file.set_len(0)?;
		file.write_all(&header(magic))?;
	} else {
		let mut head = [0; HEAD as usize];
		file.read_exact_at(&mut head, 0)?;
		if head != header(magic) {
			return Err(io::Error::new(
				io::ErrorKind::InvalidData,
				"unknown log format",
			));
		}
		let whole = HEAD + (len - HEAD) / width * width;
		if whole != len {
			file.set_len(whole)?;
		}
	}
	Ok(file)
}

fn append(path: &Path, magic: &[u8; 4], bytes: &[u8]) -> io::Result<()> {
	let mut file = open_checked(path, magic, CHECK_LEN)?;
	file.seek(SeekFrom::End(0))?;
	file.write_all(bytes)
}

fn read_body(path: &Path, magic: &[u8; 4]) -> io::Result<Option<Vec<u8>>> {
	let mut bytes = match fs::read(path) {
		Ok(bytes) => bytes,
		Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
		Err(err) => return Err(err),
	};
	if bytes.len() < HEAD as usize || bytes[..HEAD as usize] != header(magic) {
		return Ok(None);
	}
	bytes.drain(..HEAD as usize);
	Ok(Some(bytes))
}

fn records(file: &File, width: u64) -> io::Result<u64> {
	Ok(file.metadata()?.len().saturating_sub(HEAD) / width)
}

fn hour_at(file: &mut File, index: u64) -> io::Result<Hour> {
	let mut bytes = [0; HOUR_LEN as usize];
	file.seek(SeekFrom::Start(HEAD + index * HOUR_LEN))?;
	file.read_exact(&mut bytes)?;
	Ok(Hour::decode(&bytes))
}

fn last_hour(file: &mut File) -> io::Result<Option<Hour>> {
	match records(file, HOUR_LEN)? {
		0 => Ok(None),
		count => hour_at(file, count - 1).map(Some),
	}
}

// the open hour is rewritten in place on every flush, a finished one is appended
fn store_hour(path: &Path, hour: &Hour) -> io::Result<()> {
	let mut file = open_checked(path, HOUR_MAGIC, HOUR_LEN)?;
	let count = records(&file, HOUR_LEN)?;
	let at = match last_hour(&mut file)? {
		Some(last) if last.hour == hour.hour => count - 1,
		Some(last) if last.hour > hour.hour => return Ok(()),
		_ => count,
	};
	file.seek(SeekFrom::Start(HEAD + at * HOUR_LEN))?;
	file.write_all(&hour.encode())
}

#[cfg(test)]
mod tests {
	use super::*;

	fn scratch(name: &str) -> PathBuf {
		let dir = std::env::temp_dir().join(format!("impactite-log-{name}-{}", std::process::id()));
		let _ = fs::remove_dir_all(&dir);
		dir
	}

	fn check(at: i64, ms: u32, state: State) -> Check {
		Check {
			at,
			ms,
			state,
			code: 2,
		}
	}

	#[test]
	fn torn_tail_is_cut_before_the_next_append() {
		let dir = scratch("torn");
		let log = Log::open(dir.clone()).unwrap();
		log.push(1, check(1_000_000, 40, State::Up));
		log.push(1, check(1_000_060, 50, State::Up));
		log.flush().unwrap();
		let path = dir.join("1").join(format!("{}.log", 1_000_000 / 86_400));
		let mut file = OpenOptions::new().append(true).open(&path).unwrap();
		file.write_all(&[1, 2, 3]).unwrap();
		drop(file);
		let log = Log::open(dir.clone()).unwrap();
		assert_eq!(log.checks(1, 0, 2_000_000).unwrap().len(), 2);
		log.push(1, check(1_000_120, 60, State::Down));
		log.flush().unwrap();
		let all = log.checks(1, 0, 2_000_000).unwrap();
		assert_eq!(all.len(), 3);
		assert_eq!(all[2], check(1_000_120, 60, State::Down));
		let _ = fs::remove_dir_all(dir);
	}

	#[test]
	fn open_hour_goes_on_after_a_restart() {
		let dir = scratch("hours");
		let log = Log::open(dir.clone()).unwrap();
		log.push(7, check(7_200, 100, State::Up));
		log.flush().unwrap();
		let log = Log::open(dir.clone()).unwrap();
		log.push(7, check(7_260, 200, State::Down));
		log.flush().unwrap();
		let hours = log.hours(7, 0, 10_000).unwrap();
		assert_eq!(hours.len(), 1);
		assert_eq!((hours[0].checks, hours[0].failed), (2, 1));
		let _ = fs::remove_dir_all(dir);
	}

	#[test]
	fn percentiles_stay_inside_what_was_seen() {
		let mut hour = Hour::new(0);
		for _ in 0..90 {
			hour.add(check(0, 100, State::Up));
		}
		for _ in 0..10 {
			hour.add(check(0, 3000, State::Slow));
		}
		let hours = [hour];
		assert_eq!(percentile(&hours, 0.5), Some(100));
		let p95 = percentile(&hours, 0.95).unwrap();
		assert!((2048..=3000).contains(&p95));
		assert_eq!(percentile(&[Hour::new(0)], 0.5), None);
	}
}
