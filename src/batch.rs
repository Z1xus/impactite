use axum::Json;
use axum::Router;
use axum::body::{Body, to_bytes};
use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderValue, Method, header};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tower::ServiceExt;

const LIMIT: usize = 1 << 22;

#[derive(Deserialize)]
pub struct Call {
	method: String,
	path: String,
	body: Option<Value>,
}

#[derive(Serialize)]
pub struct Answer {
	status: u16,
	body: Value,
}

// runs several app calls from one request, each as if it came in alone
pub async fn batch(
	State(app): State<Router>,
	headers: HeaderMap,
	Json(calls): Json<Vec<Call>>,
) -> Json<Vec<Answer>> {
	let tasks: Vec<_> = calls
		.into_iter()
		.map(|call| tokio::spawn(run(app.clone(), headers.clone(), call)))
		.collect();
	let mut answers = Vec::with_capacity(tasks.len());
	for task in tasks {
		answers.push(task.await.unwrap_or(Answer {
			status: 500,
			body: Value::Null,
		}));
	}
	Json(answers)
}

async fn run(app: Router, headers: HeaderMap, call: Call) -> Answer {
	let refused = |status| Answer {
		status,
		body: Value::Null,
	};
	let Ok(method) = call.method.parse::<Method>() else {
		return refused(400);
	};
	if !call.path.starts_with("/api/app/") {
		return refused(404);
	}
	let body = call.body.map(|body| body.to_string()).unwrap_or_default();
	let Ok(mut request) = Request::builder()
		.method(method)
		.uri(&call.path)
		.body(Body::from(body))
	else {
		return refused(400);
	};
	for name in [
		header::COOKIE,
		header::HOST,
		header::HeaderName::from_static("x-share"),
	] {
		if let Some(value) = headers.get(&name) {
			request.headers_mut().insert(name, value.clone());
		}
	}
	request.headers_mut().insert(
		header::CONTENT_TYPE,
		HeaderValue::from_static("application/json"),
	);
	let Ok(response) = app.oneshot(request).await;
	let status = response.status().as_u16();
	let bytes = to_bytes(response.into_body(), LIMIT)
		.await
		.unwrap_or_default();
	Answer {
		status,
		body: serde_json::from_slice(&bytes).unwrap_or(Value::Null),
	}
}
