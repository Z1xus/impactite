use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::{self, ErrorKind};
use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, PoisonError, RwLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::cache::Cache;
use crate::dict::Dict;
use crate::event::Event;
use crate::query::{Field, Matcher, Query, Report, Slot};
use crate::rollup::{Counts, Metrics, Names};
use crate::segment::{Columns, Dense, Segment};
use crate::wal::Wal;

const QUEUE: usize = 1 << 16;
const CACHE: usize = 1 << 14;
const BATCH: usize = 1 << 14;
const TICK: Duration = Duration::from_secs(1);
const QUIET: Duration = Duration::from_secs(10);
const LATE: i64 = 2 * 3600;
const WAL_BYTES: u64 = 64 << 20;
const WAL_AGE: Duration = Duration::from_secs(3600);

type Key = (u64, i32);

enum Msg {
	Event(Box<Event>),
	Sync(SyncSender<io::Result<()>>),
	Stop(SyncSender<io::Result<()>>),
}

pub struct Db {
	tx: SyncSender<Msg>,
	shared: Arc<Shared>,
	lock: Mutex<Option<fs::File>>,
}

impl Db {
	pub fn open(dir: impl Into<PathBuf>) -> io::Result<Self> {
		let dir = dir.into();
		fs::create_dir_all(&dir)?;
		let lock = fs::File::create(dir.join("lock"))?;
		lock.try_lock().map_err(|_| {
			io::Error::new(ErrorKind::WouldBlock, "data dir is used by another process")
		})?;
		let shared = Arc::new(Shared {
			dir,
			dicts: Mutex::default(),
			live: RwLock::default(),
			cache: Mutex::new(Cache::new(CACHE)),
			ranges: Mutex::new(Cache::new(CACHE)),
			imports: Mutex::new(Cache::new(CACHE)),
			versions: Mutex::default(),
		});
		let writer = Writer::open(shared.clone())?;
		let (tx, rx) = mpsc::sync_channel(QUEUE);
		thread::Builder::new()
			.name("crater".into())
			.spawn(move || writer.run(&rx))?;
		Ok(Self {
			tx,
			shared,
			lock: Mutex::new(Some(lock)),
		})
	}

	pub fn insert(&self, event: Box<Event>) -> Result<(), Box<Event>> {
		self.tx
			.try_send(Msg::Event(event))
			.map_err(|err| match err {
				TrySendError::Full(Msg::Event(event))
				| TrySendError::Disconnected(Msg::Event(event)) => event,
				_ => Box::default(),
			})
	}

	pub fn sync(&self) -> io::Result<()> {
		let (tx, rx) = mpsc::sync_channel(1);
		self.tx.send(Msg::Sync(tx)).map_err(|_| stopped())?;
		rx.recv().map_err(|_| stopped())?
	}

	pub fn close(&self) -> io::Result<()> {
		let (tx, rx) = mpsc::sync_channel(1);
		self.tx.send(Msg::Stop(tx)).map_err(|_| stopped())?;
		let closed = rx.recv().map_err(|_| stopped())?;
		lock(&self.lock).take();
		closed
	}

