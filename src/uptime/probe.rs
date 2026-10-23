use std::error::Error;
use std::io;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use reqwest::{Client, Method};
use tokio::net::{TcpStream, lookup_host};
use tokio::time::timeout;
use url::Url;

use crate::outbound;
use crate::uptime::{Kind, Monitor};

const BODY: usize = 1 << 20;
const DETAIL: usize = 512;

// error kinds for the code byte of a check, http statuses use their class (2 to 5)
pub const TIMEOUT: u8 = 10;
pub const REFUSED: u8 = 11;
pub const DNS: u8 = 12;
pub const TLS: u8 = 13;
pub const BODY_CODE: u8 = 14;
pub const OTHER: u8 = 15;
pub const OVERDUE: u8 = 16;
pub const REPORTED: u8 = 17;

#[derive(Clone)]
pub struct Outcome {
	pub ok: bool,
	pub ms: u32,
	pub code: u8,
	pub cause: String,
	pub detail: String,
	pub cert: Option<i64>,
}

impl Outcome {
	pub fn up(ms: u32) -> Self {
		Self {
			ok: true,
			ms,
			code: 2,
			cause: String::new(),
			detail: String::new(),
			cert: None,
		}
	}

	pub fn down(code: u8, cause: impl Into<String>, detail: impl Into<String>) -> Self {
		Self {
			ok: false,
			ms: 0,
			code,
			cause: cause.into(),
			detail: detail.into(),
			cert: None,
		}
	}
}

pub struct Clients {
	public: Client,
	local: Client,
}

impl Clients {
	pub fn new() -> reqwest::Result<Self> {
		Ok(Self {
			public: outbound::monitor(false)?,
			local: outbound::monitor(true)?,
		})
	}
}

pub async fn run(clients: &Clients, monitor: &Monitor, local: bool) -> Outcome {
	match monitor.kind {
		Kind::Http => {
			let client = if local {
				&clients.local
			} else {
				&clients.public
			};
			http(client, monitor, local).await
		}
		Kind::Tcp => tcp(monitor, local).await,
		Kind::Heartbeat => Outcome::up(0),
	}
}

fn millis(start: Instant) -> u32 {
	(start.elapsed().as_millis() as u32).clamp(1, u32::from(u16::MAX))
}

async fn http(client: &Client, monitor: &Monitor, local: bool) -> Outcome {
	let config = &monitor.config;
	// the resolver never sees an address written into the url
	if !local && !Url::parse(&monitor.target).is_ok_and(|url| outbound::allowed(&url)) {
		return Outcome::down(OTHER, "Address is not public", "");
	}
	let method = match config.method.as_str() {
		"HEAD" => Method::HEAD,
		"POST" => Method::POST,
		_ => Method::GET,
	};
	let mut request = client
		.request(method, &monitor.target)
		.timeout(Duration::from_secs(config.timeout));
	for (name, value) in &config.headers {
		request = request.header(name, value);
	}
	let start = Instant::now();
	let mut response = match request.send().await {
		Ok(response) => response,
		Err(err) => return failed(&err, config.timeout),
	};
	let status = response.status().as_u16();
	let cert = response
		.extensions()
		.get::<reqwest::tls::TlsInfo>()
		.and_then(|info| info.peer_certificate())
		.and_then(not_after);
	let mut body = Vec::new();
	loop {
		match response.chunk().await {
			Ok(Some(chunk)) => {
				body.extend_from_slice(&chunk);
				if body.len() >= BODY {
					break;
				}
			}
			Ok(None) => break,
			Err(err) => return failed(&err, config.timeout),
		}
	}
	let ms = millis(start);
	let text = String::from_utf8_lossy(&body);
	let mut out = if !config.accepts(status) {
		Outcome::down(
			(status / 100) as u8,
			format!("HTTP {status}"),
			excerpt(&text),
		)
	} else if !config.contains.is_empty() && !text.contains(&config.contains) {
		Outcome::down(
			BODY_CODE,
			format!("Body is missing \"{}\"", short(&config.contains)),
			excerpt(&text),
		)
	} else if !config.lacks.is_empty() && text.contains(&config.lacks) {
		Outcome::down(
			BODY_CODE,
			format!("Body has \"{}\"", short(&config.lacks)),
			excerpt(&text),
		)
	} else {
		Outcome::up(ms)
	};
	out.ms = ms;
	out.cert = cert;
	if out.ok {
		out.code = (status / 100) as u8;
	}
	out
}

fn short(text: &str) -> String {
	text.chars().take(40).collect()
}

fn excerpt(text: &str) -> String {
	let mut end = text.len().min(DETAIL);
	while !text.is_char_boundary(end) {
		end -= 1;
	}
	text[..end].to_owned()
}

fn not_after(der: &[u8]) -> Option<i64> {
	let (_, cert) = x509_parser::parse_x509_certificate(der).ok()?;
	Some(cert.validity().not_after.timestamp())
}

fn failed(err: &reqwest::Error, timeout: u64) -> Outcome {
	let mut chain = err.to_string();
	let mut source = err.source();
	while let Some(next) = source {
		chain.push_str(&format!(": {next}"));
		source = next.source();
	}
	let lower = chain.to_lowercase();
	let (code, cause) = if err.is_timeout() {
		(TIMEOUT, format!("Timed out after {timeout} s"))
	} else if lower.contains("expired") {
		(TLS, "Certificate expired".to_owned())
	} else if lower.contains("certificate") || lower.contains("tls") || lower.contains("handshake")
	{
		(TLS, "Certificate is not valid".to_owned())
	} else if lower.contains("refused") {
		(REFUSED, "Connection refused".to_owned())
	} else if lower.contains("not public") {
		(OTHER, "Address is not public".to_owned())
	} else if lower.contains("dns") || lower.contains("lookup") || lower.contains("resolve") {
		(DNS, "DNS lookup failed".to_owned())
	} else if lower.contains("too many redirects") {
		(OTHER, "Too many redirects".to_owned())
	} else if lower.contains("reset") {
		(OTHER, "Connection reset".to_owned())
	} else {
		(OTHER, "Request failed".to_owned())
	};
	Outcome::down(code, cause, excerpt(&chain))
}

async fn tcp(monitor: &Monitor, local: bool) -> Outcome {
	let limit = Duration::from_secs(monitor.config.timeout);
	let start = Instant::now();
	let found = match timeout(limit, lookup_host(&monitor.target)).await {
		Err(_) => return timed_out(monitor),
		Ok(Err(err)) => return Outcome::down(DNS, "DNS lookup failed", err.to_string()),
		Ok(Ok(found)) => found,
	};
	let addrs: Vec<SocketAddr> = found
		.filter(|addr| local || outbound::public(addr.ip()))
		.collect();
	if addrs.is_empty() {
		return Outcome::down(OTHER, "Address is not public", "");
	}
	let left = limit.saturating_sub(start.elapsed());
	match timeout(left, TcpStream::connect(&addrs[..])).await {
		Err(_) => timed_out(monitor),
		Ok(Err(err)) if err.kind() == io::ErrorKind::ConnectionRefused => {
			Outcome::down(REFUSED, "Connection refused", err.to_string())
		}
		Ok(Err(err)) => Outcome::down(OTHER, "Connection failed", err.to_string()),
		Ok(Ok(_)) => Outcome::up(millis(start)),
	}
}

fn timed_out(monitor: &Monitor) -> Outcome {
	Outcome::down(
		TIMEOUT,
		format!("Timed out after {} s", monitor.config.timeout),
		"",
	)
}
