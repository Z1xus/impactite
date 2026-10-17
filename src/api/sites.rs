use std::sync::Arc;

use axum::Router;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::Response;
use axum::routing::{delete, get, put};
use jiff::tz::TimeZone;
use rusqlite::params;
use serde_json::{Value, json};

use super::{Call, Fail, broken, reply};
use crate::app::{App, Site, token};
use crate::auth::teams;
use crate::site::goals::Goal;
use crate::site::{clean_domain, drop_site};
use crate::web::Web;

pub fn routes() -> Router<Arc<Web>> {
	Router::new()
		.route("/api/v1/sites", get(list).post(create))
		.route("/api/v1/sites/goals", get(goals).put(add_goal))
		.route("/api/v1/sites/goals/{id}", delete(remove_goal))
		.route("/api/v1/sites/guests", get(guests).put(invite))
		.route("/api/v1/sites/guests/{email}", delete(remove_guest))
		.route("/api/v1/sites/shared-links", put(share))
		.route("/api/v1/sites/custom-props", put(add_prop))
		.route("/api/v1/sites/custom-props/{property}", delete(remove_prop))
		.route(
			"/api/v1/sites/{site_id}",
			get(show).put(update).delete(destroy),
		)
}

fn taken(err: rusqlite::Error) -> Fail {
	match err {
		rusqlite::Error::SqliteFailure(err, _)
			if err.code == rusqlite::ErrorCode::ConstraintViolation =>
		{
			(
				StatusCode::BAD_REQUEST,
				"domain: This domain has already been taken".to_owned(),
			)
		}
		err => broken(err),
	}
}

fn site_json(site: &Site) -> Value {
	json!({ "domain": site.domain, "timezone": site.timezone })
}

fn goal_json(domain: &str, goal: &Goal) -> Value {
	json!({
		"domain": domain,
		"id": goal.id,
		"display_name": goal.name,
		"goal_type": if goal.page { "page" } else { "event" },
		"event_name": (!goal.page).then_some(&goal.value),
		"page_path": goal.page.then_some(&goal.value),
	})
}

async fn list(State(web): State<Arc<Web>>, call: Call) -> Response {
	reply((|| {
		let limit = call
			.params
			.get("limit")
			.and_then(|limit| limit.parse().ok())
			.unwrap_or(100usize)
			.clamp(1, 100);
		let mut sites = web.app.user_sites(call.user).map_err(broken)?;
		if let Some(after) = call.params.get("after") {
			sites.retain(|site| &site.domain > after);
		}
		if let Some(before) = call.params.get("before") {
			sites.retain(|site| &site.domain < before);
			sites.drain(..sites.len().saturating_sub(limit));
		}
		let more = sites.len() > limit;
		sites.truncate(limit);
		Ok(json!({
			"sites": sites.iter().map(site_json).collect::<Vec<_>>(),
			"meta": {
				"after": more.then(|| sites.last().map(|site| &site.domain)).flatten(),
				"before": call.params.get("after").and(sites.first().map(|site| &site.domain)),
				"limit": limit,
			},
		}))
	})())
}

async fn create(State(web): State<Arc<Web>>, call: Call) -> Response {
	reply((|| {
		let domain = clean_domain(call.param("domain")?).ok_or((
			StatusCode::BAD_REQUEST,
			"domain: Use a valid domain".to_owned(),
		))?;
		let timezone = call.param("timezone").unwrap_or("Etc/UTC");
		if TimeZone::get(timezone).is_err() {
			return Err((StatusCode::BAD_REQUEST, "timezone: is invalid".to_owned()));
		}
		let team = match call.param("team_id") {
			Ok(team) => team
				.parse()
				.map_err(|_| (StatusCode::BAD_REQUEST, "team_id: is invalid".to_owned()))?,
			Err(_) => web.app.personal_team(call.user).map_err(broken)?,
		};
		if !matches!(web.app.role(call.user, team), Ok(Some(role)) if role != "viewer") {
			return Err((
				StatusCode::FORBIDDEN,
				"You can't add sites to this team".to_owned(),
			));
		}
		let site = web
			.app
			.add_site(&domain, timezone, Some(team))
			.map_err(taken)?;
		Ok(site_json(&site))
	})())
}

async fn show(State(web): State<Arc<Web>>, Path(domain): Path<String>, call: Call) -> Response {
	reply((|| {
		let site = call.site(&web, &domain, 0)?;
		let mut value = site_json(&site);
		value["custom_properties"] = json!(
			props(&web, &site)?
				.into_iter()
				.map(|(_, prop)| prop)
				.collect::<Vec<_>>()
		);
		Ok(value)
	})())
}

