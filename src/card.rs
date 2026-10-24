use std::io::Write;

use flate2::Compression;
use flate2::write::ZlibEncoder;

// departure mono at 11 px, 7 by 14 pixels per glyph, one byte per row, ascii 32 to 126
const FONT: &[u8; 1330] = include_bytes!("card/font.bin");
const BAYER: [u8; 16] = [0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5];

pub type Color = [u8; 3];
pub const INK: Color = [0x06, 0x09, 0x0b];
pub const FG: Color = [0xe3, 0xee, 0xf0];
pub const DIM: Color = [0x87, 0x98, 0xa0];
pub const FAINT: Color = [0x64, 0x75, 0x7e];
pub const CYAN: Color = [0x38, 0xe1, 0xf2];
pub const DEEP: Color = [0x0e, 0x7f, 0x8d];
pub const BAD: Color = [0xf0, 0x64, 0x5a];

pub struct Canvas {
	width: usize,
	height: usize,
	pixels: Vec<Color>,
}

impl Canvas {
	pub fn new(width: usize, height: usize, background: Color) -> Self {
		Self {
			width,
			height,
			pixels: vec![background; width * height],
		}
	}

	pub fn rect(&mut self, x: usize, y: usize, width: usize, height: usize, color: Color) {
		for row in y..(y + height).min(self.height) {
			for col in x..(x + width).min(self.width) {
				self.pixels[row * self.width + col] = color;
			}
		}
	}

	// draws text with its top left at x, y, each font pixel `scale` wide, and returns the width
	pub fn text(&mut self, x: usize, y: usize, scale: usize, color: Color, text: &str) -> usize {
		for (i, ch) in text.chars().enumerate() {
			let code = (ch as usize).wrapping_sub(32);
			let Some(glyph) = FONT.get(code * 14..code * 14 + 14) else {
				continue;
			};
			for (row, bits) in glyph.iter().enumerate() {
				for col in 0..7 {
					if bits & (1 << col) != 0 {
						self.rect(
							x + (i * 7 + col) * scale,
							y + row * scale,
							scale,
							scale,
							color,
						);
					}
				}
			}
		}
		text.chars().count() * 7 * scale
	}

	pub fn area(&mut self, [x, y, width, height]: [usize; 4], values: &[u64], cell: usize) {
		let max = values.iter().copied().max().unwrap_or(0).max(1) as f64;
		let (cols, rows) = (width / cell, height / cell);
		for col in 0..cols {
			let value = values
				.get(col * values.len() / cols.max(1))
				.copied()
				.unwrap_or(0);
			let top = rows - ((value as f64 / max) * (rows - 1) as f64).round() as usize - 1;
			for row in top..rows {
				let depth = (row - top) as f64 / rows as f64;
				let color = if row == top {
					Some(CYAN)
				} else {
					let level = 0.5 * (1.0 - depth * 0.6) * 16.0;
					(level > f64::from(BAYER[(row % 4) * 4 + col % 4]) + 0.5).then_some(DEEP)
				};
				if let Some(color) = color {
					self.rect(x + col * cell, y + row * cell, cell, cell, color);
				}
			}
		}
	}

	pub fn png(&self) -> Vec<u8> {
		let mut raw = Vec::with_capacity((self.width * 3 + 1) * self.height);
		for row in self.pixels.chunks(self.width) {
			raw.push(0);
			raw.extend(row.iter().flatten());
		}
		let mut zlib = ZlibEncoder::new(Vec::new(), Compression::default());
		let _ = zlib.write_all(&raw);
		let data = zlib.finish().unwrap_or_default();
		let mut header = Vec::with_capacity(13);
		header.extend((self.width as u32).to_be_bytes());
		header.extend((self.height as u32).to_be_bytes());
		header.extend([8, 2, 0, 0, 0]);
		let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
		for (kind, body) in [
			(b"IHDR", header.as_slice()),
			(b"IDAT", &data),
			(b"IEND", &[]),
		] {
			png.extend((body.len() as u32).to_be_bytes());
			let start = png.len();
			png.extend(kind);
			png.extend(body);
			png.extend(crc32fast::hash(&png[start..]).to_be_bytes());
		}
		png
	}
}
