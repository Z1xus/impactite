use std::fs::{File, OpenOptions};
use std::io::{self, BufReader, ErrorKind, Read, Seek, SeekFrom, Write};
use std::path::Path;

use crate::codec::{Reader, put_ivar, put_str, put_uvar};
use crate::event::{Dim, Event};

const MAGIC: &[u8; 5] = b"CRWL\x01";
const MAX_RECORD: usize = 1 << 24;

pub struct Wal {
	file: File,
	buf: Vec<u8>,
}

impl Wal {
	pub fn open(path: &Path) -> io::Result<(Self, Vec<Event>)> {
		let mut file = OpenOptions::new()
			.read(true)
			.write(true)
			.create(true)
			.truncate(false)
			.open(path)?;
		let mut events = Vec::new();

		if file.metadata()?.len() == 0 {
			file.write_all(MAGIC)?;
			file.sync_data()?;
		} else {
			let end = replay(&file, &mut events)?;
			file.set_len(end)?;
			file.seek(SeekFrom::Start(end))?;
		}

		Ok((
			Self {
				file,
				buf: Vec::new(),
			},
			events,
		))
	}

	pub fn append(&mut self, events: &[Event]) -> io::Result<()> {
		self.buf.clear();
		for event in events {
			let start = self.buf.len();
			self.buf.extend_from_slice(&[0; 8]);
			encode(&mut self.buf, event);
			let payload = &self.buf[start + 8..];
			let len = (payload.len() as u32).to_le_bytes();
			let crc = crc32fast::hash(payload).to_le_bytes();
			self.buf[start..start + 4].copy_from_slice(&len);
			self.buf[start + 4..start + 8].copy_from_slice(&crc);
		}
		self.file.write_all(&self.buf)?;
		self.file.sync_data()
	}
}

fn replay(file: &File, events: &mut Vec<Event>) -> io::Result<u64> {
	let mut reader = BufReader::new(file);
	let mut magic = [0; MAGIC.len()];
	if !read_full(&mut reader, &mut magic)? || &magic != MAGIC {
		return Err(io::Error::new(ErrorKind::InvalidData, "not a crater wal"));
	}

	let mut end = MAGIC.len() as u64;
	let mut head = [0; 8];
	let mut payload = Vec::new();
	while read_full(&mut reader, &mut head)? {
		let mut r = Reader::new(&head);
		let (Some(len), Some(crc)) = (r.u32(), r.u32()) else {
			break;
		};
		let len = len as usize;
		if len > MAX_RECORD {
			break;
		}
		payload.resize(len, 0);
		if !read_full(&mut reader, &mut payload)? || crc32fast::hash(&payload) != crc {
			break;
		}
		let Some(event) = decode(&payload) else {
			break;
		};
		events.push(event);
		end += 8 + len as u64;
	}
	Ok(end)
}

fn read_full(reader: &mut impl Read, buf: &mut [u8]) -> io::Result<bool> {
	match reader.read_exact(buf) {
		Ok(()) => Ok(true),
		Err(err) if err.kind() == ErrorKind::UnexpectedEof => Ok(false),
		Err(err) => Err(err),
	}
}

fn encode(buf: &mut Vec<u8>, event: &Event) {
	put_uvar(buf, event.site);
	put_ivar(buf, event.ts);
	put_ivar(buf, event.offset.into());
	buf.extend_from_slice(&event.visitor.to_le_bytes());
	buf.extend_from_slice(&event.session.to_le_bytes());
	buf.push(u8::from(event.interactive) | u8::from(event.scroll_depth.is_some()) << 1);
	if let Some(depth) = event.scroll_depth {
		buf.push(depth);
	}
	put_uvar(buf, event.engagement_ms.into());

	let mask = Dim::ALL
		.iter()
		.enumerate()
		.filter(|&(_, &dim)| !event.dims[dim].is_empty())
		.fold(0u64, |mask, (i, _)| mask | 1 << i);
	put_uvar(buf, mask);
	for dim in Dim::ALL {
		if !event.dims[dim].is_empty() {
			put_str(buf, &event.dims[dim]);
		}
	}

	put_uvar(buf, event.props.len() as u64);
	for (key, value) in &event.props {
		put_str(buf, key);
		put_str(buf, value);
	}
}

fn decode(buf: &[u8]) -> Option<Event> {
	let mut r = Reader::new(buf);
	let mut event = Event {
		site: r.uvar()?,
		ts: r.ivar()?,
		offset: i32::try_from(r.ivar()?).ok()?,
		visitor: r.u64()?,
		session: r.u64()?,
		..Event::default()
	};

	let flags = r.u8()?;
	event.interactive = flags & 1 != 0;
	if flags & 2 != 0 {
		event.scroll_depth = Some(r.u8()?);
	}
	event.engagement_ms = u32::try_from(r.uvar()?).ok()?;

	let mask = r.uvar()?;
	for (i, dim) in Dim::ALL.into_iter().enumerate() {
		if mask >> i & 1 != 0 {
			event.dims[dim] = r.str()?;
		}
	}

	for _ in 0..r.uvar()? {
		event.props.push((r.str()?, r.str()?));
	}

	r.is_empty().then_some(event)
}