	pub fn query(&self, query: &Query) -> io::Result<Report> {
		let site = query.site;
		let dict = self.shared.dict(site)?;
		let plan = {
			let dict = dict.read().unwrap_or_else(PoisonError::into_inner);
			Plan {
				shape: hash(&(&query.filters, &query.breakdown, query.imports)),
				imports: query.imports && query.filters.is_empty(),
				matchers: query
					.filters
					.iter()
					.map(|filter| Matcher::new(filter, &dict))
					.collect(),
				names: Names::new(&dict),
				slots: query
					.breakdown
					.iter()
					.map(|field| Slot::resolve(field, &dict))
					.collect(),
			}
		};

		let live: HashSet<i32> = {
			let live = self
				.shared
				.live
				.read()
				.unwrap_or_else(PoisonError::into_inner);
			query
				.days
				.clone()
				.filter(|&day| live.contains_key(&(site, day)))
				.collect()
		};
		let sealed: Vec<i32> = query
			.days
			.clone()
			.filter(|day| !live.contains(day))
			.collect();
		let merged = self.merged(site, &sealed, &plan)?;

		let mut total = merged.total;
		let mut hours = merged.hours;
		let mut days = Vec::new();
		let mut sealed_days = merged.days.iter();
		let mut live_rows: HashMap<u32, Metrics> = HashMap::new();
		for day in query.days.clone() {
			if !live.contains(&day) {
				days.push(sealed_days.next().copied().unwrap_or_default());
				continue;
			}
			let counts = self.day((site, day), &plan)?;
			total.merge(&counts.total);
			for (hour, other) in hours.iter_mut().zip(&counts.hours) {
				hour.merge(other);
			}
			days.push(counts.total);
			for &(id, metrics) in plan.rows(&counts) {
				live_rows.entry(id).or_default().merge(&metrics);
			}
			if plan.imports
				&& let Some(imported) = self.imported((site, day))?
			{
				total.merge(&imported.total);
				if let Some(last) = days.last_mut() {
					last.merge(&imported.total);
				}
				for &(id, metrics) in plan.rows(&imported) {
					live_rows.entry(id).or_default().merge(&metrics);
				}
			}
		}

		let mut candidates: Vec<u32> = merged
			.rows
			.iter()
			.take(query.limit)
			.map(|&(id, _)| id)
			.chain(live_rows.keys().copied())
			.collect();
		candidates.sort_unstable();
		candidates.dedup();
		let mut rows: Vec<(u32, Metrics)> = candidates
			.into_iter()
			.map(|id| {
				let mut metrics = merged
					.index
					.get(&id)
					.map(|&i| merged.rows[i].1)
					.unwrap_or_default();
				if let Some(live) = live_rows.get(&id) {
					metrics.merge(live);
				}
				(id, metrics)
			})
			.collect();
		sort_rows(&mut rows);
		rows.truncate(query.limit);

		let daily = match query.daily {
			true => query
				.days
				.clone()
				.map(|day| {
					let counts = self.day((site, day), &plan)?;
					Ok(plan.rows(&counts).take(query.limit).copied().collect())
				})
				.collect::<io::Result<Vec<Vec<(u32, Metrics)>>>>()?,
			false => Vec::new(),
		};

		let dict = dict.read().unwrap_or_else(PoisonError::into_inner);
		let resolve = |rows: Vec<(u32, Metrics)>| {
			rows.into_iter()
				.filter_map(|(id, metrics)| Some((dict.get(id)?.clone(), metrics)))
				.collect()
		};
		Ok(Report {
			total,
			days,
			hours,
			rows: resolve(rows),
			daily: daily.into_iter().map(resolve).collect(),
		})
	}

	pub fn import(
		&self,
		site: u64,
		day: i32,
		total: Metrics,
		rows: &[(Field, String, Metrics)],
	) -> io::Result<()> {
		let dict = self.shared.dict(site)?;
		let mut fields: HashMap<Slot, HashMap<u32, Metrics>> = HashMap::new();
		{
			let mut dict = dict.write().unwrap_or_else(PoisonError::into_inner);
			for (field, value, metrics) in rows {
				let slot = match field {
					Field::Prop(key) => Slot::Prop(dict.id(key)),
					field => Slot::resolve(field, &dict),
				};
				fields
					.entry(slot)
					.or_default()
					.entry(dict.id(value))
					.or_default()
					.merge(metrics);
			}
			dict.flush()?;
		}
		let counts = Counts {
			total,
			fields: fields
				.into_iter()
				.map(|(slot, rows)| {
					let mut rows: Vec<_> = rows.into_iter().collect();
					sort_rows(&mut rows);
					(slot, rows)
				})
				.collect(),
			..Counts::empty()
		};
		let key = (site, day);
		counts.write(&self.shared.path(key, "imp"))?;
		lock(&self.shared.imports).insert(key, Some(Arc::new(counts)));
		*lock(&self.shared.ranges) = Cache::new(CACHE);
		Ok(())
	}

