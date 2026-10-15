pub mod goatcounter;
pub mod plausible;
pub(crate) mod referrer;
pub(crate) mod ua;

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, RwLock};

use crater::{Db, Dim, Event};
use jiff::Timestamp;
use jiff::tz::TimeZone;

use crate::app::{App, Site};
use crate::geo::Geo;
use crate::net::Proxies;
use crate::shields::Shields;

const SESSION_SECS: i64 = 30 * 60;
const SHARDS: usize = 64;
const CLICK_IDS: &[&str] = &["gclid", "gbraid", "wbraid", "msclkid", "fbclid", "twclid"];

pub struct Tracked {
	pub site: Site,
	tz: TimeZone,
	shields: Shields,
}

pub struct Hit {
	pub name: String,
	pub hostname: String,
	pub path: String,
	pub params: Vec<(String, String)>,
	pub referrer: String,
	pub props: Vec<(String, String)>,
	pub interactive: bool,
	pub scroll_depth: Option<u8>,
	pub engagement_ms: u32,
	pub revenue: Option<(u32, String)>,
	// past hits from the goatcounter api, live hits use now
	pub at: Option<Timestamp>,
}

pub struct Client<'a> {
	pub ip: IpAddr,
	pub ua: &'a str,
	pub country: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rejected {
	Bot,
	Blocked,
	NoSession,
	Busy,
	Failed,
}

struct Session {
	id: u64,
	last: i64,
}

type Shard = Mutex<HashMap<(u64, u64), Session>>;

pub struct Ingest {
	db: Arc<Db>,
	app: Arc<App>,
	geo: Arc<Geo>,
	pub proxies: Proxies,
	sites: RwLock<HashMap<String, Arc<Tracked>>>,
	salts: Mutex<HashMap<(u64, i32), [u8; 32]>>,
	sessions: Vec<Shard>,
}

impl Ingest {
	pub fn new(db: Arc<Db>, app: Arc<App>, geo: Arc<Geo>, proxies: Proxies) -> Self {
		Self {
			db,
			app,
			geo,
			proxies,
			sites: RwLock::default(),
			salts: Mutex::default(),
			sessions: (0..SHARDS).map(|_| Mutex::default()).collect(),
		}
	}

	pub fn site(&self, domain: &str) -> Option<Arc<Tracked>> {
		let domain = domain.strip_prefix("www.").unwrap_or(domain);
		let cached = self
			.sites
			.read()
			.unwrap_or_else(PoisonError::into_inner)
			.get(domain)
			.cloned();
		if cached.is_some() {
			return cached;
		}
		let site = self.app.site(domain).ok().flatten()?;
		Some(self.remember(site))
	}

	pub fn only_site(&self) -> Option<Arc<Tracked>> {
		match self.app.sites().ok()?.as_slice() {
			[site] => Some(self.remember(site.clone())),
			_ => None,
		}
	}

	pub fn forget(&self, domain: &str) {
		self.sites
			.write()
			.unwrap_or_else(PoisonError::into_inner)
			.remove(domain);
	}

	fn remember(&self, site: Site) -> Arc<Tracked> {
		let tz = TimeZone::get(&site.timezone).unwrap_or(TimeZone::UTC);
		let shields = Shields::load(&self.app, site.id);
		let tracked = Arc::new(Tracked { site, tz, shields });
		self.sites
			.write()
			.unwrap_or_else(PoisonError::into_inner)
			.insert(tracked.site.domain.clone(), tracked.clone());
		tracked
	}

