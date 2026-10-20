use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::app::lock;

const KEYS: usize = 50_000;

// strikes per key inside a window, a key at its maximum stays shut until the window ends
pub struct Limit {
	max: u32,
	window: Duration,
	strikes: Mutex<HashMap<String, (u32, Instant)>>,
}

impl Limit {
	pub fn new(max: u32, minutes: u64) -> Self {
		Self {
			max,
			window: Duration::from_secs(minutes * 60),
			strikes: Mutex::default(),
		}
	}

	pub fn open(&self, key: &str) -> bool {
		lock(&self.strikes)
			.get(key)
			.is_none_or(|(count, since)| *count < self.max || since.elapsed() >= self.window)
	}

	pub fn strike(&self, key: &str) {
		let mut strikes = lock(&self.strikes);
		if strikes.len() >= KEYS {
			strikes.retain(|_, (_, since)| since.elapsed() < self.window);
		}
		if strikes.len() >= KEYS && !strikes.contains_key(key) {
			return;
		}
		let strike = strikes.entry(key.to_owned()).or_insert((0, Instant::now()));
		if strike.1.elapsed() >= self.window {
			*strike = (0, Instant::now());
		}
		strike.0 += 1;
	}

	pub fn clear(&self, key: &str) {
		lock(&self.strikes).remove(key);
	}
}
