use serde::Serialize;
use serde_json::{Value, json};

use crate::card;
use crate::web::Web;

pub struct Note {
	pub subject: String,
	pub line: String,
	pub alert: bool,
	pub fields: Vec<(&'static str, String)>,
	pub lists: Vec<(&'static str, Vec<(String, u64)>)>,
	pub series: Vec<u64>,
	pub link: String,
}

impl Note {
	pub fn alert(subject: String, line: String, link: String) -> Self {
		Self {
			subject,
			line,
			alert: true,
			fields: Vec::new(),
			lists: Vec::new(),
			series: Vec::new(),
			link,
		}
	}

	pub fn table(&self) -> String {
		let fields = self
			.fields
			.iter()
			.map(|(label, value)| format!("{label:<13} {value}"));
		let lists = self.lists.iter().map(|(label, rows)| {
			let rows = rows
				.iter()
				.map(|(name, count)| format!("{:>8}  {name}", grouped(*count)));
			std::iter::once(format!("\n{label}"))
				.chain(rows)
				.collect::<Vec<_>>()
				.join("\n")
		});
		fields.chain(lists).collect::<Vec<_>>().join("\n")
	}
}

pub fn grouped(n: u64) -> String {
	let digits = n.to_string();
	let mut out = String::new();
	for (i, ch) in digits.chars().enumerate() {
		if i > 0 && (digits.len() - i).is_multiple_of(3) {
			out.push(',');
		}
		out.push(ch);
	}
	out
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
	Email,
	Slack,
	Discord,
	Telegram,
	Ntfy,
	Webhook,
}

#[derive(Serialize)]
pub struct Target {
	pub kind: Kind,
	pub label: String,
	#[serde(skip)]
	url: Option<url::Url>,
}

pub fn classify(to: &str) -> Result<Target, &'static str> {
	if to.len() > 500 {
		return Err("This is too long");
	}
	if !to.contains("://") {
		let mailbox: lettre::message::Mailbox = to
			.parse()
			.map_err(|_| "Enter an email address or an https URL")?;
		return Ok(Target {
			kind: Kind::Email,
			label: mailbox.email.to_string(),
			url: None,
		});
	}
	if !to.starts_with("https://") {
		return Err("Use an https URL");
	}
	let url = url::Url::parse(to).map_err(|_| "This is not a valid URL")?;
	if !crate::outbound::allowed(&url) {
		return Err("This address is not public");
	}
	let host = url.host_str().unwrap_or_default().to_owned();
	let path = url.path().trim_end_matches('/');
	let parts = path.split('/').count();
	// the last characters tell targets apart without the secret
	let tail = |text: &str| {
		let chars: Vec<char> = text.chars().collect();
		format!(
			"…{}",
			chars[chars.len().saturating_sub(4)..]
				.iter()
				.collect::<String>()
		)
	};
	let last = path.rsplit('/').next().unwrap_or_default();
	let (kind, label) = if host == "hooks.slack.com" {
		if !path.starts_with("/services/") || parts < 5 {
			return Err("Use the whole Slack webhook URL");
		}
		(Kind::Slack, tail(last))
	} else if matches!(host.as_str(), "discord.com" | "discordapp.com") {
		if !path.starts_with("/api/webhooks/") || parts < 5 {
			return Err("Use the whole Discord webhook URL");
		}
		(Kind::Discord, tail(last))
	} else if host == "api.telegram.org" {
		if !path.starts_with("/bot") || !path.ends_with("/sendMessage") {
			return Err("Use the bot sendMessage URL");
		}
		let chat = url
			.query_pairs()
			.find(|(key, id)| key == "chat_id" && !id.is_empty());
		let chat = chat.ok_or("Add chat_id to the Telegram URL")?;
		(Kind::Telegram, format!("chat {}", tail(&chat.1)))
	} else if host.starts_with("ntfy.") {
		let topic = path.trim_start_matches('/');
		if topic.is_empty() || topic.contains('/') {
			return Err("Add a topic to the ntfy URL");
		}
		(Kind::Ntfy, format!("{host} {}", tail(topic)))
	} else {
		(Kind::Webhook, host)
	};
	Ok(Target {
		kind,
		label,
		url: Some(url),
	})
}

pub fn failure(err: &anyhow::Error) -> String {
	match err.downcast_ref::<reqwest::Error>() {
		Some(err) if err.status().is_some() => {
			format!("HTTP {}", err.status().map_or(0, |code| code.as_u16()))
		}
		Some(err) if err.is_timeout() => "Timed out".to_owned(),
		Some(_) => "Could not connect".to_owned(),
		None => err.to_string(),
	}
}

pub async fn deliver(
	web: &Web,
	to: &str,
	note: &Note,
	site: &str,
	kind: &str,
) -> anyhow::Result<()> {
	let target = classify(to).map_err(anyhow::Error::msg)?;
	let table = note.table();
	let Some(url) = target.url else {
		let lines = [note.line.clone()];
		let letter = crate::mail::Letter {
			base: &web.url,
			title: &note.subject,
			lines: &lines,
			table: (!table.is_empty()).then_some(table.as_str()),
			button: Some(("Open dashboard", &note.link)),
		};
		return web
			.mail
			.send(to, &note.subject, &letter.text(), &letter.html())
			.await;
	};
	let text = [note.line.as_str(), &table, &note.link].join("\n\n");
	let request = match target.kind {
		Kind::Slack => web.hooks.post(url).json(&slack(note)),
		Kind::Discord => discord(web, note, url)?,
		Kind::Telegram => {
			let chat = url.query_pairs().find(|(key, _)| key == "chat_id");
			let chat = chat.map(|(_, id)| id.into_owned()).unwrap_or_default();
			let mut endpoint = url;
			endpoint.set_query(None);
			let text = format!("{}\n\n{text}", note.subject);
			web.hooks.post(endpoint).json(&json!({
				"chat_id": chat,
				"text": text.chars().take(4096).collect::<String>(),
			}))
		}
		Kind::Ntfy => {
			let title: String = note
				.subject
				.chars()
				.filter(|ch| ch.is_ascii_graphic() || *ch == ' ')
				.collect();
			web.hooks
				.post(url)
				.header("Title", title)
				.header("Click", &note.link)
				.header("Tags", if note.alert { "warning" } else { "bar_chart" })
				.header("Priority", if note.alert { "high" } else { "default" })
				.body(text)
		}
		_ => web.hooks.post(url).json(&json!({
			"subject": note.subject,
			"text": text,
			"site": site,
			"kind": kind,
			"link": note.link,
		})),
	};
	request.send().await?.error_for_status()?;
	Ok(())
}

fn discord(web: &Web, note: &Note, to: url::Url) -> anyhow::Result<reqwest::RequestBuilder> {
	let mut fields: Vec<Value> = note
		.fields
		.iter()
		.take(3)
		.map(|(name, value)| json!({ "name": name, "value": value, "inline": true }))
		.collect();
	fields.extend(
		note.lists
			.iter()
			.filter(|(_, rows)| !rows.is_empty())
			.map(|(name, rows)| {
				let value = rows
					.iter()
					.map(|(row, count)| format!("`{}` {row}", grouped(*count)))
					.collect::<Vec<_>>()
					.join("\n");
				json!({ "name": name, "value": value.chars().take(1000).collect::<String>(), "inline": true })
			}),
	);
	let mut embed = json!({
		"title": note.subject,
		"url": note.link,
		"description": note.line,
		"color": if note.alert { 0xf0645a } else { 0x38e1f2 },
		"fields": fields,
		"footer": { "text": "impactite" },
	});
	if note.series.is_empty() {
		return Ok(web.hooks.post(to).json(&json!({ "embeds": [embed] })));
	}
	embed["image"] = json!({ "url": "attachment://chart.png" });
	let mut chart = card::Canvas::new(800, 200, card::INK);
	chart.area([0, 8, 800, 192], &note.series, 8);
	let form = reqwest::multipart::Form::new()
		.text("payload_json", json!({ "embeds": [embed] }).to_string())
		.part(
			"files[0]",
			reqwest::multipart::Part::bytes(chart.png())
				.file_name("chart.png")
				.mime_str("image/png")?,
		);
	Ok(web.hooks.post(to).multipart(form))
}

fn slack(note: &Note) -> Value {
	let mut blocks = vec![
		json!({ "type": "header", "text": { "type": "plain_text", "text": note.subject } }),
		json!({ "type": "section", "text": { "type": "mrkdwn", "text": note.line } }),
	];
	if !note.fields.is_empty() {
		let fields: Vec<Value> = note
			.fields
			.iter()
			.map(|(name, value)| json!({ "type": "mrkdwn", "text": format!("*{name}*\n{value}") }))
			.collect();
		blocks.push(json!({ "type": "section", "fields": fields }));
	}
	for (name, rows) in note.lists.iter().filter(|(_, rows)| !rows.is_empty()) {
		let rows = rows
			.iter()
			.map(|(row, count)| format!("`{}` {row}", grouped(*count)))
			.collect::<Vec<_>>()
			.join("\n");
		blocks.push(
			json!({ "type": "section", "text": { "type": "mrkdwn", "text": format!("*{name}*\n{rows}") } }),
		);
	}
	blocks.push(json!({
		"type": "actions",
		"elements": [{ "type": "button", "text": { "type": "plain_text", "text": "Open dashboard" }, "url": note.link }],
	}));
	json!({ "text": format!("{}. {}", note.subject, note.line), "blocks": blocks })
}
