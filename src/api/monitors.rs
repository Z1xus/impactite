use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;

use super::Key;
use crate::uptime::{Fields, public};
use crate::web::{Reply, Web};

pub fn routes() -> Router<Arc<Web>> {
	Router::new()
		.route("/api/v1/monitors", get(list).post(create))
		.route(
			"/api/v1/monitors/{id}",
			get(show).patch(update).delete(remove),
		)
		.route("/api/v1/monitors/{id}/pause", post(pause))
		.route("/api/v1/monitors/{id}/resume", post(resume))
		.route("/api/v1/monitors/{id}/incidents", get(incidents))
}

#[derive(Deserialize)]
struct Team {
	team: Option<i64>,
}

#[derive(Deserialize)]
struct Days {
	days: Option<i64>,
}

async fn list(State(web): State<Arc<Web>>, Key(user): Key, Query(query): Query<Team>) -> Reply {
	public::list(&web, user, query.team).await
}

async fn create(
	State(web): State<Arc<Web>>,
	Key(user): Key,
	Json(fields): Json<Fields>,
) -> Reply<Response> {
	let created = public::create(&web, user, fields).await?;
	Ok((StatusCode::CREATED, Json(created)).into_response())
}

async fn show(State(web): State<Arc<Web>>, Key(user): Key, Path(id): Path<i64>) -> Reply {
	public::show(&web, user, id).await
}

async fn update(
	State(web): State<Arc<Web>>,
	Key(user): Key,
	Path(id): Path<i64>,
	Json(fields): Json<Fields>,
) -> Reply {
	public::update(&web, user, id, fields).await
}

async fn remove(
	State(web): State<Arc<Web>>,
	Key(user): Key,
	Path(id): Path<i64>,
) -> Reply<StatusCode> {
	public::remove(&web, user, id)?;
	Ok(StatusCode::NO_CONTENT)
}

async fn pause(State(web): State<Arc<Web>>, Key(user): Key, Path(id): Path<i64>) -> Reply {
	public::pause(&web, user, id, true).await
}

async fn resume(State(web): State<Arc<Web>>, Key(user): Key, Path(id): Path<i64>) -> Reply {
	public::pause(&web, user, id, false).await
}

async fn incidents(
	State(web): State<Arc<Web>>,
	Key(user): Key,
	Path(id): Path<i64>,
	Query(query): Query<Days>,
) -> Reply {
	public::incidents(&web, user, id, query.days)
}
