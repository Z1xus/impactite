use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use axum::Router;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::middleware::map_response;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};

use crate::accounts::{self, Web};
use crate::ingest::{Ingest, goatcounter, plausible};
use crate::stats::{self, Stats};

pub fn router(ingest: Arc<Ingest>, stats: Arc<Stats>, web: Arc<Web>) -> Router {
	Router::new()
		.route("/api/event", post(plausible::event).options(preflight))
		.route("/count", get(goatcounter::count).post(goatcounter::count))
		.with_state(ingest)
		.layer(map_response(cors))
		.merge(
			Router::new()
				.route("/api/v2/query", post(stats::query))
				.with_state(stats),
		)
		.merge(
			accounts::routes()
				.route("/api/app/query", post(stats::dashboard))
				.with_state(web),
		)
}

async fn preflight() -> Response {
	let mut response = StatusCode::NO_CONTENT.into_response();
	let headers = response.headers_mut();
	headers.insert(
		header::ACCESS_CONTROL_ALLOW_METHODS,
		HeaderValue::from_static("GET, POST"),
	);
	headers.insert(
		header::ACCESS_CONTROL_ALLOW_HEADERS,
		HeaderValue::from_static("content-type"),
	);
	headers.insert(
		header::ACCESS_CONTROL_MAX_AGE,
		HeaderValue::from_static("86400"),
	);
	response
}

async fn cors(mut response: Response) -> Response {
	response.headers_mut().insert(
		header::ACCESS_CONTROL_ALLOW_ORIGIN,
		HeaderValue::from_static("*"),
	);
	response
}

pub fn client_ip(peer: SocketAddr, headers: &HeaderMap) -> IpAddr {
	let peer_ip = peer.ip().to_canonical();
	let trusted = match peer_ip {
		IpAddr::V4(ip) => ip.is_loopback() || ip.is_private(),
		IpAddr::V6(ip) => ip.is_loopback() || ip.is_unique_local(),
	};
	if !trusted {
		return peer_ip;
	}
	["cf-connecting-ip", "x-forwarded-for", "x-real-ip"]
		.into_iter()
		.filter_map(|name| headers.get(name)?.to_str().ok())
		.filter_map(|value| value.split(',').next()?.trim().parse().ok())
		.next()
		.unwrap_or(peer_ip)
}
