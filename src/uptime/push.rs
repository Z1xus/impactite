use std::net::SocketAddr;
use std::sync::{Arc, LazyLock};

use axum::Router;
use axum::extract::{ConnectInfo, Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get};
use serde::Deserialize;
use serde_json::json;

use crate::limit::Limit;
use crate::uptime::run::{Msg, Ping};
use crate::web::Web;

// a job pings once per run, so a minute of 60 is plenty
static PINGS: LazyLock<Limit> = LazyLock::new(|| Limit::new(60, 1));
static GUESSES: LazyLock<Limit> = LazyLock::new(|| Limit::new(30, 10));

pub fn routes() -> Router<Arc<Web>> {
	Router::new()
		.route("/ping/{token}", any(up))
		.route("/ping/{token}/fail", any(fail))
		.route("/ping/{token}/start", any(start))
		.route("/api/push/{token}", get(kuma))
}

#[derive(Deserialize)]
struct Push {
	status: Option<String>,
	msg: Option<String>,
	ping: Option<String>,
}

enum Reply {
	Sent,
	Unknown,
	Busy,
}

fn hit(web: &Web, peer: SocketAddr, headers: &HeaderMap, token: &str, ping: Ping) -> Reply {
	let client = web.client(peer, headers);
	if !GUESSES.open(&client) || !PINGS.open(token) {
		return Reply::Busy;
	}
	let Some(id) = web.app.monitor_of_token(token) else {
		GUESSES.strike(&client);
		return Reply::Unknown;
	};
	PINGS.strike(token);
	web.uptime.send(Msg::Ping(id, ping));
	Reply::Sent
}

fn plain(reply: Reply) -> Response {
	match reply {
		Reply::Sent => "OK".into_response(),
		Reply::Unknown => (StatusCode::NOT_FOUND, "Not found").into_response(),
		Reply::Busy => (StatusCode::TOO_MANY_REQUESTS, "Slow down").into_response(),
	}
}

async fn up(
	State(web): State<Arc<Web>>,
	ConnectInfo(peer): ConnectInfo<SocketAddr>,
	headers: HeaderMap,
	Path(token): Path<String>,
) -> Response {
	plain(hit(&web, peer, &headers, &token, Ping::Up(None)))
}

async fn fail(
	State(web): State<Arc<Web>>,
	ConnectInfo(peer): ConnectInfo<SocketAddr>,
	headers: HeaderMap,
	Path(token): Path<String>,
) -> Response {
	plain(hit(&web, peer, &headers, &token, Ping::Down(String::new())))
}

async fn start(
	State(web): State<Arc<Web>>,
	ConnectInfo(peer): ConnectInfo<SocketAddr>,
	headers: HeaderMap,
	Path(token): Path<String>,
) -> Response {
	plain(hit(&web, peer, &headers, &token, Ping::Start))
}

async fn kuma(
	State(web): State<Arc<Web>>,
	ConnectInfo(peer): ConnectInfo<SocketAddr>,
	headers: HeaderMap,
	Path(token): Path<String>,
	Query(push): Query<Push>,
) -> Response {
	let ping = if push.status.as_deref() == Some("down") {
		let msg = push.msg.unwrap_or_default();
		Ping::Down(msg.chars().take(100).collect())
	} else {
		Ping::Up(
			push.ping
				.and_then(|ms| ms.parse::<f64>().ok())
				.map(|ms| ms.clamp(0.0, 65_535.0) as u32),
		)
	};
	match hit(&web, peer, &headers, &token, ping) {
		Reply::Sent => axum::Json(json!({ "ok": true })).into_response(),
		Reply::Unknown => (
			StatusCode::NOT_FOUND,
			axum::Json(json!({ "ok": false, "msg": "Monitor not found or not active." })),
		)
			.into_response(),
		Reply::Busy => (
			StatusCode::TOO_MANY_REQUESTS,
			axum::Json(json!({ "ok": false, "msg": "Too many requests." })),
		)
			.into_response(),
	}
}
