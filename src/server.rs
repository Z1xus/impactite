use std::sync::Arc;

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::http::{HeaderValue, StatusCode, header};
use axum::middleware::{from_fn_with_state, map_response};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};

use crate::api;
use crate::assets;
use crate::auth::{accounts, admin, keys, passkeys, prefs, profile, sso, teams, totp};
use crate::batch;
use crate::export;
use crate::guard::guard;
use crate::icons;
use crate::ingest::{Ingest, goatcounter, plausible};
use crate::site;
use crate::site::{counter, funnels, goals, google, reports, segments, shares, shields, titles};
use crate::stats::{self, Stats};
use crate::tracker;
use crate::uptime;
use crate::web::Web;

pub fn router(ingest: Arc<Ingest>, stats: Arc<Stats>, web: Arc<Web>) -> Router {
	let mut events = Router::new()
		.route(
			"/api/event",
			post(plausible::event)
				.options(preflight)
				.layer(DefaultBodyLimit::max(1 << 17)),
		)
		.route("/count", get(goatcounter::count).post(goatcounter::count));
	let mut scripts = Router::new()
		.route("/js/{file}", get(tracker::script))
		.route("/count.js", get(tracker::count));
	if let Some(alias) = &web.alias {
		events = events.route(
			&format!("{alias}/e"),
			post(plausible::event).options(preflight),
		);
		scripts = scripts.route(&format!("{alias}/{{file}}"), get(tracker::script));
	}
	let app = events
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
				.merge(site::routes())
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
				.merge(sso::routes())
				.merge(api::routes())
				.merge(uptime::routes())
				.route("/api/app/signup", post(accounts::signup))
				.route("/api/app/query", post(stats::dashboard))
				.route("/api/app/props", post(stats::props))
				.route("/api/app/export", post(export::export))
				.route("/api/app/icon/{host}", get(icons::icon))
				.route("/counter/{*file}", get(counter::counter))
				.with_state(web.clone()),
		)
		.merge(scripts);
	let pings = uptime::push::routes()
		.with_state(web.clone())
		.layer(map_response(cors));
	let batch = Router::new()
		.route("/api/app/batch", post(batch::batch))
		.with_state(app.clone());
	app.merge(batch)
		.merge(pings)
		.merge(
			Router::new()
				.fallback(assets::serve)
				.with_state(web.clone()),
		)
		.layer(from_fn_with_state(web, guard))
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
