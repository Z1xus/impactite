use std::net::{IpAddr, SocketAddr};

use anyhow::{Context, bail};
use axum::http::HeaderMap;

const DEFAULT: &str =
	"127.0.0.0/8,::1/128,10.0.0.0/8,172.16.0.0/12,192.168.0.0/16,fc00::/7,100.64.0.0/10";
const IP_HEADERS: &[&str] = &[
	"cf-connecting-ip",
	"true-client-ip",
	"x-forwarded-for",
	"x-real-ip",
];
const COUNTRY_HEADERS: &[&str] = &[
	"cf-ipcountry",
	"x-vercel-ip-country",
	"cloudfront-viewer-country",
];

#[derive(Debug, Clone)]
pub struct Proxies(Vec<(IpAddr, u8)>);

pub struct Visitor {
	pub ip: IpAddr,
	pub country: Option<String>,
}

impl Proxies {
	pub fn parse(spec: Option<&str>) -> anyhow::Result<Self> {
		let spec = spec.unwrap_or(DEFAULT);
		let ranges = spec
			.split(',')
			.map(str::trim)
			.filter(|part| !part.is_empty())
			.flat_map(|part| match part {
				"*" => vec!["0.0.0.0/0", "::/0"],
				part => vec![part],
			})
			.map(|part| {
				let (ip, bits) = part.split_once('/').unwrap_or((part, ""));
				let ip: IpAddr = ip
					.parse()
					.with_context(|| format!("bad proxy range {part}"))?;
				let max = if ip.is_ipv4() { 32 } else { 128 };
				let bits = match bits {
					"" => max,
					bits => bits
						.parse()
						.with_context(|| format!("bad proxy range {part}"))?,
				};
				if bits > max {
					bail!("bad proxy range {part}");
				}
				Ok((ip, bits))
			})
			.collect::<anyhow::Result<_>>()?;
		Ok(Self(ranges))
	}

	pub fn visitor(&self, peer: SocketAddr, headers: &HeaderMap) -> Visitor {
		let peer = peer.ip().to_canonical();
		if !self.contains(peer) {
			return Visitor {
				ip: peer,
				country: None,
			};
		}
		let header = |name: &str| headers.get(name).and_then(|value| value.to_str().ok());
		let ip = IP_HEADERS
			.iter()
			.filter_map(|name| {
				header(name)?
					.split(',')
					.next()?
					.trim()
					.parse::<IpAddr>()
					.ok()
			})
			.chain(header("forwarded").and_then(forwarded))
			.next()
			.map_or(peer, |ip| ip.to_canonical());
		let country = COUNTRY_HEADERS
			.iter()
			.filter_map(|name| header(name))
			.map(str::to_ascii_uppercase)
			.find(|code| code.len() == 2 && !matches!(code.as_str(), "XX" | "T1"));
		Visitor { ip, country }
	}

	pub fn contains(&self, ip: IpAddr) -> bool {
		self.0.iter().any(|&(range, bits)| match (range, ip) {
			(IpAddr::V4(range), IpAddr::V4(ip)) => {
				let mask = u32::MAX.checked_shl(32 - u32::from(bits)).unwrap_or(0);
				u32::from(range) & mask == u32::from(ip) & mask
			}
			(IpAddr::V6(range), IpAddr::V6(ip)) => {
				let mask = u128::MAX.checked_shl(128 - u32::from(bits)).unwrap_or(0);
				u128::from(range) & mask == u128::from(ip) & mask
			}
			_ => false,
		})
	}
}

pub fn reported(headers: &HeaderMap) -> Option<IpAddr> {
	let first = headers
		.get("x-forwarded-for")?
		.to_str()
		.ok()?
		.split(',')
		.next()?;
	Some(first.trim().parse::<IpAddr>().ok()?.to_canonical())
}

fn forwarded(value: &str) -> Option<IpAddr> {
	value
		.split([';', ','])
		.filter_map(|part| part.trim().strip_prefix("for="))
		.map(|ip| ip.trim_matches('"'))
		.map(|ip| {
			ip.strip_prefix('[')
				.and_then(|ip| ip.split(']').next())
				.unwrap_or(ip)
		})
		.find_map(|ip| {
			ip.parse()
				.ok()
				.or_else(|| ip.rsplit_once(':').and_then(|(ip, _)| ip.parse().ok()))
		})
}
