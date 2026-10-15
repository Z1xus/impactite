use std::convert::Infallible;
use std::sync::Arc;

use argon2::{Argon2, PasswordVerifier};
use axum::extract::{FromRequestParts, Path, State};
use axum::http::StatusCode;
use axum::http::request::Parts;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use rusqlite::{OptionalExtension, params};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::accounts::{Viewer, Web, fail, hash, internal};
use crate::app::{App, token};

// a signed in member or a share link token, for read only access
pub struct Reader {
	user: Option<i64>,
	share: Option<String>,
}

impl FromRequestParts<Arc<Web>> for Reader {
	type Rejection = Infallible;

	async fn from_request_parts(parts: &mut Parts, web: &Arc<Web>) -> Result<Self, Infallible> {
		let user = Viewer::from_request_parts(parts, web)
			.await
			.ok()
			.map(|viewer| viewer.user);
		let share = parts
			.headers
			.get("x-share")
			.and_then(|value| value.to_str().ok())
			.map(str::to_owned)
			.or_else(|| {
				// event streams cannot send headers
				parts.uri.query().and_then(|query| {
					url::form_urlencoded::parse(query.as_bytes())
						.find(|(key, _)| key == "share")
						.map(|(_, value)| value.into_owned())
				})
			});
		Ok(Self { user, share })
	}
}

impl Reader {
	// someone signed in, or holding a working share link
	pub fn anyone(&self, web: &Web) -> bool {
		self.user.is_some()
			|| self
				.share
				.as_deref()
				.map(|share| share.rsplit_once('.').unwrap_or((share, "")))
				.is_some_and(
					|(slug, grant)| matches!(web.app.share(slug), Ok(Some(share)) if share.grant() == grant),
				)
	}

	pub fn reads(&self, web: &Web, domain: &str) -> bool {
		if let Some(user) = self.user {
			// a whole team, its members read it
			let role = match domain.strip_prefix("team:") {
				Some(team) => team
					.parse()
					.map_or(Ok(None), |team| web.app.role(user, team)),
				None => web.app.site_role(user, domain),
			};
			if matches!(role, Ok(Some(_))) {
				return true;
			}
		}
		let Some((slug, grant)) = self
			.share
			.as_deref()
			.map(|share| share.rsplit_once('.').unwrap_or((share, "")))
		else {
			return false;
		};
		matches!(web.app.share(slug), Ok(Some(share)) if share.domain == domain && share.grant() == grant)
	}
}

pub struct Share {
	slug: String,
	domain: String,
	timezone: String,
	name: String,
	password: String,
}

impl Share {
	// changes with the password, so a new password locks out old links
	fn grant(&self) -> String {
		match self.password.is_empty() {
			true => String::new(),
			false => blake3::hash(format!("{}:{}", self.slug, self.password).as_bytes()).to_hex()
				[..32]
				.to_owned(),
		}
	}
}

pub fn routes() -> Router<Arc<Web>> {
	Router::new()
		.route("/api/app/sites/{domain}/shares", get(list).post(add))
		.route("/api/app/sites/{domain}/shares/{id}", delete(remove))
		.route("/api/app/shares/{slug}", post(open))
}

#[derive(Deserialize)]
struct NewShare {
	name: String,
	#[serde(default)]
	password: String,
	#[serde(default)]
	public: bool,
}

#[derive(Deserialize)]
struct Unlock {
	#[serde(default)]
	password: String,
}

fn editable(web: &Web, user: i64, domain: &str) -> Option<(u64, String)> {
	let role = web.app.site_role(user, domain).ok().flatten()?;
	(role != "viewer").then_some(())?;
	let site = web.app.site(domain).ok().flatten()?;
	Some((site.id, site.domain))
}

async fn list(State(web): State<Arc<Web>>, viewer: Viewer, Path(domain): Path<String>) -> Response {
	match editable(&web, viewer.user, &domain) {
		Some((site, _)) => respond(&web, site),
		None => fail(StatusCode::FORBIDDEN, "You can't change this site."),
	}
}

