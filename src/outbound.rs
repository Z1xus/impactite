use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use reqwest::dns::{Addrs, Name, Resolve, Resolving};
use url::{Host, Url};

pub fn public(ip: IpAddr) -> bool {
	match ip.to_canonical() {
		IpAddr::V4(ip) => {
			let [first, second, ..] = ip.octets();
			!(ip.is_private()
				|| ip.is_loopback()
				|| ip.is_link_local()
				|| ip.is_broadcast()
				|| ip.is_documentation()
				|| ip.is_unspecified()
				|| ip.is_multicast()
				|| first == 0
				|| (first == 100 && second & 0xc0 == 64))
		}
		IpAddr::V6(ip) => {
			let first = ip.segments()[0];
			!(ip.is_loopback()
				|| ip.is_unspecified()
				|| ip.is_multicast()
				|| first & 0xfe00 == 0xfc00
				|| first & 0xffc0 == 0xfe80)
		}
	}
}

// names are checked when they resolve, so only an address written into the url is left to check here
pub fn allowed(url: &Url) -> bool {
	match url.host() {
		Some(Host::Ipv4(ip)) => public(ip.into()),
		Some(Host::Ipv6(ip)) => public(ip.into()),
		Some(Host::Domain(_)) => true,
		None => false,
	}
}

struct Public;

impl Resolve for Public {
	fn resolve(&self, name: Name) -> Resolving {
		Box::pin(async move {
			let found: Vec<SocketAddr> = tokio::net::lookup_host((name.as_str(), 0))
				.await?
				.filter(|addr| public(addr.ip()))
				.collect();
			if found.is_empty() {
				return Err("this address is not public".into());
			}
			Ok(Box::new(found.into_iter()) as Addrs)
		})
	}
}

// for addresses people type in: public hosts only, no redirects, and a time limit
pub fn client() -> reqwest::Result<reqwest::Client> {
	reqwest::Client::builder()
		.dns_resolver(Public)
		.redirect(reqwest::redirect::Policy::none())
		.timeout(Duration::from_secs(15))
		.build()
}

// five redirects at most, and a fresh connection each time so latency is honest
pub fn monitor(local: bool) -> reqwest::Result<reqwest::Client> {
	let mut builder = reqwest::Client::builder()
		.pool_max_idle_per_host(0)
		.tls_info(true)
		.user_agent(concat!("Impactite-Uptime/", env!("CARGO_PKG_VERSION")))
		.redirect(reqwest::redirect::Policy::custom(move |attempt| {
			if attempt.previous().len() >= 5 {
				attempt.error("too many redirects")
			} else if !local && !allowed(attempt.url()) {
				attempt.error("this address is not public")
			} else {
				attempt.follow()
			}
		}));
	if !local {
		builder = builder.dns_resolver(Public);
	}
	builder.build()
}