	pub fn prop_keys(&self, site: u64, days: RangeInclusive<i32>) -> io::Result<Vec<Arc<str>>> {
		let mut counts: HashMap<u32, u64> = HashMap::new();
		for day in days {
			let live = self
				.shared
				.live
				.read()
				.unwrap_or_else(PoisonError::into_inner)
				.get(&(site, day))
				.map(|live| live.parts.clone());
			match live {
				Some(parts) => {
					for key in parts.iter().flat_map(|part| &part.keys) {
						*counts.entry(*key).or_default() += 1;
					}
				}
				None => {
					let rollup = self.rollup((site, day))?;
					let imported = self.imported((site, day))?;
					for (slot, rows) in rollup
						.fields
						.iter()
						.chain(imported.iter().flat_map(|i| &i.fields))
					{
						if let Slot::Prop(key) = slot {
							*counts.entry(*key).or_default() +=
								rows.iter().map(|row| row.1.events).sum::<u64>();
						}
					}
				}
			}
		}
		let mut keys: Vec<(u32, u64)> =
			counts.into_iter().filter(|&(_, count)| count > 0).collect();
		keys.sort_unstable_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
		let dict = self.shared.dict(site)?;
		let dict = dict.read().unwrap_or_else(PoisonError::into_inner);
		Ok(keys
			.into_iter()
			.filter_map(|(key, _)| dict.get(key).cloned())
			.collect())
	}

	fn imported(&self, key: Key) -> io::Result<Option<Arc<Counts>>> {
		if let Some(counts) = lock(&self.shared.imports).get(&key) {
			return Ok(counts);
		}
		let counts = match Counts::read(&self.shared.path(key, "imp")) {
			Ok(counts) => Some(Arc::new(counts)),
			Err(err) if err.kind() == ErrorKind::NotFound => None,
			Err(err) => return Err(err),
		};
		lock(&self.shared.imports).insert(key, counts.clone());
		Ok(counts)
	}

	fn merged(&self, site: u64, days: &[i32], plan: &Plan) -> io::Result<Arc<Merged>> {
		let id = hash(&(site, days, plan.shape));
		if let Some(merged) = lock(&self.shared.ranges).get(&id) {
			let versions = lock(&self.shared.versions);
			let fresh = merged
				.seqs
				.iter()
				.all(|(key, seq)| versions.get(key).is_none_or(|version| version == seq));
			if fresh {
				return Ok(merged);
			}
		}

		let mut merged = Merged {
			seqs: Vec::with_capacity(days.len()),
			total: Metrics::default(),
			hours: [Metrics::default(); 24],
			days: Vec::with_capacity(days.len()),
			rows: Vec::new(),
			index: HashMap::new(),
		};
		let mut rows: HashMap<u32, Metrics> = HashMap::new();
		for &day in days {
			let counts = self.day((site, day), plan)?;
			merged.seqs.push(((site, day), counts.seq));
			merged.total.merge(&counts.total);
			for (hour, other) in merged.hours.iter_mut().zip(&counts.hours) {
				hour.merge(other);
			}
			merged.days.push(counts.total);
			for &(id, metrics) in plan.rows(&counts) {
				rows.entry(id).or_default().merge(&metrics);
			}
			if plan.imports
				&& let Some(imported) = self.imported((site, day))?
			{
				merged.total.merge(&imported.total);
				if let Some(last) = merged.days.last_mut() {
					last.merge(&imported.total);
				}
				for &(id, metrics) in plan.rows(&imported) {
					rows.entry(id).or_default().merge(&metrics);
				}
			}
		}
		merged.rows = rows.into_iter().collect();
		sort_rows(&mut merged.rows);
		merged.index = merged
			.rows
			.iter()
			.enumerate()
			.map(|(i, &(id, _))| (id, i))
			.collect();

		let merged = Arc::new(merged);
		lock(&self.shared.ranges).insert(id, merged.clone());
		Ok(merged)
	}

