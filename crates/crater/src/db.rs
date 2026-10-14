use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender};
use std::sync::{Arc, Mutex, PoisonError, RwLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::dict::Dict;
use crate::event::Event;
use crate::rollup::{Counts, Rollup};
use crate::segment::Segment;
use crate::wal::Wal;

const QUEUE: usize = 1 << 16;
const BATCH: usize = 1 << 14;
const TICK: Duration = Duration::from_secs(1);
const LATE: i64 = 2 * 3600;
const QUIET: Duration = Duration::from_secs(10);
const WAL_BYTES: u64 = 64 << 20;
const WAL_AGE: Duration = Duration::from_secs(3600);

type Key = (u64, i32);

enum Msg {
	Event(Box<Event>),
	Sync(SyncSender<io::Result<()>>),
}

pub struct Db {
	tx: SyncSender<Msg>,
	shared: Arc<Shared>,
	writer: JoinHandle<io::Result<()>>,
}

impl Db {
	pub fn open(dir: impl Into<PathBuf>) -> io::Result<Self> {
		let shared = Arc::new(Shared {
			dir: dir.into(),
			dicts: Mutex::default(),
		});
		let writer = Writer::open(shared.clone())?;
		let (tx, rx) = mpsc::sync_channel(QUEUE);
		let writer = thread::Builder::new()
			.name("crater".into())
			.spawn(move || writer.run(&rx))?;
		Ok(Self { tx, shared, writer })
	}

	pub fn insert(&self, event: Event) -> bool {
		self.tx.try_send(Msg::Event(Box::new(event))).is_ok()
	}

	pub fn sync(&self) -> io::Result<()> {
		let (tx, rx) = mpsc::sync_channel(1);
		self.tx.send(Msg::Sync(tx)).map_err(|_| stopped())?;
		rx.recv().map_err(|_| stopped())?
	}

	pub fn rollup(&self, site: u64, day: i32) -> io::Result<Option<Rollup>> {
		let counts = match Counts::read(&self.shared.path((site, day), "rlp")) {
			Ok(counts) => counts,
			Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
			Err(err) => return Err(err),
		};
		let dict = self.shared.dict(site)?;
		let dict = dict.read().unwrap_or_else(PoisonError::into_inner);
		counts.resolve(&dict).map(Some)
	}

	pub fn close(self) -> io::Result<()> {
		let Self { tx, writer, .. } = self;
		drop(tx);
		writer.join().map_err(|_| stopped())?
	}
}

fn stopped() -> io::Error {
	io::Error::other("crater writer stopped")
}

struct Shared {
	dir: PathBuf,
	dicts: Mutex<HashMap<u64, Arc<RwLock<Dict>>>>,
}

impl Shared {
	fn site_dir(&self, site: u64) -> PathBuf {
		self.dir.join("sites").join(site.to_string())
	}

	fn path(&self, (site, day): Key, ext: &str) -> PathBuf {
		self.site_dir(site).join(format!("{day}.{ext}"))
	}

	fn dict(&self, site: u64) -> io::Result<Arc<RwLock<Dict>>> {
		let mut dicts = self.dicts.lock().unwrap_or_else(PoisonError::into_inner);
		if let Some(dict) = dicts.get(&site) {
			return Ok(dict.clone());
		}
		let dir = self.site_dir(site);
		fs::create_dir_all(&dir)?;
		let dict = Arc::new(RwLock::new(Dict::open(&dir.join("dict"))?));
		dicts.insert(site, dict.clone());
		Ok(dict)
	}
}

struct Writer {
	shared: Arc<Shared>,
	wal: Wal,
	seq: u64,
	opened: Instant,
	checked: Instant,
	wal_keys: BTreeMap<u64, HashSet<Key>>,
	memtable: HashMap<Key, Vec<Event>>,
	touched: HashMap<Key, Instant>,
	offsets: HashMap<u64, i32>,
	sealed: HashMap<Key, u64>,
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
			touched: HashMap::new(),
			offsets: HashMap::new(),
			sealed: HashMap::new(),
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
				writer.offsets.insert(event.site, event.offset);
				writer.touched.insert(key, Instant::now());
				writer.memtable.entry(key).or_default().push(event);
			}
			for &key in &keys {
				writer.repair(key)?;
			}
			writer.wal_keys.insert(seq, keys);
		}

		writer.drop_sealed_wals()?;
		Ok(writer)
	}

	fn run(mut self, rx: &Receiver<Msg>) -> io::Result<()> {
		let mut batch = Vec::new();
		let mut replies = Vec::new();
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
				}
			}

			let force = !replies.is_empty();
			let result = self.write(&mut batch).and_then(|()| self.seal_due(force));
			for reply in replies.drain(..) {
				let _ = reply.send(match &result {
					Ok(()) => Ok(()),
					Err(err) => Err(io::Error::new(err.kind(), err.to_string())),
				});
			}
			result?;
		}
	}

	fn write(&mut self, batch: &mut Vec<Event>) -> io::Result<()> {
		if batch.is_empty() {
			return Ok(());
		}
		self.wal.append(batch)?;
		let keys = self.wal_keys.entry(self.seq).or_default();
		for event in batch.drain(..) {
			let key = (event.site, event.day());
			keys.insert(key);
			self.offsets.insert(event.site, event.offset);
			self.touched.insert(key, Instant::now());
			self.memtable.entry(key).or_default().push(event);
		}
		if self.wal.size >= WAL_BYTES || self.opened.elapsed() >= WAL_AGE {
			self.rotate()?;
		}
		Ok(())
	}

	fn seal_due(&mut self, force: bool) -> io::Result<()> {
		if !force && self.checked.elapsed() < TICK {
			return Ok(());
		}
		self.checked = Instant::now();
		let now = SystemTime::now()
			.duration_since(UNIX_EPOCH)
			.map_or(0, |since| since.as_secs() as i64);
		let due: Vec<Key> = self
			.memtable
			.keys()
			.copied()
			.filter(|&(site, day)| {
				let offset = self.offsets.get(&site).copied().unwrap_or_default();
				let quiet = self
					.touched
					.get(&(site, day))
					.is_none_or(|touched| touched.elapsed() >= QUIET);
				(force || quiet) && now + i64::from(offset) >= (i64::from(day) + 1) * 86_400 + LATE
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

	fn seal(&mut self, key: Key) -> io::Result<()> {
		let Some(mut events) = self.memtable.remove(&key) else {
			return Ok(());
		};
		self.touched.remove(&key);
		let (site, day) = key;
		let path = self.shared.path(key, "seg");
		let merge = self.sealed_seq(key)?.is_some();
		let dict = self.shared.dict(site)?;
		let mut dict = dict.write().unwrap_or_else(PoisonError::into_inner);
		if merge {
			events.extend(Segment::read(&path)?.events(&dict, site, day)?);
		}
		let seg = Segment::build(events, &mut dict, self.seq);
		dict.flush()?;
		seg.write(&path)?;
		Counts::compute(&seg, &dict).write(&self.shared.path(key, "rlp"))?;
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
		Counts::compute(&seg, &dict).write(&path)
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

fn wal_path(dir: &Path, seq: u64) -> PathBuf {
	dir.join(format!("{seq:020}.log"))
}
