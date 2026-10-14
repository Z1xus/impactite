use axum::http::{HeaderMap, HeaderValue, StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "web/build"]
#[exclude = "*.gz"]
#[allow_missing = true]
struct Assets;

pub async fn serve(uri: Uri, headers: HeaderMap) -> Response {
	let requested = uri.path().trim_start_matches('/');
	if requested.starts_with("api/") {
		return StatusCode::NOT_FOUND.into_response();
	}
	let tracker = requested.starts_with("js/script.") && requested.ends_with(".js");
	let path = match () {
		_ if tracker => "js/script.js",
		_ if Assets::get(requested).is_some() && !requested.is_empty() => requested,
		_ => "index.html",
	};
	let Some(file) = Assets::get(path) else {
		return (StatusCode::NOT_FOUND, "dashboard not built").into_response();
	};

	let brotli = headers
		.get(header::ACCEPT_ENCODING)
		.and_then(|value| value.to_str().ok())
		.is_some_and(|value| value.contains("br"))
		.then(|| Assets::get(&format!("{path}.br")))
		.flatten();
	let cache = match path {
		_ if path.starts_with("_app/immutable/") => "public, max-age=31536000, immutable",
		"js/script.js" => "public, max-age=3600",
		_ => "no-cache",
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
