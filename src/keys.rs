use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use crate::accounts::{Viewer, Web, fail};

pub fn routes() -> Router<Arc<Web>> {
	Router::new()
		.route("/api/app/keys", get(list).post(add))
		.route("/api/app/keys/{id}", delete(remove))
}

#[derive(Deserialize)]
struct NewKey {
	name: String,
}

async fn list(State(web): State<Arc<Web>>, viewer: Viewer) -> Response {
	respond(&web, viewer.user, None)
}

async fn add(State(web): State<Arc<Web>>, viewer: Viewer, Json(body): Json<NewKey>) -> Response {
	let name = body.name.trim();
	if name.is_empty() || name.len() > 100 {
		return fail(StatusCode::BAD_REQUEST, "Name the key.");
	}
	match web.app.create_key(name, Some(viewer.user)) {
		Ok(key) => respond(&web, viewer.user, Some(key)),
		Err(_) => fail(StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong."),
	}
}

async fn remove(State(web): State<Arc<Web>>, viewer: Viewer, Path(id): Path<i64>) -> Response {
	match web.app.remove_key(viewer.user, id) {
		Ok(()) => respond(&web, viewer.user, None),
		Err(_) => fail(StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong."),
	}
}

fn respond(web: &Web, user: i64, key: Option<String>) -> Response {
	match web.app.keys(user) {
		Ok(keys) => Json(json!({
			"key": key,
			"keys": keys
				.iter()
				.map(|(id, name, prefix, created)| json!({ "id": id, "name": name, "prefix": prefix, "created": created }))
				.collect::<Vec<_>>()
		}))
		.into_response(),
		Err(_) => fail(StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong."),
	}
}
