use aws_lc_rs::digest::{SHA256, digest};
use aws_lc_rs::signature::{
	ECDSA_P256_SHA256_ASN1, ED25519, RSA_PKCS1_2048_8192_SHA256, UnparsedPublicKey,
	VerificationAlgorithm,
};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64;
use ciborium::Value;
use serde::Deserialize;
use serde_json::{Value as Json, json};

const UP: u8 = 0x01;
const UV: u8 = 0x04;
const AT: u8 = 0x40;
const TIMEOUT_MS: u64 = 300_000;

pub struct Relying {
	pub id: String,
	pub origin: String,
}

impl Relying {
	pub fn from_url(url: &str) -> anyhow::Result<Self> {
		let url = url::Url::parse(url)?;
		Ok(Self {
			id: url.host_str().unwrap_or("localhost").to_owned(),
			origin: url.origin().ascii_serialization(),
		})
	}
}

#[derive(Deserialize)]
pub struct Registration {
	#[serde(rename = "rawId")]
	raw_id: String,
	response: Attestation,
}

#[derive(Deserialize)]
struct Attestation {
	#[serde(rename = "clientDataJSON")]
	client_data: String,
	#[serde(rename = "attestationObject")]
	attestation: String,
}

#[derive(Deserialize)]
pub struct Assertion {
	#[serde(rename = "rawId")]
	raw_id: String,
	response: Signed,
}

#[derive(Deserialize)]
struct Signed {
	#[serde(rename = "clientDataJSON")]
	client_data: String,
	#[serde(rename = "authenticatorData")]
	authenticator_data: String,
	signature: String,
}

#[derive(Deserialize)]
struct ClientData {
	#[serde(rename = "type")]
	kind: String,
	challenge: String,
	origin: String,
}

pub struct Key {
	pub id: Vec<u8>,
	pub alg: i64,
	pub public: Vec<u8>,
	pub counter: u32,
}

impl Assertion {
	pub fn credential(&self) -> Option<Vec<u8>> {
		B64.decode(&self.raw_id).ok()
	}
}

pub fn challenge() -> Option<String> {
	let mut bytes = [0; 32];
	getrandom::fill(&mut bytes).ok()?;
	Some(B64.encode(bytes))
}

pub fn creation_options(
	rp: &Relying,
	challenge: &str,
	handle: &[u8],
	email: &str,
	name: &str,
	exclude: &[Vec<u8>],
) -> Json {
	json!({
		"challenge": challenge,
		"rp": { "id": rp.id, "name": "impactite" },
		"user": { "id": B64.encode(handle), "name": email, "displayName": name },
		"pubKeyCredParams": [
			{ "type": "public-key", "alg": -8 },
			{ "type": "public-key", "alg": -7 },
			{ "type": "public-key", "alg": -257 },
		],
		"timeout": TIMEOUT_MS,
		"attestation": "none",
		"authenticatorSelection": { "residentKey": "required", "userVerification": "required" },
		"excludeCredentials": exclude
			.iter()
			.map(|id| json!({ "type": "public-key", "id": B64.encode(id) }))
			.collect::<Vec<_>>(),
	})
}

pub fn request_options(rp: &Relying, challenge: &str) -> Json {
	json!({
		"challenge": challenge,
		"rpId": rp.id,
		"timeout": TIMEOUT_MS,
		"userVerification": "required",
		"allowCredentials": [],
	})
}

pub fn register(
	rp: &Relying,
	challenge: &str,
	registration: &Registration,
) -> Result<Key, &'static str> {
	let response = &registration.response;
	client(rp, &response.client_data, "webauthn.create", challenge)?;
	let attestation: Value = ciborium::from_reader(decode(&response.attestation)?.as_slice())
		.map_err(|_| "unreadable attestation")?;
	let auth = field(&attestation, "authData")
		.and_then(Value::as_bytes)
		.ok_or("missing authenticator data")?;
	let flags = authenticator(rp, auth)?;
	if flags & AT == 0 {
		return Err("missing credential");
	}

	let id_len = auth
		.get(53..55)
		.map(|len| usize::from(u16::from_be_bytes([len[0], len[1]])))
		.ok_or("truncated credential")?;
	let id = auth
		.get(55..55 + id_len)
		.ok_or("truncated credential")?
		.to_vec();
	if decode(&registration.raw_id)? != id {
		return Err("credential id mismatch");
	}
	let cose: Value = ciborium::from_reader(auth.get(55 + id_len..).ok_or("missing key")?)
		.map_err(|_| "unreadable key")?;
	let (alg, public) = cose_key(&cose).ok_or("unsupported key")?;
	Ok(Key {
		id,
		alg,
		public,
		counter: counter(auth),
	})
}