async fn add(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path(domain): Path<String>,
	Json(body): Json<NewShare>,
) -> Response {
	let Some((site, domain)) = editable(&web, viewer.user, &domain) else {
		return fail(StatusCode::FORBIDDEN, "You can't change this site.");
	};
	let (slug, name, password) = match body.public {
		true => (domain.clone(), "public".to_owned(), String::new()),
		false => {
			let name = body.name.trim();
			if name.is_empty() || name.len() > 80 {
				return fail(StatusCode::BAD_REQUEST, "Name the link.");
			}
			let password = match body.password.is_empty() {
				true => String::new(),
				false => match hash(&body.password) {
					Ok(hash) => hash,
					Err(err) => return internal(err),
				},
			};
			match token() {
				Ok(token) => (token[..20].to_owned(), name.to_owned(), password),
				Err(err) => return internal(err),
			}
		}
	};
	match web.app.add_share(site, &slug, &name, &password) {
		Ok(()) => respond(&web, site),
		Err(err) => internal(err),
	}
}

async fn remove(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path((domain, id)): Path<(String, i64)>,
) -> Response {
	let Some((site, _)) = editable(&web, viewer.user, &domain) else {
		return fail(StatusCode::FORBIDDEN, "You can't change this site.");
	};
	match web.app.remove_share(site, id) {
		Ok(()) => respond(&web, site),
		Err(err) => internal(err),
	}
}

async fn open(
	State(web): State<Arc<Web>>,
	Path(slug): Path<String>,
	Json(body): Json<Unlock>,
) -> Response {
	let Ok(Some(share)) = web.app.share(&slug) else {
		return fail(StatusCode::NOT_FOUND, "This link does not exist.");
	};
	if !share.password.is_empty() {
		let valid = !body.password.is_empty()
			&& argon2::PasswordHash::new(&share.password).is_ok_and(|parsed| {
				Argon2::default()
					.verify_password(body.password.as_bytes(), &parsed)
					.is_ok()
			});
		if !valid {
			return (
				StatusCode::UNAUTHORIZED,
				Json(json!({ "error": "Enter the password for this link.", "locked": true })),
			)
				.into_response();
		}
	}
	Json(json!({
		"domain": share.domain,
		"timezone": share.timezone,
		"name": share.name,
		"token": format!("{}.{}", share.slug, share.grant()),
	}))
	.into_response()
}

fn respond(web: &Web, site: u64) -> Response {
	match web.app.shares(site) {
		Ok(shares) => Json(json!({ "shares": shares })).into_response(),
		Err(err) => internal(err),
	}
}

impl App {
	fn share(&self, slug: &str) -> rusqlite::Result<Option<Share>> {
		self.conn()
			.query_row(
				"select h.slug, s.domain, s.timezone, h.name, h.password from shares h
				join sites s on s.id = h.site where h.slug = ?1 and s.domain not like '~%'",
				[slug],
				|row| {
					Ok(Share {
						slug: row.get(0)?,
						domain: row.get(1)?,
						timezone: row.get(2)?,
						name: row.get(3)?,
						password: row.get(4)?,
					})
				},
			)
			.optional()
	}

	pub(crate) fn shares(&self, site: u64) -> rusqlite::Result<Vec<Value>> {
		let conn = self.conn();
		let mut stmt = conn.prepare(
			"select id, slug, name, password != '' from shares where site = ?1 order by id",
		)?;
		stmt.query_map([site as i64], |row| {
			Ok(json!({
				"id": row.get::<_, i64>(0)?,
				"slug": row.get::<_, String>(1)?,
				"name": row.get::<_, String>(2)?,
				"locked": row.get::<_, bool>(3)?,
			}))
		})?
		.collect()
	}

	pub(crate) fn add_share(
		&self,
		site: u64,
		slug: &str,
		name: &str,
		password: &str,
	) -> rusqlite::Result<()> {
		self.conn().execute(
			"insert into shares (site, slug, name, password, created_at) values (?1, ?2, ?3, ?4, unixepoch())
			on conflict (slug) do nothing",
			params![site as i64, slug, name, password],
		)?;
		Ok(())
	}

	fn remove_share(&self, site: u64, id: i64) -> rusqlite::Result<()> {
		self.conn().execute(
			"delete from shares where site = ?1 and id = ?2",
			params![site as i64, id],
		)?;
		Ok(())
	}
}
