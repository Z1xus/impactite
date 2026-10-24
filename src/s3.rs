use anyhow::{Context, bail};
use aws_lc_rs::digest::{SHA256, digest};
use aws_lc_rs::hmac;

use crate::app::hex;

// an s3 compatible bucket written as s3://KEY:SECRET@host/bucket/prefix?region=us-east-1,
// s3+http:// for a bucket on the local network
pub struct Bucket {
	scheme: &'static str,
	host: String,
	path: String,
	region: String,
	key: String,
	secret: String,
}

impl Bucket {
	pub fn parse(destination: &str) -> anyhow::Result<Self> {
		let url = url::Url::parse(destination)?;
		let scheme = match url.scheme() {
			"s3" => "https",
			"s3+http" => "http",
			_ => bail!("use s3:// for the bucket"),
		};
		let decode = |text: &str| {
			url::form_urlencoded::parse(format!("x={text}").as_bytes())
				.next()
				.map(|(_, value)| value.into_owned())
				.unwrap_or_default()
		};
		let (key, secret) = (
			decode(url.username()),
			decode(url.password().unwrap_or_default()),
		);
		if key.is_empty() || secret.is_empty() || url.path().trim_matches('/').is_empty() {
			bail!("the bucket needs a key, a secret and a bucket name");
		}
		Ok(Self {
			scheme,
			host: match url.port() {
				Some(port) => format!("{}:{port}", url.host_str().context("no host")?),
				None => url.host_str().context("no host")?.to_owned(),
			},
			path: url.path().trim_end_matches('/').to_owned(),
			region: url
				.query_pairs()
				.find(|(name, _)| name == "region")
				.map_or_else(|| "us-east-1".into(), |(_, region)| region.into_owned()),
			key,
			secret,
		})
	}

	// a bucket on the local network, set up by an instance admin
	pub fn local(&self) -> bool {
		self.scheme == "http"
	}

	pub fn shown(&self) -> String {
		format!("bucket {}{}", self.host, self.path)
	}

	pub async fn put(
		&self,
		http: &reqwest::Client,
		name: &str,
		body: Vec<u8>,
	) -> anyhow::Result<()> {
		let path: String = format!("{}/{name}", self.path)
			.split('/')
			.map(|part| {
				url::form_urlencoded::byte_serialize(part.as_bytes())
					.collect::<String>()
					.replace('+', "%20")
			})
			.collect::<Vec<_>>()
			.join("/");
		let now = jiff::Timestamp::now();
		let stamp = now.strftime("%Y%m%dT%H%M%SZ").to_string();
		let day = &stamp[..8];
		let payload = hex(digest(&SHA256, &body).as_ref());
		let headers = format!(
			"host:{}\nx-amz-content-sha256:{payload}\nx-amz-date:{stamp}\n",
			self.host
		);
		let signed = "host;x-amz-content-sha256;x-amz-date";
		let request = format!("PUT\n{path}\n\n{headers}\n{signed}\n{payload}");
		let scope = format!("{day}/{}/s3/aws4_request", self.region);
		let to_sign = format!(
			"AWS4-HMAC-SHA256\n{stamp}\n{scope}\n{}",
			hex(digest(&SHA256, request.as_bytes()).as_ref())
		);
		let sign = |key: &[u8], data: &str| {
			hmac::sign(&hmac::Key::new(hmac::HMAC_SHA256, key), data.as_bytes())
		};
		let key = [day, self.region.as_str(), "s3", "aws4_request"]
			.into_iter()
			.fold(format!("AWS4{}", self.secret).into_bytes(), |key, part| {
				sign(&key, part).as_ref().to_vec()
			});
		let signature = hex(sign(&key, &to_sign).as_ref());
		let response = http
			.put(format!("{}://{}{path}", self.scheme, self.host))
			.header("x-amz-date", &stamp)
			.header("x-amz-content-sha256", &payload)
			.header(
				"authorization",
				format!(
					"AWS4-HMAC-SHA256 Credential={}/{scope}, SignedHeaders={signed}, Signature={signature}",
					self.key
				),
			)
			.body(body)
			.send()
			.await?;
		if !response.status().is_success() {
			let status = response.status();
			bail!(
				"the bucket said {status}: {}",
				response.text().await.unwrap_or_default()
			);
		}
		Ok(())
	}
}
