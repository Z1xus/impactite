use std::fs;
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::{Arc, PoisonError};
use std::time::Duration;

use anyhow::{Context, bail};
use aws_lc_rs::digest::{SHA256, digest};
use flate2::read::GzDecoder;
use serde::Deserialize;
use serde_json::json;

use crate::accounts::Web;

const REPO: &str = "z1xus/impactite";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

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

// asks github once a day, so admins see when a release is out
pub async fn watch(web: Arc<Web>) {
	loop {
		match latest(&web.http).await {
			Ok(release) => {
				let latest = match newer(&release.tag_name) {
					true => Some(json!({
						"version": release.tag_name.trim_start_matches('v'),
						"url": release.html_url,
						"changes": changes(&web.http, &release.tag_name).await.unwrap_or_default(),
					})),
					false => None,
				};
				*web.latest.lock().unwrap_or_else(PoisonError::into_inner) = latest;
			}
			Err(err) => tracing::debug!(%err, "update check"),
		}
		tokio::time::sleep(Duration::from_secs(86_400)).await;
	}
}

// swaps this binary for the latest release once its checksum matches
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
	let actual: String = digest(&SHA256, &packed)
		.as_ref()
		.iter()
		.map(|byte| format!("{byte:02x}"))
		.collect();
	if actual != expected {
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

// the impactite file inside the release's tar.gz
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
