use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::{ConnectInfo, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};

use super::{Client, Hit, Ingest};

const GIF: &[u8; 43] = b"GIF89a\x01\x00\x01\x00\x80\x00\x00\x00\x00\x00\xff\xff\xff!\xf9\x04\x01\x00\x00\x00\x00,\x00\x00\x00\x00\x01\x00\x01\x00\x00\x02\x02D\x01\x00;";

pub async fn count(
	State(ingest): State<Arc<Ingest>>,
	ConnectInfo(peer): ConnectInfo<SocketAddr>,
	headers: HeaderMap,
	Query(params): Query<Vec<(String, String)>>,
) -> Response {
	let param = |key: &str| {
		params
			.iter()
			.find(|(name, _)| name == key)
			.map_or("", |(_, value)| value.as_str())
	};
	let flag = |key: &str| matches!(param(key), "true" | "1" | "on");

	let header = |name| {
		headers
			.get(name)
			.and_then(|value| value.to_str().ok())
			.unwrap_or_default()
	};
	let host = header(header::HOST);
	let host = host.split(':').next().unwrap_or(host);
	// on a shared server the page that sent the hit names the site
	let page = [header(header::ORIGIN), header(header::REFERER)]
		.into_iter()
		.find_map(|value| url::Url::parse(value).ok()?.host_str().map(str::to_owned))
		.unwrap_or_default();
	let Some(site) = ingest
		.site(host)
		.or_else(|| ingest.site(&page))
		.or_else(|| ingest.only_site())
	else {
		return gif(
			StatusCode::BAD_REQUEST,
			Some(&format!("no site at this domain (\"{host}\")")),
		);
	};

	let path = param("p");
	if path.is_empty() || path.len() > 2048 {
		return gif(StatusCode::BAD_REQUEST, Some("not valid: path"));
	}
	match param("b").parse::<u32>() {
		Ok(1..150) => {
			return gif(
				StatusCode::BAD_REQUEST,
				Some(&format!("wrong value: b={}", param("b"))),
			);
		}
		Ok(150..) => return gif(StatusCode::OK, None),
		_ => {}
	}

	let event = flag("e");
	let query = param("q").trim_start_matches('?');
	let hit = Hit {
		name: match event {
			true => path.to_owned(),
			false => "pageview".into(),
		},
		hostname: site.site.domain.clone(),
		path: match event {
			true => String::new(),
			// count.js sends the query in the path too, it arrives apart in `q`
			false => format!(
				"/{}",
				path.split('?')
					.next()
					.unwrap_or(path)
					.trim_start_matches('/')
			),
		},
		params: url::form_urlencoded::parse(query.as_bytes())
			.into_owned()
			.collect(),
		referrer: param("r").to_owned(),
		props: Vec::new(),
		revenue: None,
		at: None,
		title: param("t").to_owned(),
		interactive: true,
		scroll_depth: None,
		engagement_ms: 0,
	};
	let ua = headers
		.get(header::USER_AGENT)
		.and_then(|ua| ua.to_str().ok())
		.unwrap_or_default();
	let visitor = ingest.proxies.visitor(peer, &headers);
	let client = Client {
		ip: visitor.ip,
		ua,
		country: visitor.country,
	};

	let _ = ingest.track(&site, hit, &client);
	gif(StatusCode::OK, None)
}

fn gif(status: StatusCode, reason: Option<&str>) -> Response {
	let mut response = (
		status,
		[(header::CONTENT_TYPE, "image/gif")],
		GIF.as_slice(),
	)
		.into_response();
	let headers = response.headers_mut();
	if let Ok(value) = "cross-origin".parse() {
		headers.insert("cross-origin-resource-policy", value);
	}
	if let Some(value) = reason.and_then(|reason| reason.parse().ok()) {
		headers.insert("x-goatcounter", value);
	}
	response
}
