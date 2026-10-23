use std::collections::BTreeMap;
use std::sync::Arc;

use axum::Json;
use serde_json::{Value, json};

use crate::app::now;
use crate::uptime::api::{DAY, Fields, add, discard, edit, hold, member, overview, owned, summary};
use crate::uptime::calc::{Spans, round, uptime};
use crate::uptime::store::Scope;
use crate::uptime::{Config, Digest, Monitor, State as Health};
use crate::web::{Reply, Web, blocking};

fn shape(mut item: Value, team: i64) -> Value {
	if let Some(fields) = item.as_object_mut() {
		fields.remove("strip");
		fields.insert("team".to_owned(), json!(team));
	}
	item
}

pub async fn list(web: &Arc<Web>, user: i64, team: Option<i64>) -> Reply {
	let team = match team {
		Some(team) => team,
		None => web
			.app
			.personal_team(user)
			.map_err(|_| crate::web::Error::bad("Choose a team."))?,
	};
	let role = member(web, user, team)?;
	let items = overview(web, team, role != "viewer", now()).await?;
	let monitors: Vec<Value> = items.into_iter().map(|item| shape(item, team)).collect();
	Ok(Json(json!({ "monitors": monitors })))
}

pub async fn show(web: &Arc<Web>, user: i64, id: i64) -> Reply {
	let (monitor, role) = owned(web, user, id)?;
	Ok(Json(one(web, &monitor, role != "viewer").await?))
}

async fn one(web: &Arc<Web>, monitor: &Monitor, editable: bool) -> Reply<Value> {
	let (id, at) = (monitor.id, now());
	let spans = web
		.app
		.spans(&Scope::Monitor(id), at - 365 * DAY)?
		.remove(&id)
		.unwrap_or_default();
	let reader = web.clone();
	let digest = blocking(move || Ok(reader.uptime.digest(id, at))).await?;
	let site = monitor.site.and_then(|site| web.app.monitor_domain(site));
	let mut item = shape(
		summary(web, monitor, site.as_deref(), &spans, &digest, editable, at),
		monitor.team,
	);
	if monitor.paused {
		item["state"] = json!(Health::Paused);
	}
	item["config"] = if editable {
		json!(monitor.config)
	} else {
		json!(Config {
			headers: BTreeMap::new(),
			..monitor.config.clone()
		})
	};
	item["uptime_1y"] = json!(year(&spans, monitor, &digest, at));
	item["created_at"] = json!(monitor.created_at);
	Ok(item)
}

fn year(spans: &Spans, monitor: &Monitor, digest: &Digest, at: i64) -> Option<f64> {
	uptime(
		spans,
		digest.watched_from(monitor.created_at),
		at - 365 * DAY,
		at,
	)
	.map(round)
}

pub async fn create(web: &Arc<Web>, user: i64, mut fields: Fields) -> Reply<Value> {
	if fields.team.is_none() {
		fields.team = web.app.personal_team(user).ok();
	}
	let created = add(web, user, fields)?;
	one(web, &created, true).await
}

pub async fn update(web: &Arc<Web>, user: i64, id: i64, fields: Fields) -> Reply {
	edit(web, user, id, fields)?;
	show(web, user, id).await
}

pub fn remove(web: &Web, user: i64, id: i64) -> Reply<()> {
	discard(web, user, id)
}

pub async fn pause(web: &Arc<Web>, user: i64, id: i64, paused: bool) -> Reply {
	hold(web, user, id, paused)?;
	show(web, user, id).await
}

pub fn incidents(web: &Web, user: i64, id: i64, days: Option<i64>) -> Reply {
	let (_, role) = owned(web, user, id)?;
	let at = now();
	let found = web
		.app
		.monitor_incidents(id, at - days.unwrap_or(30).clamp(1, 365) * DAY)?;
	let notes = web
		.app
		.notes(&found.iter().map(|item| item.0).collect::<Vec<_>>())?;
	let items: Vec<Value> = found
		.into_iter()
		.map(|(incident, started, ended, cause, detail)| {
			json!({
				"id": incident,
				"started_at": started,
				"ended_at": ended,
				"cause": cause,
				"detail": if role == "viewer" { String::new() } else { detail },
				"duration": ended.unwrap_or(at) - started,
				"notes": notes.get(&incident).cloned().unwrap_or_default(),
			})
		})
		.collect();
	Ok(Json(json!({ "incidents": items })))
}
