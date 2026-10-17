use std::fs;
use std::io::Read;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, PoisonError, RwLock};
use std::time::{Duration, SystemTime};

use anyhow::{Context, bail};
use flate2::read::GzDecoder;
use maxminddb::{PathElement, Reader};

const MAX_AGE: Duration = Duration::from_secs(35 * 86_400);

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Location {
	pub country: String,
	pub region: String,
	pub city: String,
}

#[derive(Default)]
pub struct Geo {
	reader: RwLock<Option<Arc<Reader<Vec<u8>>>>>,
}

impl Geo {
	pub fn locate(&self, ip: IpAddr) -> Location {
		let reader = self
			.reader
			.read()
			.unwrap_or_else(PoisonError::into_inner)
			.clone();
		let Some(reader) = reader else {
			return Location::default();
		};
		let Ok(found) = reader.lookup(ip) else {
			return Location::default();
		};
		let text = |path: &[PathElement]| {
			found
				.decode_path::<&str>(path)
				.ok()
				.flatten()
				.unwrap_or_default()
				.to_owned()
		};
		let country = text(&[PathElement::Key("country"), PathElement::Key("iso_code")]);
		if matches!(country.as_str(), "" | "ZZ" | "XX" | "T1") {
			return Location::default();
		}
		let region = text(&[
			PathElement::Key("subdivisions"),
			PathElement::Index(0),
			PathElement::Key("iso_code"),
		]);
		Location {
			region: if region.is_empty() {
				region
			} else {
				format!("{country}-{region}")
			},
			city: text(&[
				PathElement::Key("city"),
				PathElement::Key("names"),
				PathElement::Key("en"),
			]),
			country,
		}
	}

	pub fn load(&self, path: &Path) -> anyhow::Result<()> {
		let reader = Reader::from_source(fs::read(path)?).context("parse geo database")?;
		*self.reader.write().unwrap_or_else(PoisonError::into_inner) = Some(Arc::new(reader));
		Ok(())
	}

	pub async fn keep_fresh(&self, dir: PathBuf) {
		let path = dir.join("dbip-country-lite.mmdb");
		let _ = self.load(&path);
		loop {
			let stale =
				fs::metadata(&path)
					.and_then(|meta| meta.modified())
					.map_or(true, |modified| {
						SystemTime::now()
							.duration_since(modified)
							.is_ok_and(|age| age > MAX_AGE)
					});
			if stale {
				match download(&path).await.and_then(|()| self.load(&path)) {
					Ok(()) => tracing::info!("geo database updated"),
					Err(err) => tracing::warn!(%err, "geo database update failed"),
				}
			}
			tokio::time::sleep(Duration::from_secs(86_400)).await;
		}
	}
}

async fn download(path: &Path) -> anyhow::Result<()> {
	let now = jiff::Zoned::now().with_time_zone(jiff::tz::TimeZone::UTC);
	let months = [
		now.date(),
		now.date().saturating_sub(jiff::Span::new().months(1)),
	];
	let client = reqwest::Client::new();
	for month in months {
		let url = format!(
			"https://download.db-ip.com/free/dbip-country-lite-{}.mmdb.gz",
			month.strftime("%Y-%m")
		);
		let response = client.get(&url).send().await?;
		if !response.status().is_success() {
			continue;
		}
		let packed = response.bytes().await?;
		let mut mmdb = Vec::new();
		GzDecoder::new(packed.as_ref()).read_to_end(&mut mmdb)?;
		Reader::from_source(mmdb.as_slice()).context("downloaded geo database is invalid")?;
		if let Some(dir) = path.parent() {
			fs::create_dir_all(dir)?;
		}
		let tmp = path.with_extension("tmp");
		fs::write(&tmp, &mmdb)?;
		fs::rename(&tmp, path)?;
		return Ok(());
	}
	bail!("no geo database available")
}
