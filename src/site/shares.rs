use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::{Arc, LazyLock};

use argon2::{Argon2, PasswordVerifier};
use axum::extract::{ConnectInfo, FromRequestParts, Path, State};
use axum::http::request::Parts;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use rusqlite::{OptionalExtension, params};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::app::{App, Site, same, token};
use crate::auth::accounts::{Viewer, hash};
use crate::limit::Limit;
use crate::web::{Error, Reply, Web};

static LINKS: LazyLock<Limit> = LazyLock::new(|| Limit::new(10, 10));
static CLIENTS: LazyLock<Limit> = LazyLock::new(|| Limit::new(30, 10));

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
	pub fn anyone(&self, web: &Web) -> bool {
		self.user.is_some()
			|| self
				.share
				.as_deref()
				.map(|share| share.rsplit_once('.').unwrap_or((share, "")))
				.is_some_and(
					|(slug, grant)| matches!(web.app.share(slug), Ok(Some(share)) if same(&share.grant(), grant)),
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
		matches!(web.app.share(slug), Ok(Some(share)) if share.domain == domain && same(&share.grant(), grant))
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
	fn opened(&self) -> Value {
		json!({
			"domain": self.domain,
			"timezone": self.timezone,
			"name": self.name,
			"token": format!("{}.{}", self.slug, self.grant()),
		})
	}

	// changes with the password, so a new password locks out old links
	fn grant(&self) -> String {
		if self.password.is_empty() {
			String::new()
		} else {
			blake3::hash(format!("{}:{}", self.slug, self.password).as_bytes()).to_hex()[..32]
				.to_owned()
		}
	}
}

pub fn routes() -> Router<Arc<Web>> {
	Router::new()
		.route("/api/app/sites/{domain}/shares", get(list).post(add))
		.route("/api/app/sites/{domain}/shares/{id}", delete(remove))
		.route("/api/app/shares/{slug}", post(open))
		.route("/api/app/shares/{slug}/card.png", get(card))
		.route("/share/{slug}", get(page))
}

// visitors in the last 30 days, the 30 before, and each day, for link previews of public shares
fn month(web: &Web, share: &Share) -> Option<(u64, u64, Vec<u64>)> {
	let tz = jiff::tz::TimeZone::get(&share.timezone).unwrap_or(jiff::tz::TimeZone::UTC);
	let today = jiff::Timestamp::now().to_zoned(tz).date();
	let ask = |from: jiff::civil::Date, daily: bool| {
		crate::stats::run(
			&web.stats,
			json!({
				"site_id": share.domain,
				"date_range": [from.to_string(), today.to_string()],
				"metrics": ["visitors"],
				"dimensions": if daily { vec!["time:day"] } else { Vec::new() },
				"pagination": { "limit": 100 },
				"include": { "imports": true },
			}),
		)
		.ok()
	};
	let start = today.checked_sub(jiff::ToSpan::days(29)).ok()?;
	let before = today.checked_sub(jiff::ToSpan::days(59)).ok()?;
	let now = ask(start, false)?["results"][0]["metrics"][0]
		.as_u64()
		.unwrap_or(0);
	let both = ask(before, false)?["results"][0]["metrics"][0]
		.as_u64()
		.unwrap_or(0);
	let daily = ask(start, true)?;
	let rows = daily["results"].as_array().cloned().unwrap_or_default();
	let series = start
		.series(jiff::ToSpan::days(1))
		.take(30)
		.map(|day| {
			rows.iter()
				.find(|row| row["dimensions"][0].as_str() == Some(&day.to_string()))
				.and_then(|row| row["metrics"][0].as_u64())
				.unwrap_or(0)
		})
		.collect();
	Some((now, both.saturating_sub(now), series))
}

fn change(now: u64, then: u64) -> Option<i64> {
	(then > 0).then(|| ((now as f64 - then as f64) / then as f64 * 100.0).round() as i64)
}

async fn page(
	State(web): State<Arc<Web>>,
	Path(slug): Path<String>,
	headers: HeaderMap,
) -> Response {
	let opened = match web.app.share(&slug) {
		Ok(Some(share)) if share.password.is_empty() => Some(share.opened()),
		_ => None,
	};
	let head = match web.app.share(&slug) {
		Ok(Some(share)) if share.password.is_empty() => month(&web, &share)
			.map(|(now, then, _)| {
				let escape = |text: &str| {
					text.replace('&', "&amp;")
						.replace('<', "&lt;")
						.replace('>', "&gt;")
						.replace('"', "&quot;")
				};
				let title = escape(&format!(
					"{}: {} visitors in the last 30 days",
					share.domain,
					crate::notify::grouped(now)
				));
				let description = match change(now, then) {
					Some(change) if change >= 0 => format!("Up {change}% on the 30 days before."),
					Some(change) => format!("Down {}% on the 30 days before.", -change),
					None => "Shared with impactite.".to_owned(),
				};
				let image = escape(&format!(
					"{}/api/app/shares/{slug}/card.png",
					web.url.trim_end_matches('/')
				));
				format!(
					"<meta property=\"og:title\" content=\"{title}\"><meta property=\"og:description\" content=\"{description}\"><meta property=\"og:image\" content=\"{image}\"><meta property=\"og:image:width\" content=\"1200\"><meta property=\"og:image:height\" content=\"630\"><meta property=\"og:site_name\" content=\"impactite\"><meta name=\"twitter:card\" content=\"summary_large_image\">"
				)
			})
			.unwrap_or_default(),
		_ => String::new(),
	};
	crate::assets::page(&web, &headers, Some((&head, opened)))
}

async fn card(State(web): State<Arc<Web>>, Path(slug): Path<String>) -> Response {
	use crate::card::{BAD, CYAN, Canvas, DIM, FAINT, FG, INK};
	let share = match web.app.share(&slug) {
		Ok(Some(share)) if share.password.is_empty() => share,
		_ => return StatusCode::NOT_FOUND.into_response(),
	};
	let Some((now, then, series)) = month(&web, &share) else {
		return StatusCode::NOT_FOUND.into_response();
	};
	let mut canvas = Canvas::new(1200, 630, INK);
	canvas.text(
		72,
		64,
		3,
		DIM,
		&format!("{} last 30 days", share.domain).to_uppercase(),
	);
	canvas.text(1200 - 72 - 9 * 7 * 3, 64, 3, FAINT, "IMPACTITE");
	canvas.text(66, 130, 10, FG, &crate::notify::grouped(now));
	if let Some(change) = change(now, then) {
		let color = if change >= 0 { CYAN } else { BAD };
		canvas.text(72, 290, 3, color, &format!("{change:+}% VISITORS"));
	}
	canvas.area([0, 370, 1200, 260], &series, 10);
	(
		[
			(axum::http::header::CONTENT_TYPE, "image/png"),
			(axum::http::header::CACHE_CONTROL, "public, max-age=3600"),
		],
		canvas.png(),
	)
		.into_response()
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

async fn list(State(web): State<Arc<Web>>, viewer: Viewer, Path(domain): Path<String>) -> Reply {
	respond(&web, web.editable(viewer.user, &domain)?.id)
}

async fn add(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path(domain): Path<String>,
	Json(body): Json<NewShare>,
) -> Reply {
	let Site {
		id: site, domain, ..
	} = web.editable(viewer.user, &domain)?;
	let (slug, name, password) = if body.public {
		(domain, "public".to_owned(), String::new())
	} else {
		let name = body.name.trim();
		if name.is_empty() || name.len() > 80 {
			return Err(Error::bad("Name the link."));
		}
		let password = if body.password.is_empty() {
			String::new()
		} else {
			hash(&body.password)?
		};
		(token()?[..20].to_owned(), name.to_owned(), password)
	};
	web.app.add_share(site, &slug, &name, &password)?;
	respond(&web, site)
}

async fn remove(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path((domain, id)): Path<(String, i64)>,
) -> Reply {
	let Site { id: site, .. } = web.editable(viewer.user, &domain)?;
	web.app.remove_share(site, id)?;
	respond(&web, site)
}

async fn open(
	State(web): State<Arc<Web>>,
	ConnectInfo(peer): ConnectInfo<SocketAddr>,
	headers: HeaderMap,
	Path(slug): Path<String>,
	Json(body): Json<Unlock>,
) -> Reply<Response> {
	let share = web
		.app
		.share(&slug)?
		.ok_or_else(|| Error::not_found("This link does not exist."))?;
	if !share.password.is_empty() {
		let client = web.client(peer, &headers);
		if !LINKS.open(&slug) || !CLIENTS.open(&client) {
			return Err(Error::too_many());
		}
		if !matches(&share.password, &body.password).await? {
			LINKS.strike(&slug);
			CLIENTS.strike(&client);
			return Ok((
				StatusCode::UNAUTHORIZED,
				Json(json!({ "error": "Wrong password.", "locked": true })),
			)
				.into_response());
		}
	}
	Ok(Json(share.opened()).into_response())
}

pub(crate) async fn matches(stored: &str, given: &str) -> Reply<bool> {
	let (stored, given) = (stored.to_owned(), given.to_owned());
	Ok(tokio::task::spawn_blocking(move || {
		!given.is_empty()
			&& argon2::PasswordHash::new(&stored).is_ok_and(|parsed| {
				Argon2::default()
					.verify_password(given.as_bytes(), &parsed)
					.is_ok()
			})
	})
	.await?)
}

fn respond(web: &Web, site: u64) -> Reply {
	Ok(Json(json!({ "shares": web.app.shares(site)? })))
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
