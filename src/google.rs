use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex, PoisonError};

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use rusqlite::{OptionalExtension, params};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::accounts::{Viewer, Web, fail, internal, now};
use crate::app::{App, Site};
use crate::passkeys::Purpose;
use crate::shares::Reader;
use crate::stats::period;

// search console terms for the google source, through the site owner's google account
const SCOPE: &str = "https://www.googleapis.com/auth/webmasters.readonly";
const TOKEN: &str = "https://oauth2.googleapis.com/token";
const API: &str = "https://www.googleapis.com/webmasters/v3/sites";

pub struct Client {
	pub id: String,
	pub secret: String,
}

static ACCESS: LazyLock<Mutex<HashMap<u64, (String, i64)>>> = LazyLock::new(Mutex::default);
// google analytics 4 imports: an hour's access token per site, and what a running import says
const ANALYTICS: &str = "https://www.googleapis.com/auth/analytics.readonly";
static ANALYTICS_ACCESS: LazyLock<Mutex<HashMap<u64, (String, i64)>>> =
	LazyLock::new(Mutex::default);
static IMPORTS: LazyLock<Mutex<HashMap<u64, Option<String>>>> = LazyLock::new(Mutex::default);

pub fn routes() -> Router<Arc<Web>> {
	Router::new()
		.route(
			"/api/app/sites/{domain}/google",
			get(show).patch(choose).delete(disconnect),
		)
		.route("/api/app/sites/{domain}/google/connect", get(connect))
		.route("/api/app/google/callback", get(callback))
		.route(
			"/api/app/sites/{domain}/analytics",
			get(analytics).post(start_import),
		)
		.route(
			"/api/app/sites/{domain}/analytics/connect",
			get(connect_analytics),
		)
		.route("/api/app/keywords", post(keywords))
}

#[derive(Deserialize)]
struct Choice {
	property: String,
}

#[derive(Deserialize)]
struct Callback {
	state: String,
	code: Option<String>,
	error: Option<String>,
}

#[derive(Deserialize)]
struct Ask {
	site_id: String,
	date_range: Value,
	#[serde(default)]
	page: Option<String>,
}

fn editable(web: &Web, user: i64, domain: &str) -> Option<Site> {
	let role = web.app.site_role(user, domain).ok().flatten()?;
	(role != "viewer").then_some(())?;
	web.app.site(domain).ok().flatten()
}

fn redirect_uri(web: &Web) -> String {
	format!("{}/api/app/google/callback", web.url.trim_end_matches('/'))
}

async fn show(State(web): State<Arc<Web>>, viewer: Viewer, Path(domain): Path<String>) -> Response {
	let Some(site) = editable(&web, viewer.user, &domain) else {
		return fail(StatusCode::FORBIDDEN, "You can't change this site.");
	};
	let link = match web.app.google(site.id) {
		Ok(link) => link,
		Err(err) => return internal(err),
	};
	let properties = match &link {
		Some(_) => match properties(&web, site.id).await {
			Ok(properties) => properties,
			Err(err) => {
				tracing::warn!(%err, "search console sites");
				Vec::new()
			}
		},
		None => Vec::new(),
	};
	Json(json!({
		"configured": web.google.is_some(),
		"redirect": redirect_uri(&web),
		"connected": link.is_some(),
		"property": link.map(|(_, property)| property).unwrap_or_default(),
		"properties": properties,
	}))
	.into_response()
}