	fn day(&self, key: Key, plan: &Plan) -> io::Result<Arc<Counts>> {
		let live = self
			.shared
			.live
			.read()
			.unwrap_or_else(PoisonError::into_inner)
			.get(&key)
			.cloned();
		if let Some(live) = live {
			let cached = lock(&self.shared.cache).get(&(key, plan.shape, true));
			if let Some(counts) = cached.filter(|counts| counts.seq == live.version) {
				return Ok(counts);
			}
			let parts: Vec<&Columns> = live.parts.iter().map(|part| &**part).collect();
			let mut counts = Counts::scan(
				&parts,
				live.visitors,
				live.sessions,
				&plan.names,
				&plan.matchers,
				&plan.slots,
			);
			counts.seq = live.version;
			let counts = Arc::new(counts);
			lock(&self.shared.cache).insert((key, plan.shape, true), counts.clone());
			return Ok(counts);
		}

		if plan.matchers.is_empty() {
			let rollup = self.rollup(key)?;
			if rollup.fields.is_empty()
				|| plan
					.slots
					.iter()
					.all(|slot| rollup.fields.contains_key(slot))
			{
				return Ok(rollup);
			}
		}

		let version = lock(&self.shared.versions).get(&key).copied();
		let cached = lock(&self.shared.cache).get(&(key, plan.shape, false));
		if let Some(counts) = cached.filter(|counts| version.is_none_or(|seq| seq == counts.seq)) {
			return Ok(counts);
		}
		let counts = Segment::read(&self.shared.path(key, "seg")).map(|seg| {
			let mut counts = Counts::scan(
				&[&seg.cols],
				seg.visitors.len(),
				seg.sessions.len(),
				&plan.names,
				&plan.matchers,
				&plan.slots,
			);
			counts.seq = seg.seq;
			counts
		});
		let counts = Arc::new(missing_as_empty(counts)?);
		lock(&self.shared.cache).insert((key, plan.shape, false), counts.clone());
		Ok(counts)
	}

	fn rollup(&self, key: Key) -> io::Result<Arc<Counts>> {
		let version = lock(&self.shared.versions).get(&key).copied();
		let cached = lock(&self.shared.cache).get(&(key, 0, false));
		if let Some(counts) = cached.filter(|counts| version.is_none_or(|seq| seq == counts.seq)) {
			return Ok(counts);
		}
		let path = self.shared.path(key, "rlp");
		let counts = match Counts::read(&path) {
			Err(err) if err.kind() == ErrorKind::InvalidData => {
				let seg = Segment::read(&self.shared.path(key, "seg"))?;
				let dict = self.shared.dict(key.0)?;
				let counts = rollup(&seg, &dict.read().unwrap_or_else(PoisonError::into_inner));
				counts.write(&path)?;
				Ok(counts)
			}
			counts => counts,
		};
		let counts = Arc::new(missing_as_empty(counts)?);
		lock(&self.shared.cache).insert((key, 0, false), counts.clone());
		Ok(counts)
	}
}

struct Plan {
	shape: u64,
	imports: bool,
	matchers: Vec<Matcher>,
	names: Names,
	slots: Vec<Slot>,
}

impl Plan {
	fn rows<'a>(&'a self, counts: &'a Counts) -> impl Iterator<Item = &'a (u32, Metrics)> {
		self.slots
			.iter()
			.filter_map(|slot| counts.fields.get(slot))
			.flatten()
	}
}

struct Merged {
	seqs: Vec<(Key, u64)>,
	total: Metrics,
	hours: [Metrics; 24],
	days: Vec<Metrics>,
	rows: Vec<(u32, Metrics)>,
	index: HashMap<u32, usize>,
}

fn hash(value: &impl Hash) -> u64 {
	let mut hasher = DefaultHasher::new();
	value.hash(&mut hasher);
	hasher.finish()
}

fn sort_rows(rows: &mut [(u32, Metrics)]) {
	rows.sort_unstable_by(|a, b| {
		b.1.visitors
			.cmp(&a.1.visitors)
			.then(b.1.pageviews.cmp(&a.1.pageviews))
			.then(a.0.cmp(&b.0))
	});
}

