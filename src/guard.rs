use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::middleware::Next;
use axum::response::Response;

use crate::web::{Web, fail};

// the event endpoints take posts from any site, everything else answers its own pages only
fn open_to_sites(web: &Web, path: &str) -> bool {
	matches!(path, "/api/event" | "/count")
		|| path.starts_with("/ping/")
		|| path.starts_with("/api/push/")
		|| web
			.alias
			.as_ref()
			.is_some_and(|alias| path.strip_prefix(alias.as_str()) == Some("/e"))
}

// a browser names where a request comes from, so a form on another site cannot ride the session cookie
fn same_origin(web: &Web, headers: &HeaderMap) -> bool {
	if let Some(site) = headers.get("sec-fetch-site") {
		return matches!(site.as_bytes(), b"same-origin" | b"none");
	}
	let Some(origin) = headers.get(header::ORIGIN) else {
		return true;
	};
	let Ok(origin) = origin.to_str() else {
		return false;
	};
	let host = headers
		.get(header::HOST)
		.and_then(|host| host.to_str().ok());
	origin.split_once("://").map(|(_, authority)| authority) == host
		|| origin == web.url.trim_end_matches('/')
}

pub async fn guard(State(web): State<Arc<Web>>, request: Request, next: Next) -> Response {
	let path = request.uri().path().to_owned();
	let writes = !matches!(
		*request.method(),
		Method::GET | Method::HEAD | Method::OPTIONS
	);
	if writes && !open_to_sites(&web, &path) && !same_origin(&web, request.headers()) {
		return fail(
			StatusCode::FORBIDDEN,
			"Cross-site requests are not allowed.",
		);
	}
	let mut response = next.run(request).await;
	let headers = response.headers_mut();
	headers.insert(
		header::X_CONTENT_TYPE_OPTIONS,
		HeaderValue::from_static("nosniff"),
	);
	if !headers.contains_key(header::REFERRER_POLICY) {
		headers.insert(
			header::REFERRER_POLICY,
			HeaderValue::from_static("same-origin"),
		);
	}
	if path.starts_with("/api/") && !headers.contains_key(header::CACHE_CONTROL) {
		headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
	}
	if web.url.starts_with("https://") {
		headers.insert(
			header::STRICT_TRANSPORT_SECURITY,
			HeaderValue::from_static("max-age=31536000"),
		);
	}
	response
}