async fn connect(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path(domain): Path<String>,
) -> Response {
	let (Some(site), Some(client)) = (editable(&web, viewer.user, &domain), &web.google) else {
		return fail(
			StatusCode::FORBIDDEN,
			"Search Console is not set up for this site.",
		);
	};
	let Some(state) = web.begin(Purpose::Google(site.id)) else {
		return fail(StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong.");
	};
	let query = url::form_urlencoded::Serializer::new(String::new())
		.append_pair("client_id", &client.id)
		.append_pair("redirect_uri", &redirect_uri(&web))
		.append_pair("response_type", "code")
		.append_pair("scope", SCOPE)
		.append_pair("access_type", "offline")
		.append_pair("prompt", "consent")
		.append_pair("state", &state)
		.finish();
	Redirect::to(&format!(
		"https://accounts.google.com/o/oauth2/v2/auth?{query}"
	))
	.into_response()
}

async fn callback(State(web): State<Arc<Web>>, Query(query): Query<Callback>) -> Response {
	let site = match web.take(&query.state) {
		Some(Purpose::Google(site)) => site,
		Some(Purpose::Analytics(site)) => return analytics_callback(&web, site, query).await,
		_ => return fail(StatusCode::BAD_REQUEST, "This link expired. Connect again."),
	};
	let Ok(Some(domain)) = web.app.site_domain(site) else {
		return fail(StatusCode::NOT_FOUND, "Site not found.");
	};
	let back = format!("/{domain}/settings#search-console");
	let failed = format!("/{domain}/settings?google=failed#search-console");
	let (Some(code), None, Some(client)) = (query.code, query.error, &web.google) else {
		return Redirect::to(&back).into_response();
	};
	let exchanged = web
		.http
		.post(TOKEN)
		.form(&[
			("code", code.as_str()),
			("client_id", &client.id),
			("client_secret", &client.secret),
			("redirect_uri", &redirect_uri(&web)),
			("grant_type", "authorization_code"),
		])
		.send()
		.await
		.and_then(reqwest::Response::error_for_status);
	let body: Value = match exchanged {
		Ok(response) => response.json().await.unwrap_or_default(),
		Err(err) => {
			tracing::warn!(%err, "search console token");
			return Redirect::to(&failed).into_response();
		}
	};
	let Some(refresh) = body["refresh_token"].as_str() else {
		return Redirect::to(&failed).into_response();
	};
	if let Some(access) = body["access_token"].as_str() {
		let expires = now() + body["expires_in"].as_i64().unwrap_or(0) - 60;
		ACCESS
			.lock()
			.unwrap_or_else(PoisonError::into_inner)
			.insert(site, (access.to_owned(), expires));
	}
	match web.app.link_google(site, refresh) {
		Ok(()) => Redirect::to(&back).into_response(),
		Err(err) => internal(err),
	}
}

async fn choose(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path(domain): Path<String>,
	Json(body): Json<Choice>,
) -> Response {
	let Some(site) = editable(&web, viewer.user, &domain) else {
		return fail(StatusCode::FORBIDDEN, "You can't change this site.");
	};
	match web.app.set_google_property(site.id, &body.property) {
		Ok(()) => StatusCode::NO_CONTENT.into_response(),
		Err(err) => internal(err),
	}
}

async fn disconnect(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path(domain): Path<String>,
) -> Response {
	let Some(site) = editable(&web, viewer.user, &domain) else {
		return fail(StatusCode::FORBIDDEN, "You can't change this site.");
	};
	if let Ok(Some((refresh, _))) = web.app.google(site.id) {
		// revoking is best effort, the link goes away either way
		let _ = web
			.http
			.post("https://oauth2.googleapis.com/revoke")
			.form(&[("token", refresh)])
			.send()
			.await;
	}
	ACCESS
		.lock()
		.unwrap_or_else(PoisonError::into_inner)
		.remove(&site.id);
	match web.app.unlink_google(site.id) {
		Ok(()) => StatusCode::NO_CONTENT.into_response(),
		Err(err) => internal(err),
	}
}

async fn keywords(State(web): State<Arc<Web>>, reader: Reader, Json(body): Json<Ask>) -> Response {
	if !reader.reads(&web, &body.site_id) {
		return fail(StatusCode::NOT_FOUND, "Site not found.");
	}
	let range = match period(&web.stats, &body.site_id, &body.date_range, true) {
		Ok(range) => range,
		Err(message) => return fail(StatusCode::BAD_REQUEST, &message),
	};
	let property = match web.app.google(range.site.id) {
		Ok(Some((_, property))) if !property.is_empty() => property,
		Ok(_) => return fail(StatusCode::NOT_FOUND, "Search Console is not connected."),
		Err(err) => return internal(err),
	};
	let mut query = json!({
		"startDate": range.from.to_string(),
		"endDate": range.to.to_string(),
		"dimensions": ["query"],
		"rowLimit": 100,
	});
	if let Some(page) = &body.page {
		let base = match property.strip_prefix("sc-domain:") {
			Some(domain) => format!("https://{domain}"),
			None => property.trim_end_matches('/').to_owned(),
		};
		query["dimensionFilterGroups"] = json!([{ "filters": [{
			"dimension": "page", "operator": "contains", "expression": format!("{base}{page}"),
		}] }]);
	}
	let url = format!("{API}/{}/searchAnalytics/query", urlencode(&property));
	match google(&web, range.site.id, |http, token| {
		http.post(&url).bearer_auth(token).json(&query)
	})
	.await
	{
		Ok(result) => Json(json!({
			"keywords": result["rows"].as_array().into_iter().flatten().map(|row| json!({
				"name": row["keys"][0],
				"visitors": row["clicks"],
				"impressions": row["impressions"],
				"ctr": row["ctr"],
				"position": row["position"],
			})).collect::<Vec<_>>()
		}))
		.into_response(),
		Err(err) => {
			tracing::warn!(%err, "search console query");
			fail(StatusCode::BAD_GATEWAY, "Search Console did not answer.")
		}
	}
}

fn urlencode(text: &str) -> String {
	url::form_urlencoded::byte_serialize(text.as_bytes()).collect()
}

async fn properties(web: &Web, site: u64) -> anyhow::Result<Vec<String>> {
	let result = google(web, site, |http, token| http.get(API).bearer_auth(token)).await?;
	Ok(result["siteEntry"]
		.as_array()
		.into_iter()
		.flatten()
		.filter_map(|entry| entry["siteUrl"].as_str().map(str::to_owned))
		.collect())
}

// calls google with a fresh access token from the stored refresh token
async fn google(
	web: &Web,
	site: u64,
	request: impl Fn(&reqwest::Client, &str) -> reqwest::RequestBuilder,
) -> anyhow::Result<Value> {
	let cached = ACCESS
		.lock()
		.unwrap_or_else(PoisonError::into_inner)
		.get(&site)
		.filter(|(_, expires)| *expires > now())
		.map(|(token, _)| token.clone());
	let token = match cached {
		Some(token) => token,
		None => {
			let client = web
				.google
				.as_ref()
				.ok_or_else(|| anyhow::anyhow!("not configured"))?;
			let (refresh, _) = web
				.app
				.google(site)?
				.ok_or_else(|| anyhow::anyhow!("not connected"))?;
			let body: Value = web
				.http
				.post(TOKEN)
				.form(&[
					("client_id", client.id.as_str()),
					("client_secret", &client.secret),
					("refresh_token", &refresh),
					("grant_type", "refresh_token"),
				])
				.send()
				.await?
				.error_for_status()?
				.json()
				.await?;
			let token = body["access_token"]
				.as_str()
				.ok_or_else(|| anyhow::anyhow!("no access token"))?
				.to_owned();
			let expires = now() + body["expires_in"].as_i64().unwrap_or(0) - 60;
			ACCESS
				.lock()
				.unwrap_or_else(PoisonError::into_inner)
				.insert(site, (token.clone(), expires));
			token
		}
	};
	Ok(request(&web.http, &token)
		.send()
		.await?
		.error_for_status()?
		.json()
		.await?)
}

impl App {
	fn google(&self, site: u64) -> rusqlite::Result<Option<(String, String)>> {
		self.conn()
			.query_row(
				"select refresh, property from google where site = ?1",
				[site as i64],
				|row| Ok((row.get(0)?, row.get(1)?)),
			)
			.optional()
	}

	fn link_google(&self, site: u64, refresh: &str) -> rusqlite::Result<()> {
		self.conn().execute(
			"insert into google (site, refresh) values (?1, ?2)
			on conflict (site) do update set refresh = excluded.refresh",
			params![site as i64, refresh],
		)?;
		Ok(())
	}

	fn set_google_property(&self, site: u64, property: &str) -> rusqlite::Result<()> {
		self.conn().execute(
			"update google set property = ?2 where site = ?1",
			params![site as i64, property],
		)?;
		Ok(())
	}

	fn unlink_google(&self, site: u64) -> rusqlite::Result<()> {
		self.conn()
			.execute("delete from google where site = ?1", [site as i64])?;
		Ok(())
	}

	fn site_domain(&self, site: u64) -> rusqlite::Result<Option<String>> {
		self.conn()
			.query_row(
				"select domain from sites where id = ?1",
				[site as i64],
				|row| row.get(0),
			)
			.optional()
	}
}

#[derive(Deserialize)]
struct Property {
	property: String,
}

fn token(site: u64) -> Option<String> {
	ANALYTICS_ACCESS
		.lock()
		.unwrap_or_else(PoisonError::into_inner)
		.get(&site)
		.filter(|(_, expires)| *expires > now())
		.map(|(token, _)| token.clone())
}

async fn connect_analytics(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path(domain): Path<String>,
) -> Response {
	let (Some(site), Some(client)) = (editable(&web, viewer.user, &domain), &web.google) else {
		return fail(
			StatusCode::FORBIDDEN,
			"Google is not set up on this server.",
		);
	};
	let Some(state) = web.begin(Purpose::Analytics(site.id)) else {
		return fail(StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong.");
	};
	let query = url::form_urlencoded::Serializer::new(String::new())
		.append_pair("client_id", &client.id)
		.append_pair("redirect_uri", &redirect_uri(&web))
		.append_pair("response_type", "code")
		.append_pair("scope", ANALYTICS)
		.append_pair("prompt", "consent")
		.append_pair("state", &state)
		.finish();
	Redirect::to(&format!(
		"https://accounts.google.com/o/oauth2/v2/auth?{query}"
	))
	.into_response()
}

async fn analytics_callback(web: &Web, site: u64, query: Callback) -> Response {
	let Ok(Some(domain)) = web.app.site_domain(site) else {
		return fail(StatusCode::NOT_FOUND, "Site not found.");
	};
	let back = format!("/{domain}/settings#imports");
	let (Some(code), None, Some(client)) = (query.code, query.error, &web.google) else {
		return Redirect::to(&back).into_response();
	};
	let exchanged = web
		.http
		.post(TOKEN)
		.form(&[
			("code", code.as_str()),
			("client_id", &client.id),
			("client_secret", &client.secret),
			("redirect_uri", &redirect_uri(web)),
			("grant_type", "authorization_code"),
		])
		.send()
		.await
		.and_then(reqwest::Response::error_for_status);
	let body: Value = match exchanged {
		Ok(response) => response.json().await.unwrap_or_default(),
		Err(err) => {
			tracing::warn!(%err, "google analytics token");
			return Redirect::to(&back).into_response();
		}
	};
	if let Some(access) = body["access_token"].as_str() {
		let expires = now() + body["expires_in"].as_i64().unwrap_or(0) - 60;
		ANALYTICS_ACCESS
			.lock()
			.unwrap_or_else(PoisonError::into_inner)
			.insert(site, (access.to_owned(), expires));
	}
	Redirect::to(&back).into_response()
}

// whether google is connected, the properties it sees, and how an import is going
async fn analytics(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path(domain): Path<String>,
) -> Response {
	let Some(site) = editable(&web, viewer.user, &domain) else {
		return fail(StatusCode::FORBIDDEN, "You can't change this site.");
	};
	let status = IMPORTS
		.lock()
		.unwrap_or_else(PoisonError::into_inner)
		.get(&site.id)
		.cloned();
	let mut properties = Vec::new();
	if let Some(token) = token(site.id) {
		let listed = web
			.http
			.get("https://analyticsadmin.googleapis.com/v1beta/accountSummaries?pageSize=200")
			.bearer_auth(token)
			.send()
			.await
			.and_then(reqwest::Response::error_for_status);
		let body: Value = match listed {
			Ok(response) => response.json().await.unwrap_or_default(),
			Err(err) => {
				tracing::warn!(%err, "google analytics properties");
				Value::Null
			}
		};
		for account in body["accountSummaries"].as_array().into_iter().flatten() {
			for property in account["propertySummaries"]
				.as_array()
				.into_iter()
				.flatten()
			{
				properties.push(json!({
					"key": property["property"],
					"label": format!(
						"{} / {}",
						account["displayName"].as_str().unwrap_or_default(),
						property["displayName"].as_str().unwrap_or_default()
					),
				}));
			}
		}
	}
	Json(json!({
		"available": web.google.is_some(),
		"connected": token(site.id).is_some(),
		"properties": properties,
		"importing": matches!(status, Some(None)),
		"error": status.flatten(),
	}))
	.into_response()
}

async fn start_import(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Path(domain): Path<String>,
	Json(body): Json<Property>,
) -> Response {
	let Some(site) = editable(&web, viewer.user, &domain) else {
		return fail(StatusCode::FORBIDDEN, "You can't change this site.");
	};
	let Some(token) = token(site.id) else {
		return fail(
			StatusCode::BAD_REQUEST,
			"The Google sign-in expired. Connect again.",
		);
	};
	if !body.property.starts_with("properties/") {
		return fail(StatusCode::BAD_REQUEST, "Pick a property.");
	}
	{
		let mut imports = IMPORTS.lock().unwrap_or_else(PoisonError::into_inner);
		if matches!(imports.get(&site.id), Some(None)) {
			return fail(StatusCode::CONFLICT, "An import is already running.");
		}
		imports.insert(site.id, None);
	}
	tokio::spawn(async move {
		let tz = jiff::tz::TimeZone::get(&site.timezone).unwrap_or(jiff::tz::TimeZone::UTC);
		let before = jiff::Timestamp::from_second(site.created_at)
			.map(|ts| ts.to_zoned(tz).date())
			.unwrap_or_default();
		let result = crate::import::ga4::import(
			&web.http,
			&token,
			&body.property,
			&web.stats.db,
			&site,
			before,
		)
		.await
		.and_then(|range| {
			if let Some((first, last)) = range {
				web.app.add_import(
					site.id,
					"google analytics",
					&first.to_string(),
					&last.to_string(),
				)?;
			}
			Ok(())
		});
		let error = result.err().map(|err| {
			tracing::warn!(%err, site = site.domain, "google analytics import");
			err.to_string()
		});
		let mut imports = IMPORTS.lock().unwrap_or_else(PoisonError::into_inner);
		match error {
			Some(error) => imports.insert(site.id, Some(error)),
			None => imports.remove(&site.id),
		};
	});
	StatusCode::ACCEPTED.into_response()
}
