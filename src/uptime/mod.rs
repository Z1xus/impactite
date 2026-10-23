mod api;
mod calc;
mod import;
pub mod log;
mod missed;
mod probe;
pub mod public;
pub mod push;
pub mod run;
mod status;
mod store;

use std::collections::{BTreeMap, HashMap};
use std::io;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, mpsc};

use crate::app::lock;
use crate::uptime::log::Log;
use crate::uptime::run::Msg;

pub use api::Fields;
pub use import::kuma;
pub use missed::visits_missed;

const DIGEST_SECS: u64 = 60;

pub fn routes() -> axum::Router<Arc<crate::web::Web>> {
	api::routes()
		.merge(status::routes())
		.merge(import::routes())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
#[repr(u8)]
pub enum State {
	Up = 0,
	Slow = 1,
	Down = 2,
	Paused = 3,
	Maintenance = 4,
	Pending = 5,
}

impl State {
	pub fn from_u8(n: u8) -> Self {
		match n {
			0 => Self::Up,
			1 => Self::Slow,
			2 => Self::Down,
			3 => Self::Paused,
			4 => Self::Maintenance,
			_ => Self::Pending,
		}
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
	Http,
	Tcp,
	Heartbeat,
}

impl Kind {
	fn as_str(self) -> &'static str {
		match self {
			Self::Http => "http",
			Self::Tcp => "tcp",
			Self::Heartbeat => "heartbeat",
		}
	}

	fn parse(name: &str) -> Option<Self> {
		match name {
			"http" => Some(Self::Http),
			"tcp" => Some(Self::Tcp),
			"heartbeat" => Some(Self::Heartbeat),
			_ => None,
		}
	}
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
	pub v: u8,
	pub method: String,
	pub status: String,
	pub contains: String,
	pub lacks: String,
	pub headers: BTreeMap<String, String>,
	pub timeout: u64,
	pub slow: u32,
	pub grace: i64,
}

impl Default for Config {
	fn default() -> Self {
		Self {
			v: 1,
			method: "GET".to_owned(),
			status: "200-399".to_owned(),
			contains: String::new(),
			lacks: String::new(),
			headers: BTreeMap::new(),
			timeout: 10,
			slow: 2000,
			grace: 60,
		}
	}
}

impl Config {
	fn ranges(spec: &str) -> Option<Vec<(u16, u16)>> {
		spec.split(',')
			.map(|part| {
				let (low, high) = part
					.trim()
					.split_once('-')
					.unwrap_or((part.trim(), part.trim()));
				let (low, high) = (low.trim().parse().ok()?, high.trim().parse().ok()?);
				((100..=599).contains(&low) && low <= high && high <= 599).then_some((low, high))
			})
			.collect()
	}

	pub fn accepts(&self, status: u16) -> bool {
		Self::ranges(&self.status).is_some_and(|ranges| {
			ranges
				.iter()
				.any(|(low, high)| (*low..=*high).contains(&status))
		})
	}

	pub fn check(&self) -> Result<(), &'static str> {
		if !matches!(self.method.as_str(), "GET" | "HEAD" | "POST") {
			return Err("Use GET, HEAD or POST.");
		}
		if Self::ranges(&self.status).is_none() {
			return Err("Use status codes like 200-399 or 200,404.");
		}
		if !(1..=60).contains(&self.timeout) {
			return Err("The timeout is 1 to 60 seconds.");
		}
		if !(100..=60_000).contains(&self.slow) {
			return Err("The slow limit is 100 to 60000 ms.");
		}
		if !(0..=86_400).contains(&self.grace) {
			return Err("The grace time is up to a day.");
		}
		if self.contains.len() > 200 || self.lacks.len() > 200 || self.headers.len() > 20 {
			return Err("That is too much to match.");
		}
		let headers = self.headers.iter().all(|(name, value)| {
			name.parse::<reqwest::header::HeaderName>().is_ok()
				&& value.parse::<reqwest::header::HeaderValue>().is_ok()
		});
		if headers {
			Ok(())
		} else {
			Err("A header is not valid.")
		}
	}
}

