use std::collections::HashMap;
use std::io::Write;
use std::sync::{Arc, LazyLock, Mutex};

use axum::extract::Path;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;

use crate::app::lock;

#[derive(RustEmbed)]
#[folder = "tracker/dist"]
#[allow_missing = true]
struct Modules;

const MODULES: [&str; 10] = [
	"hash",
	"manual",
	"local",
	"outbound-links",
	"file-downloads",
	"tagged-events",
	"form-submissions",
	"exclusions",
	"pageview-props",
	"titles",
];

type Built = Arc<(Vec<u8>, Vec<u8>)>;

static BUILT: LazyLock<Mutex<HashMap<Vec<&str>, Built>>> = LazyLock::new(Mutex::default);

pub async fn script(Path(file): Path<String>, headers: HeaderMap) -> Response {
	let Some(flags) = file
		.strip_prefix("script")
		.and_then(|rest| rest.strip_suffix(".js"))
	else {
		return StatusCode::NOT_FOUND.into_response();
	};
	let modules: Vec<&str> = MODULES
		.into_iter()
		.filter(|module| flags.split('.').any(|flag| flag == *module))
		.collect();
	let Some(built) = build(modules) else {
		return (StatusCode::NOT_FOUND, "tracker not built").into_response();
	};

	javascript(&built, &headers)
}

pub async fn count(headers: HeaderMap) -> Response {
	static COUNT: LazyLock<Option<Built>> =
		LazyLock::new(|| compress(Modules::get("count.js")?.data.into_owned()));
	match &*COUNT {
		Some(built) => javascript(built, &headers),
		None => (StatusCode::NOT_FOUND, "tracker not built").into_response(),
	}
}

fn javascript(built: &Built, headers: &HeaderMap) -> Response {
	let brotli = headers
		.get(header::ACCEPT_ENCODING)
		.and_then(|value| value.to_str().ok())
		.is_some_and(|value| value.contains("br"));
	let mut response = match brotli {
		true => built.1.clone().into_response(),
		false => built.0.clone().into_response(),
	};
	let headers = response.headers_mut();
	if brotli {
		headers.insert(header::CONTENT_ENCODING, HeaderValue::from_static("br"));
	}
	for (name, value) in [
		(header::CONTENT_TYPE, "text/javascript; charset=utf-8"),
		(header::CACHE_CONTROL, "public, max-age=3600"),
		(header::VARY, "accept-encoding"),
		(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*"),
	] {
		headers.insert(name, HeaderValue::from_static(value));
	}
	response
}

fn build(modules: Vec<&'static str>) -> Option<Built> {
	let mut cache = lock(&BUILT);
	if let Some(built) = cache.get(&modules) {
		return Some(built.clone());
	}
	let mut js = b"(()=>{const m=[];".to_vec();
	for module in &modules {
		js.extend_from_slice(&Modules::get(&format!("{module}.js"))?.data);
	}
	js.extend_from_slice(&Modules::get("core.js")?.data);
	js.extend_from_slice(b"})()");
	let built = compress(js)?;
	cache.insert(modules, built.clone());
	Some(built)
}

fn compress(js: Vec<u8>) -> Option<Built> {
	let mut compressed = Vec::new();
	{
		let mut writer = brotli::CompressorWriter::new(&mut compressed, 4096, 11, 16);
		writer.write_all(&js).ok()?;
	}
	Some(Arc::new((js, compressed)))
}