async fn update(State(web): State<Arc<Web>>, Path(domain): Path<String>, call: Call) -> Response {
	reply((|| {
		let mut site = call.site(&web, &domain, 1)?;
		if let Ok(raw) = call.param("domain") {
			let domain = clean_domain(raw).ok_or((
				StatusCode::BAD_REQUEST,
				"domain: Use a valid domain".to_owned(),
			))?;
			if domain != site.domain {
				web.app.rename_site(site.id, &domain).map_err(taken)?;
				web.ingest.forget(&site.domain);
				site.domain = domain;
			}
		}
		Ok(site_json(&site))
	})())
}

async fn destroy(State(web): State<Arc<Web>>, Path(domain): Path<String>, call: Call) -> Response {
	let site = match call.site(&web, &domain, 2) {
		Ok(site) => site,
		Err(fail) => return reply(Err(fail)),
	};
	reply(match drop_site(&web, site).await {
		Ok(()) => Ok(json!({ "deleted": true })),
		Err(_) => Err(broken("drop site")),
	})
}

async fn goals(State(web): State<Arc<Web>>, call: Call) -> Response {
	reply((|| {
		let site = call.site(&web, call.param("site_id")?, 0)?;
		let goals = web.app.goals(site.id).map_err(broken)?;
		Ok(json!({
			"goals": goals.iter().map(|goal| goal_json(&site.domain, goal)).collect::<Vec<_>>(),
			"meta": { "after": null, "before": null, "limit": goals.len().max(100) },
		}))
	})())
}

async fn add_goal(State(web): State<Arc<Web>>, call: Call) -> Response {
	reply((|| {
		let site = call.site(&web, call.param("site_id")?, 1)?;
		let page = match call.param("goal_type")? {
			"page" => true,
			"event" => false,
			_ => {
				return Err((
					StatusCode::BAD_REQUEST,
					"goal_type: must be event or page".to_owned(),
				));
			}
		};
		let value = call.param(if page { "page_path" } else { "event_name" })?;
		if page && !value.starts_with('/') {
			return Err((
				StatusCode::BAD_REQUEST,
				"page_path: must start with /".to_owned(),
			));
		}
		let currency = if page {
			String::new()
		} else {
			call.param("currency").unwrap_or_default().to_uppercase()
		};
		let find = || -> Result<Option<Goal>, Fail> {
			Ok(web
				.app
				.goals(site.id)
				.map_err(broken)?
				.into_iter()
				.find(|goal| {
					goal.page == page
						&& goal.value == value
						&& goal.scroll == 0
						&& goal.currency == currency
				}))
		};
		if find()?.is_none() {
			let name = match call.param("display_name") {
				Ok(name) => name.to_owned(),
				Err(_) if page => format!("Visit {value}"),
				Err(_) => value.to_owned(),
			};
			web.app
				.add_goal(site.id, page, value, &name, 0, &currency)
				.map_err(|_| {
					(
						StatusCode::BAD_REQUEST,
						"display_name: has already been taken".to_owned(),
					)
				})?;
		}
		let goal = find()?.ok_or_else(|| broken("goal missing"))?;
		Ok(goal_json(&site.domain, &goal))
	})())
}

async fn remove_goal(State(web): State<Arc<Web>>, Path(id): Path<i64>, call: Call) -> Response {
	reply((|| {
		let site = call.site(&web, call.param("site_id")?, 1)?;
		web.app.remove_goal(site.id, id).map_err(broken)?;
		Ok(json!({ "deleted": true }))
	})())
}

async fn guests(State(web): State<Arc<Web>>, call: Call) -> Response {
	reply((|| {
		let site = call.site(&web, call.param("site_id")?, 2)?;
		let people = web.app.guests(site.id).map_err(broken)?;
		let entry = |person: &Value, status: &str| json!({ "email": person["email"], "role": person["role"], "status": status });
		let guests: Vec<Value> = people["guests"]
			.as_array()
			.into_iter()
			.flatten()
			.map(|guest| entry(guest, "accepted"))
			.chain(
				people["invites"]
					.as_array()
					.into_iter()
					.flatten()
					.map(|invite| entry(invite, "invited")),
			)
			.collect();
		Ok(json!({
			"guests": guests,
			"meta": { "after": null, "before": null, "limit": guests.len().max(100) },
		}))
	})())
}

