use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Path, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};

use crate::site::shares::Reader;
use crate::web::Web;

const MAX_BYTES: usize = 256 * 1024;

pub async fn icon(
	State(web): State<Arc<Web>>,
	reader: Reader,
	Path(host): Path<String>,
) -> Response {
	if !reader.anyone(&web) {
		return StatusCode::UNAUTHORIZED.into_response();
	}
	let host = host.to_ascii_lowercase();
	let valid = host.len() <= 253
		&& host.contains('.')
		&& host
			.bytes()
			.all(|byte| byte.is_ascii_alphanumeric() || byte == b'.' || byte == b'-');
	if !valid {
		return StatusCode::NOT_FOUND.into_response();
	}

	let dir = web.data.join("icons");
	let path = dir.join(&host);
	let bytes = match tokio::fs::read(&path).await {
		// a host without an icon is asked again after a day
		Ok(bytes) if !bytes.is_empty() || recent(&path).await => bytes,
		_ => {
			let fetched = fetch(&web.http, &host).await.unwrap_or_default();
			let _ = tokio::fs::create_dir_all(&dir).await;
			let _ = tokio::fs::write(&path, &fetched).await;
			fetched
		}
	};

	let Some(kind) = kind(&bytes) else {
		return (
			StatusCode::NOT_FOUND,
			[(header::CACHE_CONTROL, "private, max-age=86400")],
		)
			.into_response();
	};
	(
		[
			(header::CONTENT_TYPE, kind),
			(header::CACHE_CONTROL, "private, max-age=604800"),
			(header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
		],
		bytes,
	)
		.into_response()
}

async fn recent(path: &std::path::Path) -> bool {
	let modified = tokio::fs::metadata(path)
		.await
		.and_then(|meta| meta.modified());
	modified.is_ok_and(|at| {
		at.elapsed()
			.is_ok_and(|age| age < Duration::from_secs(86_400))
	})
}

async fn fetch(http: &reqwest::Client, host: &str) -> Option<Vec<u8>> {
	let response = http
		.get(format!("https://icons.duckduckgo.com/ip3/{host}.ico"))
		.timeout(Duration::from_secs(4))
		.send()
		.await
		.ok()?;
	if !response.status().is_success() {
		return None;
	}
	let bytes = response.bytes().await.ok()?;
	(bytes.len() <= MAX_BYTES).then(|| bytes.to_vec())
}

fn kind(bytes: &[u8]) -> Option<&'static str> {
	match bytes {
		[0x89, b'P', b'N', b'G', ..] => Some("image/png"),
		[0, 0, 1, 0, ..] => Some("image/x-icon"),
		[b'G', b'I', b'F', ..] => Some("image/gif"),
		[0xff, 0xd8, ..] => Some("image/jpeg"),
		_ if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") => Some("image/webp"),
		_ => None,
	}
}
