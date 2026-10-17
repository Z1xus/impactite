use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};

use aws_lc_rs::hmac;
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};
use rusqlite::{OptionalExtension, params};
use serde::Deserialize;
use serde_json::json;

use crate::app::now;
use crate::app::{App, key_hash, lock, token};
use crate::auth::accounts::Viewer;
use crate::web::{Error, Reply, Web};

// rfc 6238 codes, 6 digits every 30 seconds, one step of clock drift allowed
const STEP: i64 = 30;
const BASE32: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
const TRIES: u32 = 5;
const LOCK_SECS: i64 = 5 * 60;

static FAILS: LazyLock<Mutex<HashMap<i64, (u32, i64)>>> = LazyLock::new(Mutex::default);

pub fn routes() -> Router<Arc<Web>> {
	Router::new()
		.route("/api/app/totp/begin", post(begin))
		.route("/api/app/totp/enable", post(enable))
		.route("/api/app/totp/disable", post(disable))
}

#[derive(Deserialize)]
struct Enable {
	secret: String,
	code: String,
}

#[derive(Deserialize)]
struct Disable {
	code: String,
}

fn encode(bytes: &[u8]) -> String {
	let mut out = String::new();
	for chunk in bytes.chunks(5) {
		let mut block = [0u8; 5];
		block[..chunk.len()].copy_from_slice(chunk);
		let bits = block
			.iter()
			.fold(0u64, |acc, &byte| acc << 8 | u64::from(byte));
		for i in 0..(chunk.len() * 8).div_ceil(5) {
			out.push(BASE32[(bits >> (35 - i * 5) & 31) as usize] as char);
		}
	}
	out
}

fn decode(text: &str) -> Option<Vec<u8>> {
	let mut bits = 0u64;
	let mut count = 0;
	let mut out = Vec::new();
	for c in text.bytes().filter(|c| *c != b'=' && *c != b' ') {
		let value = BASE32.iter().position(|&b| b == c.to_ascii_uppercase())?;
		bits = bits << 5 | value as u64;
		count += 5;
		if count >= 8 {
			count -= 8;
			out.push((bits >> count) as u8);
		}
	}
	Some(out)
}

fn code(secret: &[u8], step: i64) -> u32 {
	let key = hmac::Key::new(hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY, secret);
	let tag = hmac::sign(&key, &step.to_be_bytes());
	let tag = tag.as_ref();
	let at = usize::from(tag[19] & 15);
	let number = u32::from_be_bytes([tag[at] & 0x7f, tag[at + 1], tag[at + 2], tag[at + 3]]);
	number % 1_000_000
}

// the step of a matching code, so a used code can be refused later
fn matching(secret: &str, given: &str) -> Option<i64> {
	let secret = decode(secret)?;
	let given: u32 = given.trim().replace(' ', "").parse().ok()?;
	let now = now() / STEP;
	(now - 1..=now + 1).find(|&step| code(&secret, step) == given)
}

pub fn verify(app: &App, user: i64, given: &str) -> Result<(), &'static str> {
	let mut fails = lock(&FAILS);
	if fails
		.get(&user)
		.is_some_and(|&(count, until)| count >= TRIES && until > now())
	{
		return Err("Too many wrong codes. Wait a few minutes.");
	}
	let Ok(Some((secret, last, recovery))) = app.totp(user) else {
		return Err("Two factor is not on.");
	};
	let accepted = match matching(&secret, given) {
		Some(step) if step > last => app.used_step(user, step).is_ok(),
		_ => {
			let hash = key_hash(given.trim());
			let codes: Vec<&str> = recovery
				.split(',')
				.filter(|code| !code.is_empty())
				.collect();
			codes.contains(&hash.as_str())
				&& app
					.set_recovery(
						user,
						&codes
							.iter()
							.filter(|&&code| code != hash)
							.copied()
							.collect::<Vec<_>>()
							.join(","),
					)
					.is_ok()
		}
	};
	if accepted {
		fails.remove(&user);
		return Ok(());
	}
	let entry = fails.entry(user).or_insert((0, 0));
	*entry = (entry.0 + 1, now() + LOCK_SECS);
	Err("Wrong code.")
}

async fn begin(State(web): State<Arc<Web>>, viewer: Viewer) -> Reply {
	let mut bytes = [0u8; 20];
	getrandom::fill(&mut bytes).map_err(|_| Error::broken())?;
	let secret = encode(&bytes);
	let email = web.app.user_email(viewer.user).unwrap_or_default();
	let label: String =
		url::form_urlencoded::byte_serialize(format!("Impactite:{email}").as_bytes()).collect();
	Ok(Json(json!({
		"secret": secret,
		"uri": format!("otpauth://totp/{label}?secret={secret}&issuer=Impactite"),
	})))
}

async fn enable(State(web): State<Arc<Web>>, viewer: Viewer, Json(body): Json<Enable>) -> Reply {
	let step = matching(&body.secret, &body.code)
		.ok_or_else(|| Error::bad("Wrong code. Check the time on your phone."))?;
	let mut codes = Vec::new();
	for _ in 0..8 {
		let token = token()?;
		codes.push(format!("{}-{}", &token[..5], &token[5..10]));
	}
	let hashes: Vec<String> = codes.iter().map(|code| key_hash(code)).collect();
	web.app
		.enable_totp(viewer.user, &body.secret, step, &hashes.join(","))?;
	Ok(Json(json!({ "recovery": codes })))
}

async fn disable(
	State(web): State<Arc<Web>>,
	viewer: Viewer,
	Json(body): Json<Disable>,
) -> Reply<StatusCode> {
	verify(&web.app, viewer.user, &body.code)
		.map_err(|message| Error::new(StatusCode::UNAUTHORIZED, message))?;
	web.app.enable_totp(viewer.user, "", 0, "")?;
	Ok(StatusCode::NO_CONTENT)
}

impl App {
	// secret, last used step and recovery hashes, when two factor is on
	pub fn totp(&self, user: i64) -> rusqlite::Result<Option<(String, i64, String)>> {
		Ok(self
			.conn()
			.query_row(
				"select totp, totp_step, recovery from users where id = ?1",
				[user],
				|row| Ok((row.get::<_, String>(0)?, row.get(1)?, row.get(2)?)),
			)
			.optional()?
			.filter(|(secret, _, _)| !secret.is_empty()))
	}

	fn enable_totp(
		&self,
		user: i64,
		secret: &str,
		step: i64,
		recovery: &str,
	) -> rusqlite::Result<()> {
		self.conn().execute(
			"update users set totp = ?2, totp_step = ?3, recovery = ?4 where id = ?1",
			params![user, secret, step, recovery],
		)?;
		Ok(())
	}

	fn used_step(&self, user: i64, step: i64) -> rusqlite::Result<()> {
		self.conn().execute(
			"update users set totp_step = ?2 where id = ?1",
			params![user, step],
		)?;
		Ok(())
	}

	fn set_recovery(&self, user: i64, recovery: &str) -> rusqlite::Result<()> {
		self.conn().execute(
			"update users set recovery = ?2 where id = ?1",
			params![user, recovery],
		)?;
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	// rfc 6238 appendix b, sha1 with the 20 byte ascii key, last 6 digits
	#[test]
	fn rfc_vectors() {
		let key = b"12345678901234567890";
		for (time, expected) in [
			(59, 287_082),
			(1_111_111_109, 81_804),
			(2_000_000_000, 279_037),
		] {
			assert_eq!(code(key, time / STEP), expected);
		}
		assert_eq!(decode(&encode(key)).as_deref(), Some(&key[..]));
	}
}
