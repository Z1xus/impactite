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
