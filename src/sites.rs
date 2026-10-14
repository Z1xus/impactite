use std::fs;
use std::ops::RangeInclusive;
use std::sync::Arc;

use anyhow::bail;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, patch};
use axum::{Json, Router};
use jiff::civil::Date;
use jiff::tz::TimeZone;
use serde::Deserialize;
use serde_json::json;

use crate::accounts::{Viewer, Web, fail};
use crate::app::{Site, token};
use crate::import;
use crate::stats::day_number;

const UPLOAD: usize = 1 << 30;

pub fn routes() -> Router<Arc<Web>> {
	Router::new()
		.route("/api/app/sites/{domain}", patch(update))
		.route(
			"/api/app/sites/{domain}/imports",
			get(list).post(upload).layer(DefaultBodyLimit::max(UPLOAD)),
		)
		.route("/api/app/sites/{domain}/imports/{id}", delete(remove))
}

#[derive(Deserialize)]
struct Update {
	timezone: String,
}

#[derive(Deserialize)]
struct Upload {
	source: String,
}

async fn update(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path(domain): Path<String>,
	Json(body): Json<Update>,
) -> Response {
	let Some(site) = editable(&web, viewer.user, &domain) else {
		return forbidden();
	};
	if TimeZone::get(&body.timezone).is_err() {
		return fail(StatusCode::BAD_REQUEST, "Use a valid timezone.");
	}
	match web.app.set_timezone(site.id, &body.timezone) {
		Ok(()) => {
			web.ingest.forget(&site.domain);
			StatusCode::NO_CONTENT.into_response()
		}
		Err(_) => broken(),
	}
}

async fn list(State(web): State<Arc<Web>>, viewer: Viewer, Path(domain): Path<String>) -> Response {
	match editable(&web, viewer.user, &domain) {
		Some(site) => respond(&web, &site),
		None => forbidden(),
	}
}

async fn upload(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path(domain): Path<String>,
	Query(upload): Query<Upload>,
	body: Bytes,
) -> Response {
	let Some(site) = editable(&web, viewer.user, &domain) else {
		return forbidden();
	};
	let shared = web.clone();
	let done = tokio::task::spawn_blocking(move || -> anyhow::Result<String> {
		let dir = shared.data.join("uploads");
		fs::create_dir_all(&dir)?;
		let extension = match (upload.source.as_str(), body.starts_with(&[0x1f, 0x8b])) {
			("plausible", _) => "zip",
			("goatcounter", true) => "csv.gz",
			("goatcounter", false) => "csv",
			_ => bail!("Choose Plausible or GoatCounter."),
		};
		let path = dir.join(format!("{}.{extension}", token()?));
		fs::write(&path, &body)?;
		let db = &shared.stats.db;
		let done = match extension {
			"zip" => import::plausible_zip(db, &shared.app, &site, &path)
				.map(|days| format!("{days} days")),
			_ => import::goatcounter_csv(db, &shared.app, &site, &path)
				.map(|events| format!("{events} events")),
		};
		let _ = fs::remove_file(&path);
		done
	})
	.await;
	match done {
		Ok(Ok(done)) => Json(json!({ "imported": done })).into_response(),
		Ok(Err(err)) => fail(StatusCode::BAD_REQUEST, &err.to_string()),
		Err(_) => broken(),
	}
}

async fn remove(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path((domain, id)): Path<(String, i64)>,
) -> Response {
	let Some(site) = editable(&web, viewer.user, &domain) else {
		return forbidden();
	};
	let Ok(Some((first, last))) = web.app.remove_import(site.id, id) else {
		return fail(StatusCode::NOT_FOUND, "Import not found.");
	};
	match days(&first, &last).map(|days| web.stats.db.drop_imports(site.id, days)) {
		Some(Ok(())) => respond(&web, &site),
		_ => broken(),
	}
}

fn days(first: &str, last: &str) -> Option<RangeInclusive<i32>> {
	let first: Date = first.parse().ok()?;
	let last: Date = last.parse().ok()?;
	Some(day_number(first)..=day_number(last))
}

fn editable(web: &Web, user: i64, domain: &str) -> Option<Site> {
	let role = web.app.site_role(user, domain).ok().flatten()?;
	(role != "viewer").then_some(())?;
	web.app.site(domain).ok().flatten()
}

fn respond(web: &Web, site: &Site) -> Response {
	match web.app.imports(site.id) {
		Ok(imports) => Json(json!({
			"imports": imports
				.iter()
				.map(|(id, source, first, last)| json!({ "id": id, "source": source, "first": first, "last": last }))
				.collect::<Vec<_>>()
		}))
		.into_response(),
		Err(_) => broken(),
	}
}

fn forbidden() -> Response {
	fail(StatusCode::FORBIDDEN, "You can't change this site.")
}

fn broken() -> Response {
	fail(StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong.")
}
