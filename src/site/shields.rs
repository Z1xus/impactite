use std::collections::HashSet;
use std::net::IpAddr;
use std::sync::Arc;

use axum::extract::{ConnectInfo, Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get};
use axum::{Json, Router};
use crater::glob;
use rusqlite::params;
use serde::Deserialize;
use serde_json::json;

use crate::app::{App, Site};
use crate::auth::accounts::Viewer;
use crate::net::Proxies;
use crate::web::{Web, fail, internal};

const KINDS: [&str; 5] = ["ip", "country", "page", "hostname", "prop"];

#[derive(Default)]
pub struct Shields {
	ips: Option<Proxies>,
	countries: HashSet<String>,
	pages: Vec<String>,
	hostnames: Vec<String>,
	props: HashSet<String>,
}

impl Shields {
	pub fn load(app: &App, site: u64) -> Self {
		let mut shields = Self::default();
		let mut ips = Vec::new();
		for (_, kind, value) in app.shields(site).unwrap_or_default() {
			match kind.as_str() {
				"ip" => ips.push(value),
				"country" => {
					shields.countries.insert(value);
				}
				"page" => shields.pages.push(value),
				"hostname" => shields.hostnames.push(value),
				_ => {
					shields.props.insert(value);
				}
			}
		}
		shields.ips = (!ips.is_empty())
			.then(|| Proxies::parse(Some(&ips.join(","))).ok())
			.flatten();
		shields
	}

	pub fn blocks(&self, ip: IpAddr, country: &str, page: &str, hostname: &str) -> bool {
		self.ips.as_ref().is_some_and(|ips| ips.contains(ip))
			|| self.countries.contains(country)
			|| self.pages.iter().any(|pattern| glob(pattern, page))
			|| (!self.hostnames.is_empty()
				&& !self.hostnames.iter().any(|pattern| glob(pattern, hostname)))
	}

	pub fn keeps(&self, key: &str) -> bool {
		self.props.is_empty() || self.props.contains(key)
	}
}

pub fn routes() -> Router<Arc<Web>> {
	Router::new()
		.route("/api/app/sites/{domain}/shields", get(list).post(add))
		.route("/api/app/sites/{domain}/shields/{id}", delete(remove))
		.route("/api/app/ip", get(ip))
}

#[derive(Deserialize)]
struct Rule {
	kind: String,
	value: String,
}

async fn list(State(web): State<Arc<Web>>, viewer: Viewer, Path(domain): Path<String>) -> Response {
	match web.app.editable(viewer.user, &domain) {
		Some(site) => respond(&web, site.id),
		None => fail(StatusCode::FORBIDDEN, "You can't change this site."),
	}
}

async fn add(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path(domain): Path<String>,
	Json(body): Json<Rule>,
) -> Response {
	let Some(Site {
		id: site, domain, ..
	}) = web.app.editable(viewer.user, &domain)
	else {
		return fail(StatusCode::FORBIDDEN, "You can't change this site.");
	};
	let value = body.value.trim();
	let valid = match body.kind.as_str() {
		"ip" => Proxies::parse(Some(value)).is_ok(),
		"country" => value.len() == 2 && value.chars().all(|c| c.is_ascii_alphabetic()),
		kind => KINDS.contains(&kind) && !value.is_empty() && value.len() <= 300,
	};
	if !valid || value.is_empty() {
		return fail(StatusCode::BAD_REQUEST, "This rule is not valid.");
	}
	let value = match body.kind.as_str() {
		"country" => value.to_uppercase(),
		_ => value.to_owned(),
	};
	match web.app.add_shield(site, &body.kind, &value) {
		Ok(()) => {
			web.ingest.forget(&domain);
			respond(&web, site)
		}
		Err(err) => internal(err),
	}
}

async fn remove(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path((domain, id)): Path<(String, i64)>,
) -> Response {
	let Some(Site {
		id: site, domain, ..
	}) = web.app.editable(viewer.user, &domain)
	else {
		return fail(StatusCode::FORBIDDEN, "You can't change this site.");
	};
	match web.app.remove_shield(site, id) {
		Ok(()) => {
			web.ingest.forget(&domain);
			respond(&web, site)
		}
		Err(err) => internal(err),
	}
}

async fn ip(
	State(web): State<Arc<Web>>,
	_viewer: Viewer,
	ConnectInfo(peer): ConnectInfo<std::net::SocketAddr>,
	headers: HeaderMap,
) -> Response {
	Json(json!({ "ip": web.ingest.proxies.visitor(peer, &headers).ip.to_string() })).into_response()
}

fn respond(web: &Web, site: u64) -> Response {
	match web.app.shields(site) {
		Ok(rules) => Json(json!({
			"shields": rules
				.iter()
				.map(|(id, kind, value)| json!({ "id": id, "kind": kind, "value": value }))
				.collect::<Vec<_>>()
		}))
		.into_response(),
		Err(err) => internal(err),
	}
}

impl App {
	pub(crate) fn shields(&self, site: u64) -> rusqlite::Result<Vec<(i64, String, String)>> {
		let conn = self.conn();
		let mut stmt = conn
			.prepare("select id, kind, value from shields where site = ?1 order by kind, value")?;
		stmt.query_map([site as i64], |row| {
			Ok((row.get(0)?, row.get(1)?, row.get(2)?))
		})?
		.collect()
	}

	pub(crate) fn add_shield(&self, site: u64, kind: &str, value: &str) -> rusqlite::Result<()> {
		self.conn().execute(
			"insert into shields (site, kind, value) values (?1, ?2, ?3) on conflict do nothing",
			params![site as i64, kind, value],
		)?;
		Ok(())
	}

	pub(crate) fn remove_shield(&self, site: u64, id: i64) -> rusqlite::Result<()> {
		self.conn().execute(
			"delete from shields where site = ?1 and id = ?2",
			params![site as i64, id],
		)?;
		Ok(())
	}
}
