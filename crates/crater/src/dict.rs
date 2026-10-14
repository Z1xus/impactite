use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Write};
use std::path::Path;
use std::sync::Arc;

use crate::codec::{Reader, put_str};
use crate::frame;

const MAGIC: &[u8; 5] = b"CRDC\x01";

pub(crate) struct Dict {
	file: File,
	strings: Vec<Arc<str>>,
	ids: HashMap<Arc<str>, u32>,
	flushed: usize,
}

impl Dict {
	pub(crate) fn open(path: &Path) -> io::Result<Self> {
		let mut strings: Vec<Arc<str>> = Vec::new();
		let file = frame::open_log(path, MAGIC, |payload| {
			let mut r = Reader::new(payload);
			let mut batch = Vec::new();
			while !r.is_empty() {
				let Some(s) = r.str() else {
					return false;
				};
				batch.push(Arc::from(s));
			}
			strings.append(&mut batch);
			true
		})?;
		let ids = strings
			.iter()
			.enumerate()
			.map(|(id, s)| (s.clone(), id as u32))
			.collect();
		Ok(Self {
			file,
			flushed: strings.len(),
			strings,
			ids,
		})
	}

	pub(crate) fn id(&mut self, s: &str) -> u32 {
		if let Some(&id) = self.ids.get(s) {
			return id;
		}
		let id = self.strings.len() as u32;
		let s: Arc<str> = Arc::from(s);
		self.strings.push(s.clone());
		self.ids.insert(s, id);
		id
	}

	pub(crate) fn find(&self, s: &str) -> Option<u32> {
		self.ids.get(s).copied()
	}

	pub(crate) fn get(&self, id: u32) -> Option<&Arc<str>> {
		self.strings.get(id as usize)
	}

	pub(crate) fn flush(&mut self) -> io::Result<()> {
		let pending = &self.strings[self.flushed..];
		if pending.is_empty() {
			return Ok(());
		}
		let mut buf = Vec::new();
		frame::put(&mut buf, |buf| {
			for s in pending {
				put_str(buf, s);
			}
		});
		self.file.write_all(&buf)?;
		self.file.sync_data()?;
		self.flushed = self.strings.len();
		Ok(())
	}
}
