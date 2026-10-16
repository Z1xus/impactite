use std::sync::Arc;

use aws_lc_rs::digest::{SHA256, digest};
use aws_lc_rs::signature::{
	ECDSA_P256_SHA256_FIXED, ED25519, RSA_PKCS1_2048_8192_SHA256, UnparsedPublicKey,
	VerificationAlgorithm,
};
use axum::Router;
use axum::extract::{Query, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::get;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64;
use serde::Deserialize;
use serde_json::Value;

use crate::accounts::{Web, now, signed_in};
use crate::passkeys::Purpose;
use crate::webauthn;

pub struct Provider {
	pub issuer: String,
	pub id: String,
	pub secret: String,
	pub name: String,
}

pub fn routes() -> Router<Arc<Web>> {
	Router::new()
		.route("/api/app/sso", get(start))
		.route("/api/app/sso/callback", get(callback))
}

#[derive(Deserialize)]
struct Callback {
	state: String,
	code: Option<String>,
}

struct Discovery {
	authorize: String,
	token: String,
	keys: String,
}

async fn discover(web: &Web, provider: &Provider) -> anyhow::Result<Discovery> {
	let url = format!(
		"{}/.well-known/openid-configuration",
		provider.issuer.trim_end_matches('/')
	);
	let doc: Value = web
		.http
		.get(url)
		.send()
		.await?
		.error_for_status()?
		.json()
		.await?;
	let field = |name: &str| {
		doc[name]
			.as_str()
			.map(str::to_owned)
			.ok_or_else(|| anyhow::anyhow!("discovery has no {name}"))
	};
	Ok(Discovery {
		authorize: field("authorization_endpoint")?,
		token: field("token_endpoint")?,
		keys: field("jwks_uri")?,
	})
}

fn redirect_uri(web: &Web) -> String {
	format!("{}/api/app/sso/callback", web.url.trim_end_matches('/'))
}

fn failed(reason: &str) -> Response {
	Redirect::to(&format!("/login?sso={reason}")).into_response()
}

async fn start(State(web): State<Arc<Web>>) -> Response {
	let Some(provider) = &web.sso else {
		return failed("off");
	};
	let discovery = match discover(&web, provider).await {
		Ok(discovery) => discovery,
		Err(err) => {
			tracing::warn!(%err, "sso discovery");
			return failed("provider");
		}
	};
	let (Some(nonce), Some(verifier)) = (webauthn::challenge(), webauthn::challenge()) else {
		return failed("provider");
	};
	let Some(state) = web.begin(Purpose::Sso(nonce.clone(), verifier.clone())) else {
		return failed("provider");
	};
	let challenge = B64.encode(digest(&SHA256, verifier.as_bytes()));
	let query = url::form_urlencoded::Serializer::new(String::new())
		.append_pair("client_id", &provider.id)
		.append_pair("redirect_uri", &redirect_uri(&web))
		.append_pair("response_type", "code")
		.append_pair("scope", "openid email profile")
		.append_pair("state", &state)
		.append_pair("nonce", &nonce)
		.append_pair("code_challenge", &challenge)
		.append_pair("code_challenge_method", "S256")
		.finish();
	let glue = if discovery.authorize.contains('?') {
		'&'
	} else {
		'?'
	};
	Redirect::to(&format!("{}{glue}{query}", discovery.authorize)).into_response()
}

async fn callback(State(web): State<Arc<Web>>, Query(query): Query<Callback>) -> Response {
	let Some(Purpose::Sso(nonce, verifier)) = web.take(&query.state) else {
		return failed("expired");
	};
	let (Some(provider), Some(code)) = (&web.sso, query.code) else {
		return failed("denied");
	};
	let claims = match identify(&web, provider, &code, &nonce, &verifier).await {
		Ok(claims) => claims,
		Err(err) => {
			tracing::warn!(%err, "sso sign in");
			return failed("provider");
		}
	};
	let Some(email) = claims["email"].as_str().map(str::trim) else {
		return failed("email");
	};
	if claims["email_verified"] == Value::Bool(false) {
		return failed("email");
	}
	let user = match web.app.user_by_email(email) {
		Ok(Some(user)) => user,
		// a new person only gets in where anyone could sign up
		Ok(None) if web.app.signup_open() => {
			let name = claims["name"].as_str().unwrap_or(email);
			match web.app.create_user(name, email, None) {
				Ok(user) => user,
				Err(err) => {
					tracing::error!(%err, "sso account");
					return failed("provider");
				}
			}
		}
		Ok(None) => return failed("invite"),
		Err(err) => {
			tracing::error!(%err, "sso account");
			return failed("provider");
		}
	};
	let mut response = Redirect::to("/").into_response();
	let session = signed_in(&web, user, None);
	if let Some(cookie) = session.headers().get(axum::http::header::SET_COOKIE) {
		response
			.headers_mut()
			.insert(axum::http::header::SET_COOKIE, cookie.clone());
	}
	response
}

async fn identify(
	web: &Web,
	provider: &Provider,
	code: &str,
	nonce: &str,
	verifier: &str,
) -> anyhow::Result<Value> {
	let discovery = discover(web, provider).await?;
	let tokens: Value = web
		.http
		.post(&discovery.token)
		.form(&[
			("grant_type", "authorization_code"),
			("code", code),
			("redirect_uri", &redirect_uri(web)),
			("client_id", &provider.id),
			("client_secret", &provider.secret),
			("code_verifier", verifier),
		])
		.send()
		.await?
		.error_for_status()?
		.json()
		.await?;
	let token = tokens["id_token"]
		.as_str()
		.ok_or_else(|| anyhow::anyhow!("no id token"))?;
	let mut parts = token.split('.');
	let (Some(head), Some(body), Some(signature), None) =
		(parts.next(), parts.next(), parts.next(), parts.next())
	else {
		anyhow::bail!("malformed id token");
	};
	let header: Value = serde_json::from_slice(&B64.decode(head)?)?;
	let claims: Value = serde_json::from_slice(&B64.decode(body)?)?;

	let keys: Value = web
		.http
		.get(&discovery.keys)
		.send()
		.await?
		.error_for_status()?
		.json()
		.await?;
	let kid = header["kid"].as_str();
	let key = keys["keys"]
		.as_array()
		.into_iter()
		.flatten()
		.find(|key| kid.is_none() || key["kid"].as_str() == kid)
		.ok_or_else(|| anyhow::anyhow!("no matching signing key"))?;
	let bytes = |name: &str| -> anyhow::Result<Vec<u8>> {
		Ok(B64.decode(key[name].as_str().unwrap_or_default())?)
	};
	let (algorithm, public): (&'static dyn VerificationAlgorithm, Vec<u8>) =
		match header["alg"].as_str() {
			Some("RS256") => (
				&RSA_PKCS1_2048_8192_SHA256,
				webauthn::rsa_der(&bytes("n")?, &bytes("e")?),
			),
			Some("ES256") => {
				let mut point = vec![0x04];
				point.extend(bytes("x")?);
				point.extend(bytes("y")?);
				(&ECDSA_P256_SHA256_FIXED, point)
			}
			Some("EdDSA") => (&ED25519, bytes("x")?),
			other => anyhow::bail!("unsupported signing algorithm {other:?}"),
		};
	UnparsedPublicKey::new(algorithm, &public)
		.verify(format!("{head}.{body}").as_bytes(), &B64.decode(signature)?)
		.map_err(|_| anyhow::anyhow!("bad id token signature"))?;

	let audience = match &claims["aud"] {
		Value::String(aud) => aud == &provider.id,
		Value::Array(all) => all.iter().any(|aud| aud.as_str() == Some(&provider.id)),
		_ => false,
	};
	let issuer = claims["iss"].as_str().map(|iss| iss.trim_end_matches('/'))
		== Some(provider.issuer.trim_end_matches('/'));
	let fresh = claims["exp"].as_i64().is_some_and(|exp| exp > now());
	if !audience || !issuer || !fresh || claims["nonce"].as_str() != Some(nonce) {
		anyhow::bail!("id token is not for this sign in");
	}
	Ok(claims)
}
