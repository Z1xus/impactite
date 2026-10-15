use std::sync::Arc;

use axum::Router;
use axum::http::{HeaderValue, StatusCode, header};
use axum::middleware::map_response;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};

use crate::accounts::{self, Web};
use crate::admin;
use crate::api;
use crate::assets;
use crate::counter;
use crate::export;
use crate::funnels;
use crate::goals;
use crate::google;
use crate::icons;
use crate::ingest::{Ingest, goatcounter, plausible};
use crate::keys;
use crate::passkeys;
use crate::prefs;
use crate::profile;
use crate::reports;
use crate::segments;
use crate::shares;
use crate::shields;
use crate::sites;
use crate::stats::{self, Stats};
use crate::teams;
use crate::titles;
use crate::totp;
use crate::tracker;

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
				.merge(passkeys::routes())
				.merge(goals::routes())
				.merge(funnels::routes())
				.merge(keys::routes())
				.merge(sites::routes())
				.merge(teams::routes())
				.merge(profile::routes())
				.merge(admin::routes())
				.merge(segments::routes())
				.merge(prefs::routes())
				.merge(shields::routes())
				.merge(shares::routes())
				.merge(reports::routes())
				.merge(titles::routes())
				.merge(google::routes())
				.merge(totp::routes())
				.merge(api::routes())
				.route("/api/app/signup", post(accounts::signup))
				.route("/api/app/query", post(stats::dashboard))
				.route("/api/app/props", post(stats::props))
				.route("/api/app/export", post(export::export))
				.route("/api/app/icon/{host}", get(icons::icon))
				.route("/counter/{*file}", get(counter::counter))
				.with_state(web),
		)
		.route("/js/{file}", get(tracker::script))
		.route("/count.js", get(tracker::count))
		.fallback(assets::serve)
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