#[derive(Clone, Debug, PartialEq)]
pub struct Monitor {
	pub id: i64,
	pub team: i64,
	pub site: Option<i64>,
	pub name: String,
	pub kind: Kind,
	pub target: String,
	pub config: Config,
	pub interval: i64,
	pub paused: bool,
	pub muted: bool,
	pub token: Option<String>,
	pub created_by: Option<i64>,
	pub created_at: i64,
}

#[derive(Clone, Copy, Debug)]
pub struct Live {
	pub state: State,
	pub since: i64,
	pub latency: Option<u32>,
	pub at: i64,
	pub cert: Option<i64>,
}

#[derive(Clone, Copy)]
pub struct Event {
	pub team: i64,
	pub id: i64,
	pub live: Live,
}

#[derive(Clone)]
pub struct Digest {
	pub p50: Option<u32>,
	pub slow_days: [bool; 90],
	seen: bool,
}

impl Default for Digest {
	fn default() -> Self {
		Self {
			p50: None,
			slow_days: [false; 90],
			seen: false,
		}
	}
}

impl Digest {
	pub fn watched_from(&self, created: i64) -> i64 {
		if self.seen { created } else { i64::MAX }
	}
}

pub struct Uptime {
	pub log: Log,
	clients: Arc<probe::Clients>,
	tx: mpsc::UnboundedSender<Msg>,
	events: broadcast::Sender<Event>,
	live: Mutex<HashMap<i64, Live>>,
	digests: Mutex<HashMap<i64, (Instant, Digest)>>,
	missed: Mutex<HashMap<i64, Option<u32>>>,
}

impl Uptime {
	pub fn open(data: &Path) -> io::Result<(Arc<Self>, mpsc::UnboundedReceiver<Msg>)> {
		let (tx, rx) = mpsc::unbounded_channel();
		let uptime = Self {
			log: Log::open(data.join("uptime"))?,
			clients: Arc::new(probe::Clients::new().map_err(io::Error::other)?),
			tx,
			events: broadcast::channel(1024).0,
			live: Mutex::default(),
			digests: Mutex::default(),
			missed: Mutex::default(),
		};
		Ok((Arc::new(uptime), rx))
	}

	pub fn sync(&self) {
		let _ = self.tx.send(Msg::Sync);
	}

	fn send(&self, msg: Msg) {
		let _ = self.tx.send(msg);
	}

	pub fn subscribe(&self) -> broadcast::Receiver<Event> {
		self.events.subscribe()
	}

	fn live(&self, id: i64) -> Option<Live> {
		lock(&self.live).get(&id).copied()
	}

	fn publish(&self, team: i64, id: i64, live: Live) {
		lock(&self.live).insert(id, live);
		let _ = self.events.send(Event { team, id, live });
	}

	fn forget(&self, id: i64) {
		lock(&self.live).remove(&id);
		lock(&self.digests).remove(&id);
		self.log.remove(id);
	}

	fn digest(&self, id: i64, now: i64) -> Digest {
		if let Some((at, digest)) = lock(&self.digests).get(&id)
			&& at.elapsed().as_secs() < DIGEST_SECS
		{
			return digest.clone();
		}
		let hours = self
			.log
			.hours(id, now - 90 * 86_400, now)
			.unwrap_or_default();
		let day = now / 86_400;
		let mut slow_days = [false; 90];
		for hour in hours.iter().filter(|hour| hour.slow > 0) {
			let back = day - i64::from(hour.hour) / 24;
			if (0..90).contains(&back) {
				slow_days[89 - back as usize] = true;
			}
		}
		let seen = !hours.is_empty()
			|| self
				.live(id)
				.is_some_and(|live| live.state != State::Pending);
		let recent: Vec<_> = hours
			.into_iter()
			.filter(|hour| i64::from(hour.hour) * 3600 > now - 86_400)
			.collect();
		let digest = Digest {
			p50: log::percentile(&recent, 0.5),
			slow_days,
			seen,
		};
		if seen {
			lock(&self.digests).insert(id, (Instant::now(), digest.clone()));
		}
		digest
	}
}
