use std::sync::Arc;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use jiff::tz::TimeZone;
use serde_json::{Value, json};

use super::key_user;
use crate::site::notes;
use crate::stats::{key_reads, run, unauthorized};
use crate::update::VERSION;
use crate::web::Web;
use crate::{insights, vitals};

const PROTOCOLS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];

pub fn routes() -> Router<Arc<Web>> {
	Router::new().route(
		"/mcp",
		post(call).get(|| async { StatusCode::METHOD_NOT_ALLOWED }),
	)
}

fn tools() -> Value {
	let site = json!({ "type": "string", "description": "The site domain, like example.com" });
	let range = json!({
		"description": "\"day\", \"7d\", \"28d\", \"30d\", \"91d\", \"month\", \"6mo\", \"12mo\", \"year\", \"all\" or [\"2026-01-01\", \"2026-01-31\"]",
	});
	let filters = json!({
		"type": "array",
		"description": "Plausible style filters like [\"is\", \"visit:country\", [\"DE\"]]",
	});
	json!([
		{
			"name": "list_sites",
			"description": "Lists the sites this key can read.",
			"inputSchema": { "type": "object", "properties": {} },
		},
		{
			"name": "query",
			"description": "Runs a Plausible Stats API v2 query. Metrics: visitors, visits, pageviews, views_per_visit, bounce_rate, visit_duration, events, conversion_rate, time_on_page, scroll_depth, exit_rate, total_revenue, average_revenue. Dimensions: event:page, event:hostname, event:goal, event:props:<key>, visit:source, visit:referrer, visit:channel, visit:utm_*, visit:country, visit:region, visit:city, visit:device, visit:browser, visit:os, visit:language, visit:entry_page, visit:exit_page, time:hour, time:day, time:week, time:month.",
			"inputSchema": {
				"type": "object",
				"properties": {
					"site_id": site,
					"metrics": { "type": "array", "items": { "type": "string" } },
					"date_range": range,
					"dimensions": { "type": "array", "items": { "type": "string" } },
					"filters": filters,
					"order_by": { "type": "array" },
					"pagination": { "type": "object" },
				},
				"required": ["site_id", "metrics", "date_range"],
			},
		},
		{
			"name": "explain_change",
			"description": "Finds traffic spikes in a range and the sources and pages that drove them, and what changed most against a previous range.",
			"inputSchema": {
				"type": "object",
				"properties": { "site_id": site, "date_range": range, "previous": range, "filters": filters },
				"required": ["site_id", "date_range"],
			},
		},
		{
			"name": "page_speed",
			"description": "Web vitals at the 75th percentile with good, needs work and poor shares. With a metric (lcp, inp, cls, fcp or ttfb) it lists the busiest pages, slowest first.",
			"inputSchema": {
				"type": "object",
				"properties": {
					"site_id": site,
					"date_range": range,
					"filters": filters,
					"metric": { "type": "string", "enum": ["lcp", "inp", "cls", "fcp", "ttfb"] },
				},
				"required": ["site_id", "date_range"],
			},
		},
		{
			"name": "current_visitors",
			"description": "Counts visitors on the site in the last 5 minutes.",
			"inputSchema": { "type": "object", "properties": { "site_id": site }, "required": ["site_id"] },
		},
		{
			"name": "add_note",
			"description": "Adds a note to the visitor chart, like a launch or a release.",
			"inputSchema": {
				"type": "object",
				"properties": {
					"site_id": site,
					"text": { "type": "string" },
					"date": { "type": "string", "description": "Like 2026-10-24, today when left out" },
				},
				"required": ["site_id", "text"],
			},
		},
	])
}

fn use_tool(web: &Web, key: &str, user: i64, name: &str, args: Value) -> Result<Value, String> {
	let domain = args["site_id"].as_str().unwrap_or_default();
	if name != "list_sites" && !key_reads(&web.app, key, domain) {
		return Err(format!("This key can't read {domain}."));
	}
	match name {
		"list_sites" => {
			// a server key reads every site
			let sites = match web.app.key_owner(key) {
				Ok(Some(None)) => web.app.sites(),
				_ => web.app.user_sites(user),
			}
			.map_err(|err| err.to_string())?;
			Ok(json!(
				sites
					.iter()
					.map(|site| json!({ "domain": site.domain, "timezone": site.timezone }))
					.collect::<Vec<_>>()
			))
		}
		"query" => run(&web.stats, args),
		"explain_change" => insights::explain(&web.stats, args),
		"page_speed" if args["metric"].is_string() => vitals::slowest(&web.stats, args),
		"page_speed" => vitals::measure(&web.stats, args),
		"current_visitors" => {
			let site = web
				.app
				.site(domain)
				.map_err(|err| err.to_string())?
				.ok_or("This site doesn't exist.")?;
			Ok(json!({ "visitors": web.ingest.current_visitors(site.id) }))
		}
		"add_note" => {
			let site = web
				.app
				.editable(user, domain)
				.ok_or("This key can't change the site.")?;
			let today = jiff::Timestamp::now()
				.to_zoned(TimeZone::get(&site.timezone).unwrap_or(TimeZone::UTC))
				.date()
				.to_string();
			let (day, text) = notes::checked(
				args["date"].as_str().unwrap_or(&today),
				args["text"].as_str().unwrap_or_default(),
			)?;
			let id = web
				.app
				.add_site_note(site.id, &day, &text)
				.map_err(|err| err.to_string())?;
			Ok(json!({ "id": id, "date": day, "text": text }))
		}
		_ => Err(format!("Unknown tool {name}.")),
	}
}

// json-rpc over plain http posts, each answer comes back in the response
async fn call(
	State(web): State<Arc<Web>>,
	headers: HeaderMap,
	Json(message): Json<Value>,
) -> Response {
	let Some(user) = key_user(&web, &headers) else {
		return unauthorized();
	};
	let key = headers
		.get(header::AUTHORIZATION)
		.and_then(|value| value.to_str().ok())
		.and_then(|value| value.strip_prefix("Bearer "))
		.unwrap_or_default();
	let Some(id) = message.get("id").cloned() else {
		return StatusCode::ACCEPTED.into_response();
	};
	let params = &message["params"];
	let result = match message["method"].as_str().unwrap_or_default() {
		"initialize" => {
			let asked = params["protocolVersion"].as_str().unwrap_or_default();
			Ok(json!({
				"protocolVersion": PROTOCOLS.iter().find(|&&known| known == asked).unwrap_or(&PROTOCOLS[0]),
				"capabilities": { "tools": {} },
				"serverInfo": { "name": "impactite", "version": VERSION },
			}))
		}
		"ping" => Ok(json!({})),
		"tools/list" => Ok(json!({ "tools": tools() })),
		"tools/call" => {
			let name = params["name"].as_str().unwrap_or_default();
			let args = params
				.get("arguments")
				.cloned()
				.unwrap_or_else(|| json!({}));
			Ok(match use_tool(&web, key, user, name, args) {
				Ok(value) => json!({
					"content": [{ "type": "text", "text": value.to_string() }],
					"structuredContent": if value.is_object() { value } else { json!({ "result": value }) },
				}),
				Err(message) => json!({
					"content": [{ "type": "text", "text": message }],
					"isError": true,
				}),
			})
		}
		method => Err(json!({ "code": -32601, "message": format!("Method not found: {method}") })),
	};
	Json(match result {
		Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
		Err(error) => json!({ "jsonrpc": "2.0", "id": id, "error": error }),
	})
	.into_response()
}