pub fn verify(
	rp: &Relying,
	challenge: &str,
	assertion: &Assertion,
	key: &Key,
) -> Result<u32, &'static str> {
	let response = &assertion.response;
	let client_data = client(rp, &response.client_data, "webauthn.get", challenge)?;
	let auth = decode(&response.authenticator_data)?;
	authenticator(rp, &auth)?;

	let mut message = auth.clone();
	message.extend_from_slice(digest(&SHA256, &client_data).as_ref());
	let algorithm: &'static dyn VerificationAlgorithm = match key.alg {
		-7 => &ECDSA_P256_SHA256_ASN1,
		-8 => &ED25519,
		-257 => &RSA_PKCS1_2048_8192_SHA256,
		_ => return Err("unsupported key"),
	};
	UnparsedPublicKey::new(algorithm, &key.public)
		.verify(&message, &decode(&response.signature)?)
		.map_err(|_| "bad signature")?;

	let next = counter(&auth);
	if key.counter > 0 && next <= key.counter {
		return Err("cloned authenticator");
	}
	Ok(next)
}

fn client(rp: &Relying, raw: &str, kind: &str, challenge: &str) -> Result<Vec<u8>, &'static str> {
	let bytes = decode(raw)?;
	let data: ClientData = serde_json::from_slice(&bytes).map_err(|_| "unreadable client data")?;
	if data.kind != kind || data.challenge != challenge || data.origin != rp.origin {
		return Err("ceremony mismatch");
	}
	Ok(bytes)
}

fn authenticator(rp: &Relying, auth: &[u8]) -> Result<u8, &'static str> {
	let rp_hash = auth.get(..32).ok_or("truncated authenticator data")?;
	let flags = *auth.get(32).ok_or("truncated authenticator data")?;
	if rp_hash != digest(&SHA256, rp.id.as_bytes()).as_ref() {
		return Err("wrong relying party");
	}
	if flags & UP == 0 || flags & UV == 0 {
		return Err("user not verified");
	}
	Ok(flags)
}

fn counter(auth: &[u8]) -> u32 {
	auth.get(33..37).map_or(0, |bytes| {
		u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
	})
}

fn decode(text: &str) -> Result<Vec<u8>, &'static str> {
	B64.decode(text.trim_end_matches('='))
		.map_err(|_| "bad encoding")
}

fn field<'a>(value: &'a Value, name: &str) -> Option<&'a Value> {
	value
		.as_map()?
		.iter()
		.find(|(key, _)| key.as_text() == Some(name))
		.map(|(_, value)| value)
}

fn param(value: &Value, label: i64) -> Option<&Value> {
	value
		.as_map()?
		.iter()
		.find(|(key, _)| key.as_integer().and_then(|n| i64::try_from(n).ok()) == Some(label))
		.map(|(_, value)| value)
}

fn int(value: &Value, label: i64) -> Option<i64> {
	param(value, label)?
		.as_integer()
		.and_then(|n| i64::try_from(n).ok())
}

fn bytes(value: &Value, label: i64) -> Option<&[u8]> {
	param(value, label)?.as_bytes().map(Vec::as_slice)
}

fn cose_key(key: &Value) -> Option<(i64, Vec<u8>)> {
	match (int(key, 1)?, int(key, 3)?) {
		(2, -7) if int(key, -1) == Some(1) => {
			let mut point = vec![0x04];
			point.extend_from_slice(bytes(key, -2).filter(|x| x.len() == 32)?);
			point.extend_from_slice(bytes(key, -3).filter(|y| y.len() == 32)?);
			Some((-7, point))
		}
		(1, -8) if int(key, -1) == Some(6) => {
			Some((-8, bytes(key, -2).filter(|x| x.len() == 32)?.to_vec()))
		}
		(3, -257) => Some((-257, rsa_der(bytes(key, -1)?, bytes(key, -2)?))),
		_ => None,
	}
}

fn rsa_der(n: &[u8], e: &[u8]) -> Vec<u8> {
	let mut body = der_integer(n);
	body.extend(der_integer(e));
	der(0x30, &body)
}

fn der_integer(value: &[u8]) -> Vec<u8> {
	let trimmed = &value[value.iter().take_while(|&&b| b == 0).count()..];
	let mut content = Vec::with_capacity(trimmed.len() + 1);
	if trimmed.first().is_none_or(|&b| b & 0x80 != 0) {
		content.push(0);
	}
	content.extend_from_slice(trimmed);
	der(0x02, &content)
}

