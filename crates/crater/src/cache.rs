use std::collections::HashMap;
use std::hash::Hash;

pub(crate) struct Cache<K, V> {
	entries: HashMap<K, (V, u64)>,
	clock: u64,
	capacity: usize,
}

impl<K: Hash + Eq + Clone, V: Clone> Cache<K, V> {
	pub(crate) fn new(capacity: usize) -> Self {
		Self {
			entries: HashMap::new(),
			clock: 0,
			capacity,
		}
	}

	pub(crate) fn get(&mut self, key: &K) -> Option<V> {
		self.clock += 1;
		let (value, used) = self.entries.get_mut(key)?;
		*used = self.clock;
		Some(value.clone())
	}

	pub(crate) fn insert(&mut self, key: K, value: V) {
		self.clock += 1;
		if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
			let oldest = self
				.entries
				.iter()
				.min_by_key(|(_, (_, used))| *used)
				.map(|(key, _)| key.clone());
			if let Some(oldest) = oldest {
				self.entries.remove(&oldest);
			}
		}
		self.entries.insert(key, (value, self.clock));
	}
}
