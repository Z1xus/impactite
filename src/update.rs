use std::fs;
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, bail};
use aws_lc_rs::digest::{SHA256, digest};
use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::app::{hex, lock};
use crate::web::Web;

const REPO: &str = "z1xus/impactite";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Serialize)]
pub struct Status {
	pub enabled: bool,
	pub checking: bool,
	pub checked_at: Option<i64>,
	pub failed: bool,
	pub latest: Option<serde_json::Value>,
	#[serde(skip)]
	attempted: Option<Instant>,
}

impl Status {
	pub fn new(enabled: bool) -> Self {
		Self {
			enabled,
			checking: false,
			checked_at: None,
			failed: false,
			latest: None,
			attempted: None,
		}
	}
}

pub enum CheckError {
	Disabled,
	Busy,
	Recent,
}

#[derive(Deserialize)]
struct Release {
	tag_name: String,
	html_url: String,
}

async fn latest(http: &reqwest::Client) -> anyhow::Result<Release> {
	Ok(http
		.get(format!(
			"https://api.github.com/repos/{REPO}/releases/latest"
		))
		.header("user-agent", "impactite")
		.timeout(Duration::from_secs(10))
		.send()
		.await?
		.error_for_status()?
		.json()
		.await?)
}

fn parts(version: &str) -> Vec<u64> {
	version
		.trim_start_matches('v')
		.split(['.', '-'])
		.map_while(|part| part.parse().ok())
		.collect()
}

fn newer(tag: &str) -> bool {
	parts(tag) > parts(VERSION)
}

// the changelog sections after this version, up to the release
async fn changes(http: &reqwest::Client, tag: &str) -> anyhow::Result<serde_json::Value> {
	let text = http
		.get(format!(
			"https://raw.githubusercontent.com/{REPO}/{tag}/CHANGELOG.md"
		))
		.timeout(Duration::from_secs(10))
		.send()
		.await?
		.error_for_status()?
		.text()
		.await?;
	let mut sections: Vec<(&str, Vec<&str>)> = Vec::new();
	let mut shown = false;
	for line in text.lines() {
		if let Some(heading) = line.strip_prefix("## ") {
			let version = heading.trim();
			shown = parts(version) > parts(VERSION) && parts(version) <= parts(tag);
			if shown {
				sections.push((version, Vec::new()));
			}
		} else if let (Some(item), true) = (line.strip_prefix("- "), shown)
			&& let Some((_, lines)) = sections.last_mut()
		{
			lines.push(item.trim());
		}
	}
	Ok(sections
		.into_iter()
		.map(|(version, lines)| json!({ "version": version, "lines": lines }))
		.collect())
}

pub async fn check(web: &Web) -> Result<(), CheckError> {
	{
		let mut status = lock(&web.updates);
		if !status.enabled {
			return Err(CheckError::Disabled);
		}
		if status.checking {
			return Err(CheckError::Busy);
		}
		if status
			.attempted
			.is_some_and(|at| at.elapsed() < Duration::from_secs(60))
		{
			return Err(CheckError::Recent);
		}
		status.checking = true;
		status.attempted = Some(Instant::now());
	}
	match latest(&web.http).await {
		Ok(release) => {
			let latest = if newer(&release.tag_name) {
				Some(json!({
					"version": release.tag_name.trim_start_matches('v'),
					"url": release.html_url,
					"changes": changes(&web.http, &release.tag_name).await.unwrap_or_else(|_| json!([])),
				}))
			} else {
				None
			};
			let mut status = lock(&web.updates);
			status.latest = latest;
			status.checked_at = Some(crate::app::now());
			status.failed = false;
			status.checking = false;
		}
		Err(err) => {
			tracing::debug!(%err, "update check");
			let mut status = lock(&web.updates);
			status.failed = true;
			status.checking = false;
		}
	}
	Ok(())
}

pub async fn watch(web: Arc<Web>) {
	loop {
		let _ = check(&web).await;
		tokio::time::sleep(Duration::from_secs(3600)).await;
	}
}

pub async fn run() -> anyhow::Result<()> {
	if Path::new("/.dockerenv").exists() || Path::new("/run/.containerenv").exists() {
		bail!("impactite runs in a container here. Pull ghcr.io/{REPO}:latest and restart it");
	}
	let http = reqwest::Client::new();
	let release = latest(&http).await.context("check the latest release")?;
	if !newer(&release.tag_name) {
		println!("impactite {VERSION} is the latest version");
		return Ok(());
	}
	let archive = format!(
		"impactite-{}-unknown-linux-musl.tar.gz",
		std::env::consts::ARCH
	);
	let base = format!(
		"https://github.com/{REPO}/releases/download/{}/{archive}",
		release.tag_name
	);
	let get = |url: String| {
		let http = http.clone();
		async move { http.get(url).send().await?.error_for_status() }
	};
	let packed = get(base.clone()).await?.bytes().await?;
	let sums = get(format!("{base}.sha256")).await?.text().await?;
	let expected = sums.split_whitespace().next().context("empty checksum")?;
	if hex(digest(&SHA256, &packed).as_ref()) != expected {
		bail!("the download doesn't match its checksum. Nothing changed");
	}

	let current = std::env::current_exe()?;
	let fresh = current.with_extension("new");
	fs::write(&fresh, binary(&packed)?)?;
	fs::set_permissions(&fresh, fs::Permissions::from_mode(0o755))?;
	fs::rename(&fresh, &current).context("replace the binary")?;
	println!(
		"updated to {}. restart impactite to run it, and run setcap again if you used it",
		release.tag_name.trim_start_matches('v')
	);
	Ok(())
}

fn binary(packed: &[u8]) -> anyhow::Result<Vec<u8>> {
	let mut tar = Vec::new();
	GzDecoder::new(packed).read_to_end(&mut tar)?;
	let text = |field: &[u8]| {
		String::from_utf8_lossy(field)
			.trim_matches(['\0', ' '])
			.to_owned()
	};
	let mut at = 0;
	while let Some(header) = tar.get(at..at + 512) {
		let name = text(&header[..100]);
		let size = usize::from_str_radix(&text(&header[124..136]), 8).unwrap_or(0);
		let body = at + 512;
		if name == "impactite" || name.ends_with("/impactite") {
			return Ok(tar
				.get(body..body + size)
				.context("truncated archive")?
				.to_vec());
		}
		at = body + size.div_ceil(512) * 512;
	}
	bail!("the release has no impactite binary")
}
