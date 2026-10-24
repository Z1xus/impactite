use std::sync::Arc;

use axum::extract::{Path, State};
use axum::routing::{delete, get};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use crate::auth::accounts::Viewer;
use crate::web::{Error, Reply, Web};

pub fn routes() -> Router<Arc<Web>> {
	Router::new()
		.route("/api/app/keys", get(list).post(add))
		.route("/api/app/keys/{id}", delete(remove))
}

#[derive(Deserialize)]
struct NewKey {
	name: String,
}

async fn list(State(web): State<Arc<Web>>, viewer: Viewer) -> Reply {
	respond(&web, viewer.user, None)
}

async fn add(State(web): State<Arc<Web>>, viewer: Viewer, Json(body): Json<NewKey>) -> Reply {
	let name = body.name.trim();
	if name.is_empty() || name.len() > 100 {
		return Err(Error::bad("Enter a name for this key."));
	}
	let key = web.app.create_key(name, Some(viewer.user))?;
	respond(&web, viewer.user, Some(key))
}

async fn remove(State(web): State<Arc<Web>>, viewer: Viewer, Path(id): Path<i64>) -> Reply {
	web.app.remove_key(viewer.user, id)?;
	respond(&web, viewer.user, None)
}

fn respond(web: &Web, user: i64, key: Option<String>) -> Reply {
	let keys = web.app.keys(user)?;
	Ok(Json(json!({
		"key": key,
		"keys": keys
			.iter()
			.map(|(id, name, prefix, created)| json!({ "id": id, "name": name, "prefix": prefix, "created": created }))
			.collect::<Vec<_>>()
	})))
}
