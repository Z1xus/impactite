use std::sync::Arc;

use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;
use serde_json::Value;

use crate::auth::accounts::app_state;
use crate::web::Web;

#[derive(RustEmbed)]
#[folder = "web/build"]
#[exclude = "*.gz"]
#[allow_missing = true]
struct Assets;

pub async fn serve(State(web): State<Arc<Web>>, uri: Uri, headers: HeaderMap) -> Response {
	let requested = uri.path().trim_start_matches('/');
	if requested.starts_with("api/") {
		return StatusCode::NOT_FOUND.into_response();
	}
	let path = if Assets::get(requested).is_some() && !requested.is_empty() {
		requested
	} else {
		"index.html"
	};
	let Some(file) = Assets::get(path) else {
		return (StatusCode::NOT_FOUND, "dashboard not built").into_response();
	};

	if path == "index.html" {
		return page(&web, &headers, None);
	}

	let brotli = headers
		.get(header::ACCEPT_ENCODING)
		.and_then(|value| value.to_str().ok())
		.is_some_and(|value| value.contains("br"))
		.then(|| Assets::get(&format!("{path}.br")))
		.flatten();
	let cache = match path {
		_ if path.starts_with("_app/immutable/") => "public, max-age=31536000, immutable",
		"_app/version.json" => "no-cache",
		_ => "public, max-age=86400",
	};

	let mut response = match brotli {
		Some(compressed) => {
			let mut response = compressed.data.into_owned().into_response();
			response
				.headers_mut()
				.insert(header::CONTENT_ENCODING, HeaderValue::from_static("br"));
			response
		}
		None => file.data.into_owned().into_response(),
	};
	let headers = response.headers_mut();
	headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(kind(path)));
	headers.insert(header::CACHE_CONTROL, HeaderValue::from_static(cache));
	headers.insert(header::VARY, HeaderValue::from_static("accept-encoding"));
	headers.insert(
		header::X_CONTENT_TYPE_OPTIONS,
		HeaderValue::from_static("nosniff"),
	);
	headers.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
	headers.insert(
		header::REFERRER_POLICY,
		HeaderValue::from_static("same-origin"),
	);
	response
}

fn kind(path: &str) -> &'static str {
	match path.rsplit('.').next().unwrap_or_default() {
		"html" => "text/html; charset=utf-8",
		"js" => "text/javascript; charset=utf-8",
		"css" => "text/css; charset=utf-8",
		"json" | "webmanifest" => "application/json",
		"svg" => "image/svg+xml",
		"png" => "image/png",
		"ico" => "image/x-icon",
		"woff2" => "font/woff2",
		"woff" => "font/woff",
		"txt" => "text/plain; charset=utf-8",
		_ => "application/octet-stream",
	}
}

// the session rides along with the page, so the app needs no request to start
pub fn page(web: &Web, headers: &HeaderMap, share: Option<(&str, Option<Value>)>) -> Response {
	let Some(file) = Assets::get("index.html") else {
		return (StatusCode::NOT_FOUND, "dashboard not built").into_response();
	};
	let inline = |value: Value| value.to_string().replace('<', "\\u003c");
	let mut start = format!(
		"<script>window.early=Promise.resolve({})</script>",
		inline(app_state(web, headers))
	);
	let mut html = String::from_utf8_lossy(&file.data).into_owned();
	let framed = share.is_some();
	// other sites may frame a share link for embeds, and a public one carries preview tags
	if let Some((head, opened)) = share {
		if let Some(opened) = opened {
			start += &format!("<script>window.opened={}</script>", inline(opened));
		}
		html = html.replacen("</head>", &format!("{head}</head>"), 1);
	}
	let Some(nonce) = crate::auth::webauthn::challenge() else {
		return StatusCode::INTERNAL_SERVER_ERROR.into_response();
	};
	let html = html
		.replacen("<head>", &format!("<head>{start}"), 1)
		.replace("<script", &format!("<script nonce=\"{nonce}\""));
	let ancestors = if framed { "*" } else { "'none'" };
	let policy = format!(
		"default-src 'self'; script-src 'self' 'nonce-{nonce}'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self' data:; connect-src 'self'; object-src 'none'; base-uri 'self'; form-action 'self'; frame-ancestors {ancestors}"
	);
	let mut response = (
		[
			(header::CONTENT_TYPE, "text/html; charset=utf-8"),
			// no-transform keeps cloudflare rocket loader from rewriting nonced scripts
			(header::CACHE_CONTROL, "private, no-cache, no-transform"),
			(header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
			(header::REFERRER_POLICY, "same-origin"),
		],
		html,
	)
		.into_response();
	if let Ok(policy) = HeaderValue::from_str(&policy) {
		response
			.headers_mut()
			.insert(header::CONTENT_SECURITY_POLICY, policy);
	}
	if !framed {
		response
			.headers_mut()
			.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
	}
	response
}