fn missing_as_empty(counts: io::Result<Counts>) -> io::Result<Counts> {
	match counts {
		Err(err) if err.kind() == ErrorKind::NotFound => Ok(Counts::empty()),
		counts => counts,
	}
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
	mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

fn stopped() -> io::Error {
	io::Error::other("crater writer stopped")
}

#[derive(Clone)]
struct LiveView {
	version: u64,
	parts: Vec<Arc<Columns>>,
	visitors: usize,
	sessions: usize,
}

struct Shared {
	dir: PathBuf,
	dicts: Mutex<HashMap<u64, Arc<RwLock<Dict>>>>,
	live: RwLock<HashMap<Key, LiveView>>,
	cache: Mutex<Cache<(Key, u64, bool), Arc<Counts>>>,
	ranges: Mutex<Cache<u64, Arc<Merged>>>,
	imports: Mutex<Cache<Key, Option<Arc<Counts>>>>,
	versions: Mutex<HashMap<Key, u64>>,
}

impl Shared {
	fn site_dir(&self, site: u64) -> PathBuf {
		self.dir.join("sites").join(site.to_string())
	}

	fn path(&self, (site, day): Key, ext: &str) -> PathBuf {
		self.site_dir(site).join(format!("{day}.{ext}"))
	}

	fn dict(&self, site: u64) -> io::Result<Arc<RwLock<Dict>>> {
		let mut dicts = lock(&self.dicts);
		if let Some(dict) = dicts.get(&site) {
			return Ok(dict.clone());
		}
		let dir = self.site_dir(site);
		fs::create_dir_all(&dir)?;
		let dict = Arc::new(RwLock::new(Dict::open(&dir.join("dict"))?));
		dicts.insert(site, dict.clone());
		Ok(dict)
	}

	fn publish(&self, key: Key, view: Option<LiveView>) {
		let mut live = self.live.write().unwrap_or_else(PoisonError::into_inner);
		match view {
			Some(view) => live.insert(key, view),
			None => live.remove(&key),
		};
	}
}

struct Live {
	dense: Dense,
	parts: Vec<Arc<Columns>>,
	pending: Vec<Event>,
	touched: Instant,
}

struct Writer {
	shared: Arc<Shared>,
	wal: Wal,
	seq: u64,
	opened: Instant,
	checked: Instant,
	wal_keys: BTreeMap<u64, HashSet<Key>>,
	memtable: HashMap<Key, Live>,
	offsets: HashMap<u64, i32>,
	sealed: HashMap<Key, u64>,
	published: u64,
}

impl Writer {
	fn open(shared: Arc<Shared>) -> io::Result<Self> {
		let wal_dir = shared.dir.join("wal");
		fs::create_dir_all(&wal_dir)?;
		let mut seqs: Vec<u64> = fs::read_dir(&wal_dir)?
			.filter_map(|entry| {
				let path = entry.ok()?.path();
				path.extension()?.eq("log").then_some(())?;
				path.file_stem()?.to_str()?.parse().ok()
			})
			.collect();
		seqs.sort_unstable();

		let seq = seqs.last().map_or(0, |last| last + 1);
		let (wal, _) = Wal::open(&wal_path(&wal_dir, seq))?;
		let mut writer = Self {
			shared,
			wal,
			seq,
			opened: Instant::now(),
			checked: Instant::now(),
			wal_keys: BTreeMap::from([(seq, HashSet::new())]),
			memtable: HashMap::new(),
			offsets: HashMap::new(),
			sealed: HashMap::new(),
			published: 0,
		};

		for seq in seqs {
			let (_, events) = Wal::open(&wal_path(&wal_dir, seq))?;
			let mut keys = HashSet::new();
			for event in events {
				let key = (event.site, event.day());
				keys.insert(key);
				if writer.sealed_seq(key)?.is_some_and(|sealed| sealed >= seq) {
					continue;
				}
				writer.stage(event)?;
			}
			for &key in &keys {
				writer.repair(key)?;
			}
			writer.wal_keys.insert(seq, keys);
		}

		writer.drop_sealed_wals()?;
		writer.tick(true)?;
		Ok(writer)
	}

	fn run(mut self, rx: &Receiver<Msg>) -> io::Result<()> {
		let mut batch = Vec::new();
		let mut replies = Vec::new();
		let mut stop = None;
		loop {
			let first = match rx.recv_timeout(TICK) {
				Ok(msg) => Some(msg),
				Err(RecvTimeoutError::Timeout) => None,
				Err(RecvTimeoutError::Disconnected) => return Ok(()),
			};
			for msg in first.into_iter().chain(rx.try_iter().take(BATCH)) {
				match msg {
					Msg::Event(event) => batch.push(*event),
					Msg::Sync(reply) => replies.push(reply),
					Msg::Stop(reply) => stop = Some(reply),
				}
			}

			let stop_requested = stop.is_some();
			let force = !replies.is_empty() || stop_requested;
			let result = self.write(&mut batch).and_then(|()| self.tick(force));
			for reply in replies.drain(..).chain(stop.take()) {
				let _ = reply.send(match &result {
					Ok(()) => Ok(()),
					Err(err) => Err(io::Error::new(err.kind(), err.to_string())),
				});
			}
			result?;
			if stop_requested {
				return Ok(());
			}
		}
	}

	fn write(&mut self, batch: &mut Vec<Event>) -> io::Result<()> {
		if batch.is_empty() {
			return Ok(());
		}
		self.wal.append(batch)?;
		for event in batch.drain(..) {
			let key = (event.site, event.day());
			self.wal_keys.entry(self.seq).or_default().insert(key);
			self.stage(event)?;
		}
		if self.wal.size >= WAL_BYTES || self.opened.elapsed() >= WAL_AGE {
			self.rotate()?;
		}
		Ok(())
	}

	fn stage(&mut self, event: Event) -> io::Result<()> {
		let key = (event.site, event.day());
		self.offsets.insert(event.site, event.offset);
		if !self.memtable.contains_key(&key) {
			let live = self.revive(key)?;
			self.memtable.insert(key, live);
		}
		if let Some(live) = self.memtable.get_mut(&key) {
			live.pending.push(event);
			live.touched = Instant::now();
		}
		Ok(())
	}

	fn revive(&mut self, key: Key) -> io::Result<Live> {
		let mut live = Live {
			dense: Dense::default(),
			parts: Vec::new(),
			pending: Vec::new(),
			touched: Instant::now(),
		};
		if self.sealed_seq(key)?.is_some() {
			let seg = Segment::read(&self.shared.path(key, "seg"))?;
			live.dense = Dense::from_lists(seg.visitors, seg.sessions);
			live.parts.push(Arc::new(seg.cols));
		}
		Ok(live)
	}

	fn tick(&mut self, force: bool) -> io::Result<()> {
		if !force && self.checked.elapsed() < TICK {
			return Ok(());
		}
		self.checked = Instant::now();

		let keys: Vec<Key> = self.memtable.keys().copied().collect();
		for &key in &keys {
			self.flush(key)?;
		}

		let now = SystemTime::now()
			.duration_since(UNIX_EPOCH)
			.map_or(0, |since| since.as_secs() as i64);
		let due: Vec<Key> = keys
			.into_iter()
			.filter(|key| {
				let offset = self.offsets.get(&key.0).copied().unwrap_or_default();
				let ended = now + i64::from(offset) >= (i64::from(key.1) + 1) * 86_400 + LATE;
				let quiet = self.memtable[key].touched.elapsed() >= QUIET;
				ended && (force || quiet)
			})
			.collect();
		if due.is_empty() {
			return Ok(());
		}
		for key in due {
			self.seal(key)?;
		}
		self.rotate()?;
		self.drop_sealed_wals()
	}

	fn flush(&mut self, key: Key) -> io::Result<()> {
		let Some(live) = self.memtable.get_mut(&key) else {
			return Ok(());
		};
		if live.pending.is_empty() {
			return Ok(());
		}
		let dict = self.shared.dict(key.0)?;
		let mut dict = dict.write().unwrap_or_else(PoisonError::into_inner);
		let mut cols = Columns::new();
		for event in live.pending.drain(..) {
			cols.push(&event, &mut live.dense, &mut dict);
		}
		live.parts.push(Arc::new(cols));
		self.published += 1;
		self.shared.publish(
			key,
			Some(LiveView {
				version: self.published,
				parts: live.parts.clone(),
				visitors: live.dense.visitors.len(),
				sessions: live.dense.sessions.len(),
			}),
		);
		Ok(())
	}

	fn seal(&mut self, key: Key) -> io::Result<()> {
		self.flush(key)?;
		let Some(live) = self.memtable.remove(&key) else {
			return Ok(());
		};
		let seg = Segment {
			seq: self.seq,
			cols: Columns::concat(live.parts.iter().map(|part| &**part)),
			visitors: live.dense.visitors,
			sessions: live.dense.sessions,
		};
		let dict = self.shared.dict(key.0)?;
		let mut dict = dict.write().unwrap_or_else(PoisonError::into_inner);
		dict.flush()?;
		seg.write(&self.shared.path(key, "seg"))?;
		rollup(&seg, &dict).write(&self.shared.path(key, "rlp"))?;
		lock(&self.shared.versions).insert(key, self.seq);
		self.shared.publish(key, None);
		self.sealed.insert(key, self.seq);
		Ok(())
	}

	fn repair(&mut self, key: Key) -> io::Result<()> {
		let Some(sealed) = self.sealed_seq(key)? else {
			return Ok(());
		};
		let path = self.shared.path(key, "rlp");
		if Counts::read(&path).is_ok_and(|counts| counts.seq == sealed) {
			return Ok(());
		}
		let seg = Segment::read(&self.shared.path(key, "seg"))?;
		let dict = self.shared.dict(key.0)?;
		let dict = dict.read().unwrap_or_else(PoisonError::into_inner);
		rollup(&seg, &dict).write(&path)?;
		lock(&self.shared.versions).insert(key, sealed);
		Ok(())
	}

	fn sealed_seq(&mut self, key: Key) -> io::Result<Option<u64>> {
		if let Some(&seq) = self.sealed.get(&key) {
			return Ok(Some(seq));
		}
		match Segment::read_seq(&self.shared.path(key, "seg")) {
			Ok(seq) => {
				self.sealed.insert(key, seq);
				Ok(Some(seq))
			}
			Err(err) if err.kind() == ErrorKind::NotFound => Ok(None),
			Err(err) => Err(err),
		}
	}

	fn rotate(&mut self) -> io::Result<()> {
		let seq = self.seq + 1;
		let (wal, _) = Wal::open(&wal_path(&self.shared.dir.join("wal"), seq))?;
		self.wal = wal;
		self.seq = seq;
		self.opened = Instant::now();
		self.wal_keys.insert(seq, HashSet::new());
		Ok(())
	}

	fn drop_sealed_wals(&mut self) -> io::Result<()> {
		let old: Vec<u64> = self
			.wal_keys
			.range(..self.seq)
			.map(|(&seq, _)| seq)
			.collect();
		for seq in old {
			let keys: Vec<Key> = self.wal_keys[&seq].iter().copied().collect();
			let mut done = true;
			for key in keys {
				done &= self.sealed_seq(key)?.is_some_and(|sealed| sealed >= seq);
			}
			if done {
				match fs::remove_file(wal_path(&self.shared.dir.join("wal"), seq)) {
					Err(err) if err.kind() != ErrorKind::NotFound => return Err(err),
					_ => {}
				}
				self.wal_keys.remove(&seq);
			}
		}
		Ok(())
	}
}

fn rollup(seg: &Segment, dict: &Dict) -> Counts {
	let mut counts = Counts::scan(
		&[&seg.cols],
		seg.visitors.len(),
		seg.sessions.len(),
		&Names::new(dict),
		&[],
		&Counts::rollup_slots(&[&seg.cols]),
	);
	counts.seq = seg.seq;
	counts
}

fn wal_path(dir: &Path, seq: u64) -> PathBuf {
	dir.join(format!("{seq:020}.log"))
}
