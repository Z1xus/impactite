pub(crate) fn put_uvar(buf: &mut Vec<u8>, mut v: u64) {
	while v >= 0x80 {
		buf.push(v as u8 | 0x80);
		v >>= 7;
	}
	buf.push(v as u8);
}

pub(crate) fn put_ivar(buf: &mut Vec<u8>, v: i64) {
	put_uvar(buf, ((v << 1) ^ (v >> 63)) as u64);
}

pub(crate) fn put_str(buf: &mut Vec<u8>, s: &str) {
	put_uvar(buf, s.len() as u64);
	buf.extend_from_slice(s.as_bytes());
}

pub(crate) fn put_packed(buf: &mut Vec<u8>, values: &[u32]) {
	let width = values
		.iter()
		.max()
		.map_or(0, |max| 32 - max.leading_zeros());
	put_uvar(buf, width.into());
	put_uvar(buf, values.len() as u64);
	if width == 0 {
		return;
	}
	let (mut acc, mut bits) = (0u64, 0);
	for &value in values {
		acc |= u64::from(value) << bits;
		bits += width;
		while bits >= 8 {
			buf.push(acc as u8);
			acc >>= 8;
			bits -= 8;
		}
	}
	if bits > 0 {
		buf.push(acc as u8);
	}
}

pub(crate) struct Reader<'a> {
	buf: &'a [u8],
}

impl<'a> Reader<'a> {
	pub(crate) fn new(buf: &'a [u8]) -> Self {
		Self { buf }
	}

	pub(crate) fn is_empty(&self) -> bool {
		self.buf.is_empty()
	}

	pub(crate) fn bytes(&mut self, n: usize) -> Option<&'a [u8]> {
		let (head, tail) = self.buf.split_at_checked(n)?;
		self.buf = tail;
		Some(head)
	}

	pub(crate) fn u8(&mut self) -> Option<u8> {
		let (&b, tail) = self.buf.split_first()?;
		self.buf = tail;
		Some(b)
	}

	pub(crate) fn u32(&mut self) -> Option<u32> {
		Some(u32::from_le_bytes(self.bytes(4)?.try_into().ok()?))
	}

	pub(crate) fn u64(&mut self) -> Option<u64> {
		Some(u64::from_le_bytes(self.bytes(8)?.try_into().ok()?))
	}

	pub(crate) fn uvar(&mut self) -> Option<u64> {
		let mut v = 0u64;
		for shift in (0..64).step_by(7) {
			let b = self.u8()?;
			v |= u64::from(b & 0x7f) << shift;
			if b < 0x80 {
				return Some(v);
			}
		}
		None
	}

	pub(crate) fn ivar(&mut self) -> Option<i64> {
		let v = self.uvar()?;
		Some((v >> 1) as i64 ^ -((v & 1) as i64))
	}

	pub(crate) fn packed(&mut self) -> Option<Vec<u32>> {
		let width = u32::try_from(self.uvar()?)
			.ok()
			.filter(|&width| width <= 32)?;
		let len = usize::try_from(self.uvar()?).ok()?;
		if width == 0 {
			return Some(vec![0; len]);
		}
		let bytes = self.bytes(len.checked_mul(width as usize)?.div_ceil(8))?;
		let mask = (1u64 << width) - 1;
		let (mut acc, mut bits) = (0u64, 0);
		let mut bytes = bytes.iter();
		let mut values = Vec::with_capacity(len);
		for _ in 0..len {
			while bits < width {
				acc |= u64::from(*bytes.next()?) << bits;
				bits += 8;
			}
			values.push((acc & mask) as u32);
			acc >>= width;
			bits -= width;
		}
		Some(values)
	}

	pub(crate) fn str(&mut self) -> Option<String> {
		let len = usize::try_from(self.uvar()?).ok()?;
		String::from_utf8(self.bytes(len)?.to_vec()).ok()
	}
}
