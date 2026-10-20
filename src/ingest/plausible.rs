use std::net::SocketAddr;
use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{ConnectInfo, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{Client, Hit, Ingest};
use crate::{net, stats};

#[derive(Deserialize)]
struct Payload {
	#[serde(alias = "name")]
	n: Option<Value>,
	#[serde(alias = "url")]
	u: Option<String>,
	#[serde(alias = "domain")]
	d: Option<String>,
	#[serde(alias = "referrer")]
	r: Option<String>,
	#[serde(alias = "props")]
	p: Option<Value>,
	#[serde(alias = "meta")]
	m: Option<Value>,
	#[serde(alias = "interactive")]
	i: Option<bool>,
	#[serde(alias = "hashMode")]
	h: Option<Value>,
	sd: Option<Value>,
	e: Option<Value>,
	t: Option<String>,
	#[serde(rename = "$", alias = "revenue")]
	revenue: Option<Value>,
}

pub async fn event(
	State(ingest): State<Arc<Ingest>>,
	ConnectInfo(peer): ConnectInfo<SocketAddr>,
	headers: HeaderMap,
	body: Bytes,
) -> Response {
	let Ok(payload) = serde_json::from_slice::<Payload>(&body) else {
		return invalid("request", "Unable to parse request body as json");
	};

	let name = match &payload.n {
		Some(Value::String(name)) => name.clone(),
		Some(Value::Number(name)) => name.to_string(),
		_ => return invalid("name", "can't be blank"),
	};
	if name.chars().count() > 120 {
		return invalid("name", "should be at most 120 character(s)");
	}
	let Some(raw_url) = payload.u.filter(|url| !url.is_empty()) else {
		return invalid("url", "is required");
	};
	if raw_url.len() > 2000 {
		return invalid("url", "should be at most 2000 byte(s)");
	}
	let Ok(url) = url::Url::parse(&raw_url) else {
		return invalid("url", "is invalid");
	};
	if url.scheme() == "data" {
		return invalid("url", "is invalid");
	}

	let props = match props(payload.p.or(payload.m)) {
		Ok(props) => props,
		Err(field) => return invalid("props", field),
	};
	let hostname = url.host_str().unwrap_or("(none)");
	let hostname = hostname.strip_prefix("www.").unwrap_or(hostname).to_owned();
	let mut path = url.path().to_owned();
	if number(payload.h.as_ref()) == Some(1)
		&& let Some(fragment) = url.fragment()
	{
		path = format!("{path}#{fragment}");
	}
	let revenue = match revenue(payload.revenue.as_ref()) {
		Ok(revenue) => revenue,
		Err(message) => return invalid("revenue", message),
	};
	let engagement = name == "engagement";
	let hit = || Hit {
		name: name.clone(),
		hostname: hostname.clone(),
		path: path.clone(),
		params: url.query_pairs().into_owned().collect(),
		referrer: payload.r.clone().unwrap_or_default(),
		props: props.clone(),
		interactive: engagement || payload.i.unwrap_or(true),
		scroll_depth: engagement
			.then(|| number(payload.sd.as_ref()).map(|depth| depth.min(100) as u8))
			.flatten(),
		engagement_ms: engagement
			.then(|| number(payload.e.as_ref()).filter(|&ms| ms < 2_592_000_000))
			.flatten()
			.unwrap_or_default() as u32,
		revenue: revenue.clone(),
		at: None,
		title: payload.t.clone().unwrap_or_default(),
	};

	let ua = headers
		.get(header::USER_AGENT)
		.and_then(|ua| ua.to_str().ok())
		.unwrap_or_default();
	let visitor = ingest.proxies.visitor(peer, &headers);
	let client = Client {
		ip: visitor.ip,
		ua,
		country: visitor.country,
		language: super::language(&headers),
	};
	// a proxy only sees the sender, so a keyed sender says which visitor it reports
	let reported = headers
		.contains_key(header::AUTHORIZATION)
		.then(|| net::reported(&headers))
		.flatten()
		.map(|ip| Client {
			ip,
			ua,
			country: None,
			language: super::language(&headers),
		});
	let domains = payload.d.unwrap_or_else(|| hostname.clone());
	let mut dropped = 0;
	for domain in domains
		.split(',')
		.map(str::trim)
		.filter(|d| !d.is_empty())
		.take(10)
	{
		let tracked = if let Some(site) = ingest.site(domain) {
			let keyed = reported
				.as_ref()
				.filter(|_| stats::keyed(&ingest.app, &headers, domain));
			ingest.track(&site, hit(), keyed.unwrap_or(&client))
		} else {
			ingest.stray(domain);
			Err(super::Rejected::Failed)
		};
		dropped += usize::from(tracked.is_err());
	}

	let mut response = (StatusCode::ACCEPTED, "ok").into_response();
	if dropped > 0
		&& let Ok(value) = dropped.to_string().parse()
	{
		response.headers_mut().insert("x-plausible-dropped", value);
	}
	response
}

fn props(raw: Option<Value>) -> Result<Vec<(String, String)>, &'static str> {
	let value = match raw {
		Some(Value::String(text)) => serde_json::from_str(&text).unwrap_or(Value::Null),
		Some(value) => value,
		None => Value::Null,
	};
	let Value::Object(map) = value else {
		return Ok(Vec::new());
	};
	let mut props = Vec::new();
	for (key, value) in map {
		let value = match value {
			Value::String(value) => value,
			Value::Number(value) => value.to_string(),
			Value::Bool(value) => value.to_string(),
			_ => continue,
		};
		if key.is_empty() || value.is_empty() || props.len() == 30 {
			continue;
		}
		if key.len() > 300 || value.len() > 2000 {
			return Err("keys should have at most 300 bytes and values 2000 bytes");
		}
		props.push((key, value));
	}
	Ok(props)
}

// amount in hundredths of a three letter currency
fn revenue(value: Option<&Value>) -> Result<Option<(u32, String)>, &'static str> {
	let Some(value) = value.filter(|value| !value.is_null()) else {
		return Ok(None);
	};
	let amount = match &value["amount"] {
		Value::Number(number) => number.as_f64(),
		Value::String(text) => text.parse().ok(),
		_ => None,
	}
	.filter(|amount: &f64| amount.is_finite() && *amount >= 0.0)
	.ok_or("amount must be a positive number")?;
	let currency = value["currency"]
		.as_str()
		.filter(|code| code.len() == 3 && code.chars().all(|c| c.is_ascii_alphabetic()))
		.ok_or("currency must be a 3 letter code")?;
	Ok(Some((
		(amount * 100.0).round().min(f64::from(u32::MAX)) as u32,
		currency.to_uppercase(),
	)))
}

fn number(value: Option<&Value>) -> Option<u64> {
	match value? {
		Value::Number(number) => number.as_u64(),
		Value::String(text) => text.parse().ok(),
		_ => None,
	}
}

fn invalid(field: &str, message: &str) -> Response {
	let body = json!({ "errors": { field: [message] } });
	(StatusCode::BAD_REQUEST, axum::Json(body)).into_response()
}
