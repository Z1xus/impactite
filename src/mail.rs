use anyhow::{Context, bail};
use lettre::message::{Mailbox, MultiPart};
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use serde_json::json;

enum Transport {
	Smtp(AsyncSmtpTransport<Tokio1Executor>),
	Resend(String),
	Cloudflare { account: String, token: String },
	Log,
}

pub struct Mail {
	transport: Transport,
	from: Mailbox,
	http: reqwest::Client,
}

impl Mail {
	pub fn new(url: Option<&str>, from: &str) -> anyhow::Result<Self> {
		let transport = match url {
			None => Transport::Log,
			Some(url) if url.starts_with("smtp://") || url.starts_with("smtps://") => {
				let url = match url.starts_with("smtp://") && !url.contains("tls=") {
					true => format!(
						"{url}{}tls=required",
						if url.contains('?') { '&' } else { '?' }
					),
					false => url.to_owned(),
				};
				Transport::Smtp(
					AsyncSmtpTransport::<Tokio1Executor>::from_url(&url)
						.context("invalid smtp url")?
						.build(),
				)
			}
			Some(url) if url.starts_with("resend://") => {
				Transport::Resend(url.trim_start_matches("resend://").to_owned())
			}
			Some(url) if url.starts_with("cloudflare://") => {
				let (token, account) = url
					.trim_start_matches("cloudflare://")
					.split_once('@')
					.context("use cloudflare://API_TOKEN@ACCOUNT_ID")?;
				Transport::Cloudflare {
					account: account.to_owned(),
					token: token.to_owned(),
				}
			}
			Some(_) => {
				bail!("mail url must start with smtp://, smtps://, resend:// or cloudflare://")
			}
		};
		Ok(Self {
			transport,
			from: from.parse().context("invalid mail sender")?,
			http: reqwest::Client::new(),
		})
	}

	pub fn ready(&self) -> bool {
		!matches!(self.transport, Transport::Log)
	}

	pub async fn send(
		&self,
		to: &str,
		subject: &str,
		text: &str,
		html: &str,
	) -> anyhow::Result<()> {
		let to_box: Mailbox = to.parse().context("invalid recipient")?;
		match &self.transport {
			Transport::Smtp(smtp) => {
				let message = Message::builder()
					.from(self.from.clone())
					.to(to_box)
					.subject(subject)
					.multipart(MultiPart::alternative_plain_html(
						text.to_owned(),
						html.to_owned(),
					))?;
				smtp.send(message).await?;
			}
			Transport::Resend(key) => {
				self.http
					.post("https://api.resend.com/emails")
					.bearer_auth(key)
					.json(&json!({
						"from": self.from.to_string(),
						"to": [to],
						"subject": subject,
						"text": text,
						"html": html,
					}))
					.send()
					.await?
					.error_for_status()?;
			}
			Transport::Cloudflare { account, token } => {
				self.http
					.post(format!(
						"https://api.cloudflare.com/client/v4/accounts/{account}/email/sending/send"
					))
					.bearer_auth(token)
					.json(&json!({
						"from": self.from.email.to_string(),
						"to": to_box.email.to_string(),
						"subject": subject,
						"text": text,
						"html": html,
					}))
					.send()
					.await?
					.error_for_status()?;
			}
			Transport::Log => tracing::info!(to, subject, text, "mail"),
		}
		Ok(())
	}
}

const MONO: &str = "ui-monospace,'SFMono-Regular',Menlo,Consolas,monospace";
const SANS: &str = "-apple-system,'Segoe UI',Roboto,Helvetica,Arial,sans-serif";

pub struct Letter<'a> {
	// the instance address, for the logo and the footer
	pub base: &'a str,
	pub title: &'a str,
	pub lines: &'a [String],
	pub table: Option<&'a str>,
	pub button: Option<(&'a str, &'a str)>,
}