fn der(tag: u8, content: &[u8]) -> Vec<u8> {
	let mut out = vec![tag];
	match content.len() {
		len @ 0..0x80 => out.push(len as u8),
		len => {
			let bytes: Vec<u8> = len
				.to_be_bytes()
				.into_iter()
				.skip_while(|&b| b == 0)
				.collect();
			out.push(0x80 | bytes.len() as u8);
			out.extend(bytes);
		}
	}
	out.extend_from_slice(content);
	out
}

#[cfg(test)]
mod tests {
	use aws_lc_rs::rand::SystemRandom;
	use aws_lc_rs::signature::{ECDSA_P256_SHA256_ASN1_SIGNING, EcdsaKeyPair, KeyPair};

	use super::*;

	const ORIGIN: &str = "http://localhost:5173";

	fn rp() -> Relying {
		Relying {
			id: "localhost".into(),
			origin: ORIGIN.into(),
		}
	}

	fn client_data(kind: &str, challenge: &str, origin: &str) -> String {
		B64.encode(json!({ "type": kind, "challenge": challenge, "origin": origin }).to_string())
	}

	fn authenticator_data(flags: u8, counter: u32, credential: Option<(&[u8], &[u8])>) -> Vec<u8> {
		let mut data = digest(&SHA256, b"localhost").as_ref().to_vec();
		data.push(flags);
		data.extend_from_slice(&counter.to_be_bytes());
		if let Some((id, cose)) = credential {
			data.extend_from_slice(&[0; 16]);
			data.extend_from_slice(&(id.len() as u16).to_be_bytes());
			data.extend_from_slice(id);
			data.extend_from_slice(cose);
		}
		data
	}

	fn cbor(value: &Value) -> Vec<u8> {
		let mut out = Vec::new();
		ciborium::into_writer(value, &mut out).unwrap();
		out
	}

	#[test]
	fn passkey_round_trip() {
		let rng = SystemRandom::new();
		let key = EcdsaKeyPair::generate(&ECDSA_P256_SHA256_ASN1_SIGNING).unwrap();
		let point = key.public_key().as_ref();
		let id = b"credential-1";
		let cose = cbor(&Value::Map(vec![
			(1.into(), 2.into()),
			(3.into(), (-7).into()),
			((-1).into(), 1.into()),
			((-2).into(), Value::Bytes(point[1..33].to_vec())),
			((-3).into(), Value::Bytes(point[33..].to_vec())),
		]));
		let attestation = cbor(&Value::Map(vec![
			("fmt".into(), "none".into()),
			("attStmt".into(), Value::Map(Vec::new())),
			(
				"authData".into(),
				Value::Bytes(authenticator_data(UP | UV | AT, 0, Some((id, &cose)))),
			),
		]));
		let registration = Registration {
			raw_id: B64.encode(id),
			response: Attestation {
				client_data: client_data("webauthn.create", "c1", ORIGIN),
				attestation: B64.encode(attestation),
			},
		};
		assert!(register(&rp(), "other", &registration).is_err());
		let stored = register(&rp(), "c1", &registration).unwrap();

		let assert = |challenge: &str, origin: &str, flags: u8, tamper: bool| {
			let client = client_data("webauthn.get", challenge, origin);
			let auth = authenticator_data(flags, 1, None);
			let mut message = auth.clone();
			message.extend_from_slice(digest(&SHA256, &B64.decode(&client).unwrap()).as_ref());
			let mut signature = key.sign(&rng, &message).unwrap().as_ref().to_vec();
			if tamper {
				signature[8] ^= 1;
			}
			Assertion {
				raw_id: B64.encode(id),
				response: Signed {
					client_data: client,
					authenticator_data: B64.encode(auth),
					signature: B64.encode(signature),
				},
			}
		};

		assert_eq!(
			verify(&rp(), "c2", &assert("c2", ORIGIN, UP | UV, false), &stored),
			Ok(1)
		);
		assert!(verify(&rp(), "c2", &assert("c3", ORIGIN, UP | UV, false), &stored).is_err());
		assert!(
			verify(
				&rp(),
				"c2",
				&assert("c2", "https://evil.example", UP | UV, false),
				&stored
			)
			.is_err()
		);
		assert!(verify(&rp(), "c2", &assert("c2", ORIGIN, UP, false), &stored).is_err());
		assert!(verify(&rp(), "c2", &assert("c2", ORIGIN, UP | UV, true), &stored).is_err());
	}
}
