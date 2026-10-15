use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{FromRequest, Multipart, Request};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::{Json, Router};
use serde_json::Value;

use crate::accounts::Web;
use crate::app::Site;
use crate::stats::{error, unauthorized};

mod goatcounter;
mod sites;
mod stats;

type Fail = (StatusCode, String);

pub fn routes() -> Router<Arc<Web>> {
	stats::routes()
		.merge(sites::routes())
		.merge(goatcounter::routes())
}

// the key's user, with params merged from the query and a json, form or multipart body
struct Call {
	host: String,
	user: i64,
	params: HashMap<String, String>,
}

impl FromRequest<Arc<Web>> for Call {
	type Rejection = Response;

	async fn from_request(request: Request, web: &Arc<Web>) -> Result<Self, Response> {
		let owner = request
			.headers()
			.get(header::AUTHORIZATION)
			.and_then(|value| value.to_str().ok())
			.and_then(|value| value.strip_prefix("Bearer "))
			.and_then(|key| web.app.key_owner(key).ok().flatten());
		let user = match owner {
			Some(Some(user)) => user,
			Some(None) => web
				.app
				.first_user()
				.ok()
				.flatten()
				.ok_or_else(unauthorized)?,
			None => return Err(unauthorized()),
		};
		let host = request
			.headers()
			.get(header::HOST)
			.and_then(|value| value.to_str().ok())
			.and_then(|value| value.split(':').next())
			.unwrap_or_default()
			.to_owned();
		let mut params: HashMap<String, String> = request
			.uri()
			.query()
			.map(|query| {
				url::form_urlencoded::parse(query.as_bytes())
					.into_owned()
					.collect()
			})
			.unwrap_or_default();
		let kind = request
			.headers()
			.get(header::CONTENT_TYPE)
			.and_then(|value| value.to_str().ok())
			.unwrap_or_default()
			.to_owned();
		if kind.starts_with("multipart/form-data") {
			let mut form = Multipart::from_request(request, web)
				.await
				.map_err(IntoResponse::into_response)?;
			while let Some(field) = form
				.next_field()
				.await
				.map_err(IntoResponse::into_response)?
			{
				let name = field.name().unwrap_or_default().to_owned();
				params.insert(
					name,
					field.text().await.map_err(IntoResponse::into_response)?,
				);
			}
		} else {
			let body = axum::body::to_bytes(request.into_body(), 1 << 16)
				.await
				.map_err(|_| error(StatusCode::PAYLOAD_TOO_LARGE, "Body too large"))?;
			match kind.starts_with("application/json") {
				true => {
					if let Ok(Value::Object(fields)) = serde_json::from_slice(&body) {
						params.extend(fields.into_iter().map(|(name, value)| match value {
							Value::String(text) => (name, text),
							value => (name, value.to_string()),
						}));
					}
				}
				false => params.extend(url::form_urlencoded::parse(&body).into_owned()),
			}
		}
		Ok(Call { host, user, params })
	}
}

impl Call {
	fn param(&self, name: &str) -> Result<&str, Fail> {
		self.params
			.get(name)
			.map(|value| value.trim())
			.filter(|value| !value.is_empty())
			.ok_or_else(|| {
				(
					StatusCode::BAD_REQUEST,
					format!("Parameter `{name}` is required"),
				)
			})
	}

	// level 0 reads, 1 edits, 2 manages
	fn site(&self, web: &Web, domain: &str, level: u8) -> Result<Site, Fail> {
		let missing = || (StatusCode::NOT_FOUND, "Site could not be found".to_owned());
		let rank = match web
			.app
			.site_role(self.user, domain)
			.map_err(broken)?
			.as_deref()
		{
			Some("owner" | "admin") => 2,
			Some("editor") => 1,
			Some(_) => 0,
			None => return Err(missing()),
		};
		if rank < level {
			return Err((
				StatusCode::FORBIDDEN,
				"Your role can't do this on this site".to_owned(),
			));
		}
		web.app.site(domain).map_err(broken)?.ok_or_else(missing)
	}
}

fn reply(result: Result<Value, Fail>) -> Response {
	match result {
		Ok(value) => Json(value).into_response(),
		Err((status, message)) => error(status, &message),
	}
}

fn broken(err: impl std::fmt::Display) -> Fail {
	tracing::warn!(%err, "sites api");
	(
		StatusCode::INTERNAL_SERVER_ERROR,
		"Something went wrong".to_owned(),
	)
}