impl Letter<'_> {
	pub fn text(&self) -> String {
		let mut out = format!("{}\n\n", self.title);
		for line in self.lines {
			out.push_str(&format!("{line}\n\n"));
		}
		if let Some(table) = self.table {
			out.push_str(&format!("{table}\n\n"));
		}
		if let Some((_, url)) = self.button {
			out.push_str(&format!("{url}\n\n"));
		}
		out.push_str(&self.footer());
		out
	}

	pub fn html(&self) -> String {
		let mut body = format!(
			"<div style=\"font:600 18px {MONO};color:#e3eef0;margin:0 0 16px\">{}</div>",
			escape(self.title)
		);
		for line in self.lines {
			body.push_str(&format!(
				"<p style=\"font:15px/1.55 {SANS};color:#8798a0;margin:0 0 14px\">{}</p>",
				escape(line)
			));
		}
		if let Some(table) = self.table {
			body.push_str(&format!(
				"<pre style=\"font:13px/1.55 {MONO};color:#e3eef0;background:#06090b;border:1px solid #172127;border-radius:2px;padding:14px;margin:0 0 16px;white-space:pre-wrap\">{}</pre>",
				escape(table)
			));
		}
		if let Some((label, url)) = self.button {
			let url = escape(url);
			body.push_str(&format!(
				"<table role=\"presentation\" cellpadding=\"0\" cellspacing=\"0\" style=\"margin:6px 0 16px\"><tr><td style=\"background:#38e1f2;border-radius:2px\"><a href=\"{url}\" style=\"display:inline-block;padding:11px 18px;font:500 12px {MONO};letter-spacing:0.08em;text-transform:uppercase;color:#06090b;text-decoration:none\">{}</a></td></tr></table><p style=\"font:12px/1.5 {SANS};color:#64757e;margin:0;word-break:break-all\">Or open <a href=\"{url}\" style=\"color:#64757e\">{url}</a></p>",
				escape(label)
			));
		}
		let base = escape(self.base.trim_end_matches('/'));
		// the logo comes from the server, a mail sent without one goes without it
		let logo = match base.is_empty() {
			true => String::new(),
			false => format!(
				"<img src=\"{base}/apple-touch-icon.png\" width=\"22\" height=\"22\" alt=\"\" style=\"vertical-align:middle;border-radius:2px\">"
			),
		};
		format!(
			"<!doctype html><html><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width\"><meta name=\"color-scheme\" content=\"dark\"><meta name=\"supported-color-schemes\" content=\"dark\"></head>\
			<body style=\"margin:0;padding:0;background:#06090b\">\
			<table role=\"presentation\" width=\"100%\" cellpadding=\"0\" cellspacing=\"0\" style=\"background:#06090b\"><tr><td align=\"center\" style=\"padding:40px 16px\">\
			<table role=\"presentation\" width=\"100%\" cellpadding=\"0\" cellspacing=\"0\" style=\"max-width:520px\">\
			<tr><td style=\"padding:0 0 18px\">{logo}<span style=\"font:15px {MONO};color:#e3eef0;vertical-align:middle;padding-left:8px\">impactite</span></td></tr>\
			<tr><td style=\"background:#0a0f12;border:1px solid #172127;border-radius:2px;padding:28px\">{body}</td></tr>\
			<tr><td style=\"padding:16px 2px 0;font:12px {SANS};color:#64757e\">{}</td></tr>\
			</table></td></tr></table></body></html>",
			escape(&self.footer())
		)
	}

	fn footer(&self) -> String {
		match url::Url::parse(self.base)
			.ok()
			.and_then(|url| url.host_str().map(str::to_owned))
		{
			Some(host) => format!("Sent by impactite at {host}."),
			None => "Sent by impactite.".to_owned(),
		}
	}
}

fn escape(text: &str) -> String {
	text.replace('&', "&amp;")
		.replace('<', "&lt;")
		.replace('>', "&gt;")
		.replace('"', "&quot;")
}