async fn invite(State(web): State<Arc<Web>>, call: Call) -> Response {
	let result: Result<Value, Fail> = async {
		let site = call.site(&web, call.param("site_id")?, 2)?;
		let email = call.param("email")?;
		let role = call.param("role")?;
		if !matches!(role, "viewer" | "editor") || !email.contains('@') {
			return Err((
				StatusCode::BAD_REQUEST,
				"Use a valid email and the viewer or editor role".to_owned(),
			));
		}
		let people = web.app.guests(site.id).map_err(broken)?;
		for (list, status) in [("guests", "accepted"), ("invites", "invited")] {
			let known = people[list]
				.as_array()
				.into_iter()
				.flatten()
				.find(|person| {
					person["email"]
						.as_str()
						.is_some_and(|known| known.eq_ignore_ascii_case(email))
				});
			if let Some(person) = known {
				return Ok(json!({ "email": email, "role": person["role"], "status": status }));
			}
		}
		let team = web.app.site_team(site.id).map_err(broken)?;
		let token = web
			.app
			.invite(team, Some(site.id), email, role)
			.map_err(broken)?;
		teams::mail_invite(&web, call.user, &site.domain, email, &token)
			.await
			.map_err(broken)?;
		Ok(json!({ "email": email, "role": role, "status": "invited" }))
	}
	.await;
	reply(result)
}

async fn remove_guest(
	State(web): State<Arc<Web>>,
	Path(email): Path<String>,
	call: Call,
) -> Response {
	reply((|| {
		let site = call.site(&web, call.param("site_id")?, 2)?;
		web.app
			.remove_guest_email(site.id, &email)
			.map_err(broken)?;
		Ok(json!({ "deleted": true }))
	})())
}

async fn share(State(web): State<Arc<Web>>, call: Call) -> Response {
	reply((|| {
		let site = call.site(&web, call.param("site_id")?, 1)?;
		let name = call.param("name")?;
		let find = || -> Result<Option<String>, Fail> {
			Ok(web
				.app
				.shares(site.id)
				.map_err(broken)?
				.into_iter()
				.find(|link| link["name"] == name && link["slug"] != site.domain.as_str())
				.and_then(|link| link["slug"].as_str().map(str::to_owned)))
		};
		let slug = if let Some(slug) = find()? {
			slug
		} else {
			let slug = token().map_err(broken)?[..20].to_owned();
			web.app
				.add_share(site.id, &slug, name, "")
				.map_err(broken)?;
			slug
		};
		Ok(json!({
			"name": name,
			"url": format!("{}/share/{slug}", web.url.trim_end_matches('/')),
		}))
	})())
}

async fn add_prop(State(web): State<Arc<Web>>, call: Call) -> Response {
	reply((|| {
		let site = call.site(&web, call.param("site_id")?, 1)?;
		let prop = call.param("property")?;
		if prop.len() > 300 {
			return Err((StatusCode::BAD_REQUEST, "property: is too long".to_owned()));
		}
		web.app.add_shield(site.id, "prop", prop).map_err(broken)?;
		web.ingest.forget(&site.domain);
		Ok(json!({ "created": true }))
	})())
}

async fn remove_prop(
	State(web): State<Arc<Web>>,
	Path(prop): Path<String>,
	call: Call,
) -> Response {
	reply((|| {
		let site = call.site(&web, call.param("site_id")?, 1)?;
		for (id, _) in props(&web, &site)?
			.into_iter()
			.filter(|(_, kept)| *kept == prop)
		{
			web.app.remove_shield(site.id, id).map_err(broken)?;
		}
		web.ingest.forget(&site.domain);
		Ok(json!({ "deleted": true }))
	})())
}

fn props(web: &Web, site: &Site) -> Result<Vec<(i64, String)>, Fail> {
	Ok(web
		.app
		.shields(site.id)
		.map_err(broken)?
		.into_iter()
		.filter(|(_, kind, _)| kind == "prop")
		.map(|(id, _, prop)| (id, prop))
		.collect())
}

impl App {
	pub(super) fn user_sites(&self, user: i64) -> rusqlite::Result<Vec<Site>> {
		let conn = self.conn();
		let mut stmt = conn.prepare(
			"select id, domain, timezone, created_at from sites
			where (?1 = 0 and domain not like '~%') or team in (select team from members where user = ?1)
			or id in (select site from guests where user = ?1)
			order by domain",
		)?;
		stmt.query_map([user], |row| {
			Ok(Site {
				id: row.get::<_, i64>(0)? as u64,
				domain: row.get(1)?,
				timezone: row.get(2)?,
				created_at: row.get(3)?,
			})
		})?
		.collect()
	}

	fn remove_guest_email(&self, site: u64, email: &str) -> rusqlite::Result<()> {
		let conn = self.conn();
		conn.execute(
			"delete from guests where site = ?1 and user in (select id from users where email = ?2 collate nocase)",
			params![site as i64, email],
		)?;
		conn.execute(
			"delete from invites where site = ?1 and email = ?2 collate nocase",
			params![site as i64, email],
		)?;
		Ok(())
	}
}