	pub fn track(&self, site: &Tracked, hit: Hit, client: &Client) -> Result<(), Rejected> {
		if ua::is_bot(client.ua) {
			return Err(Rejected::Bot);
		}
		let now = hit.at.unwrap_or_else(Timestamp::now);
		let ts = now.as_second();
		let offset = site.tz.to_offset(now).seconds();
		let day = (ts + i64::from(offset)).div_euclid(86_400) as i32;
		let visitor = self.visitor(site, day, client)?;
		let previous = self.visitor(site, day - 1, client)?;
		let session = self.session(site.site.id, visitor, previous, ts, &hit.name)?;

		let param = |key: &str| {
			hit.params
				.iter()
				.find(|(name, _)| name == key)
				.map_or("", |(_, value)| value.as_str())
		};
		let tagged = [param("utm_source"), param("source"), param("ref")]
			.into_iter()
			.find(|value| !value.is_empty())
			.unwrap_or_default();
		let (referrer_host, referrer) = referrer_parts(&hit.referrer, &hit.hostname);
		let click_id = CLICK_IDS.iter().find(|&&id| !param(id).is_empty());
		let source = referrer::source(&referrer_host, tagged);
		let medium = match (param("utm_medium"), click_id, source.as_str()) {
			("", Some(&"gclid"), "Google") => "(gclid)",
			("", Some(&"msclkid"), "Bing") => "(msclkid)",
			(medium, _, _) => medium,
		};
		let channel = referrer::channel(&referrer::Traffic {
			referrer_host: &referrer_host,
			utm_source: tagged,
			utm_medium: medium,
			click_id: click_id.is_some(),
		});
		let agent = ua::parse(client.ua);
		let mut location = self.geo.locate(client.ip);
		if location.country.is_empty()
			&& let Some(country) = &client.country
		{
			location.country = country.clone();
		}
		if site
			.shields
			.blocks(client.ip, &location.country, &hit.path, &hit.hostname)
		{
			return Err(Rejected::Blocked);
		}

		let mut event = Event {
			site: site.site.id,
			ts,
			offset,
			visitor,
			session,
			interactive: hit.interactive,
			scroll_depth: hit.scroll_depth,
			engagement_ms: hit.engagement_ms,
			revenue: hit.revenue.clone(),
			..Event::default()
		};
		for (dim, value) in [
			(Dim::Name, hit.name.as_str()),
			(Dim::Hostname, hit.hostname.as_str()),
			(Dim::Page, hit.path.as_str()),
			(Dim::Referrer, referrer.as_str()),
			(Dim::Source, source.as_str()),
			(Dim::Channel, channel),
			(Dim::UtmSource, param("utm_source")),
			(Dim::UtmMedium, medium),
			(Dim::UtmCampaign, param("utm_campaign")),
			(Dim::UtmContent, param("utm_content")),
			(Dim::UtmTerm, param("utm_term")),
			(Dim::Country, location.country.as_str()),
			(Dim::Region, location.region.as_str()),
			(Dim::City, location.city.as_str()),
			(Dim::Device, agent.device),
			(Dim::Browser, agent.browser),
			(Dim::BrowserVersion, agent.browser_version.as_str()),
			(Dim::Os, agent.os),
			(Dim::OsVersion, agent.os_version.as_str()),
		] {
			event.dims[dim] = value.to_owned();
		}
		event.props = hit
			.props
			.into_iter()
			.filter(|(key, _)| site.shields.keeps(key))
			.collect();

		self.db.insert(Box::new(event)).map_err(|_| Rejected::Busy)
	}

	pub fn current_visitors(&self, site: u64) -> usize {
		let cutoff = Timestamp::now().as_second() - 5 * 60;
		self.sessions
			.iter()
			.map(|shard| {
				lock(shard)
					.iter()
					.filter(|((id, _), session)| *id == site && session.last >= cutoff)
					.count()
			})
			.sum()
	}

	pub fn sweep(&self) {
		let cutoff = Timestamp::now().as_second() - SESSION_SECS;
		for shard in &self.sessions {
			lock(shard).retain(|_, session| session.last >= cutoff);
		}
		let mut salts = lock(&self.salts);
		let newest = salts.keys().map(|&(_, day)| day).max().unwrap_or_default();
		salts.retain(|&(_, day), _| day >= newest - 2);
	}

	fn visitor(&self, site: &Tracked, day: i32, client: &Client) -> Result<u64, Rejected> {
		let salt = self.salt(site.site.id, day)?;
		let mut hasher = blake3::Hasher::new_keyed(&salt);
		hasher.update(client.ua.as_bytes());
		match client.ip {
			IpAddr::V4(ip) => hasher.update(&ip.octets()),
			IpAddr::V6(ip) => hasher.update(&ip.octets()),
		};
		hasher.update(site.site.domain.as_bytes());
		let hash = hasher.finalize();
		let mut id = [0; 8];
		id.copy_from_slice(&hash.as_bytes()[..8]);
		Ok(u64::from_le_bytes(id))
	}

	fn salt(&self, site: u64, day: i32) -> Result<[u8; 32], Rejected> {
		if let Some(salt) = lock(&self.salts).get(&(site, day)) {
			return Ok(*salt);
		}
		let salt = self.app.salt(site, day).map_err(|err| {
			tracing::error!(%err, site, day, "salt");
			Rejected::Failed
		})?;
		lock(&self.salts).insert((site, day), salt);
		Ok(salt)
	}

	fn session(
		&self,
		site: u64,
		visitor: u64,
		previous: u64,
		ts: i64,
		name: &str,
	) -> Result<u64, Rejected> {
		let shard = |visitor: u64| &self.sessions[(visitor % SHARDS as u64) as usize];
		let live = |visitor: u64| {
			lock(shard(visitor))
				.get(&(site, visitor))
				.filter(|session| ts - session.last <= SESSION_SECS)
				.map(|session| session.id)
		};
		let id = match live(visitor).or_else(|| live(previous)) {
			Some(id) => id,
			None if name == "engagement" => return Err(Rejected::NoSession),
			None => getrandom::u64().map_err(|_| Rejected::Failed)?,
		};
		lock(shard(visitor)).insert((site, visitor), Session { id, last: ts });
		Ok(id)
	}
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
	mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

fn referrer_parts(raw: &str, page_host: &str) -> (String, String) {
	let Ok(url) = url::Url::parse(raw) else {
		return Default::default();
	};
	let host = url.host_str().unwrap_or_default();
	let host = host.strip_prefix("www.").unwrap_or(host);
	let valid = matches!(url.scheme(), "http" | "https" | "android-app")
		&& !host.is_empty()
		&& host != page_host
		&& host != "localhost";
	if !valid {
		return Default::default();
	}
	let path = url.path().trim_end_matches('/');
	(host.to_owned(), format!("{host}{path}"))
}
