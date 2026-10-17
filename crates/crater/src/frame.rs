use std::fs::{self, File, OpenOptions};
use std::io::{self, BufReader, ErrorKind, Read, Seek, SeekFrom, Write};
use std::path::Path;

use crate::codec::Reader;

const MAX_FRAME: usize = 1 << 31;

pub(crate) fn invalid(what: &str) -> io::Error {
	io::Error::new(ErrorKind::InvalidData, what.to_owned())
}

pub(crate) fn put(buf: &mut Vec<u8>, write: impl FnOnce(&mut Vec<u8>)) {
	let start = buf.len();
	buf.extend_from_slice(&[0; 8]);
	write(buf);
	let payload = &buf[start + 8..];
	let len = (payload.len() as u32).to_le_bytes();
	let crc = crc32fast::hash(payload).to_le_bytes();
	buf[start..start + 4].copy_from_slice(&len);
	buf[start + 4..start + 8].copy_from_slice(&crc);
}

pub(crate) fn open_log(
	path: &Path,
	magic: &[u8],
	mut each: impl FnMut(&[u8]) -> bool,
) -> io::Result<File> {
	let mut file = OpenOptions::new()
		.read(true)
		.write(true)
		.create(true)
		.truncate(false)
		.open(path)?;

	if file.metadata()?.len() == 0 {
		file.write_all(magic)?;
		file.sync_data()?;
		return Ok(file);
	}

	let mut reader = BufReader::new(&file);
	check_magic(&mut reader, magic)?;
	let mut end = magic.len() as u64;
	while let Some(payload) = next(&mut reader)? {
		if !each(&payload) {
			break;
		}
		end += 8 + payload.len() as u64;
	}
	drop(reader);

	file.set_len(end)?;
	file.seek(SeekFrom::Start(end))?;
	Ok(file)
}

pub(crate) fn write_file(path: &Path, magic: &[u8], frames: &[u8]) -> io::Result<()> {
	let tmp = path.with_extension("tmp");
	let mut file = File::create(&tmp)?;
	file.write_all(magic)?;
	file.write_all(frames)?;
	file.sync_all()?;
	fs::rename(&tmp, path)?;
	if let Some(dir) = path.parent() {
		File::open(dir)?.sync_all()?;
	}
	Ok(())
}

pub(crate) fn read_file(path: &Path, magic: &[u8], frames: usize) -> io::Result<Vec<Vec<u8>>> {
	let mut reader = BufReader::new(File::open(path)?);
	check_magic(&mut reader, magic)?;
	(0..frames)
		.map(|_| next(&mut reader)?.ok_or_else(|| invalid("truncated file")))
		.collect()
}

fn check_magic(reader: &mut impl Read, magic: &[u8]) -> io::Result<()> {
	let mut head = vec![0; magic.len()];
	if read_full(reader, &mut head)? && head == magic {
		Ok(())
	} else {
		Err(invalid("unknown file format"))
	}
}

fn next(reader: &mut impl Read) -> io::Result<Option<Vec<u8>>> {
	let mut head = [0; 8];
	if !read_full(reader, &mut head)? {
		return Ok(None);
	}
	let mut r = Reader::new(&head);
	let (Some(len), Some(crc)) = (r.u32(), r.u32()) else {
		return Ok(None);
	};
	let len = len as usize;
	if len > MAX_FRAME {
		return Ok(None);
	}
	let mut payload = vec![0; len];
	if !read_full(reader, &mut payload)? || crc32fast::hash(&payload) != crc {
		return Ok(None);
	}
	Ok(Some(payload))
}

fn read_full(reader: &mut impl Read, buf: &mut [u8]) -> io::Result<bool> {
	match reader.read_exact(buf) {
		Ok(()) => Ok(true),
		Err(err) if err.kind() == ErrorKind::UnexpectedEof => Ok(false),
		Err(err) => Err(err),
	}
}
