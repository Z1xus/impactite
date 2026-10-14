use std::ops::{Index, IndexMut};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dim {
	Name,
	Hostname,
	Page,
	Referrer,
	Source,
	Channel,
	UtmSource,
	UtmMedium,
	UtmCampaign,
	UtmContent,
	UtmTerm,
	Country,
	Region,
	City,
	Device,
	Browser,
	BrowserVersion,
	Os,
	OsVersion,
}

impl Dim {
	pub const ALL: [Dim; 19] = [
		Dim::Name,
		Dim::Hostname,
		Dim::Page,
		Dim::Referrer,
		Dim::Source,
		Dim::Channel,
		Dim::UtmSource,
		Dim::UtmMedium,
		Dim::UtmCampaign,
		Dim::UtmContent,
		Dim::UtmTerm,
		Dim::Country,
		Dim::Region,
		Dim::City,
		Dim::Device,
		Dim::Browser,
		Dim::BrowserVersion,
		Dim::Os,
		Dim::OsVersion,
	];
	pub const COUNT: usize = Self::ALL.len();

	pub const NAMES: [&str; 19] = [
		"name",
		"hostname",
		"page",
		"referrer",
		"source",
		"channel",
		"utm_source",
		"utm_medium",
		"utm_campaign",
		"utm_content",
		"utm_term",
		"country",
		"region",
		"city",
		"device",
		"browser",
		"browser_version",
		"os",
		"os_version",
	];

	pub fn name(self) -> &'static str {
		Self::NAMES[self as usize]
	}

	pub fn is_event(self) -> bool {
		matches!(self, Dim::Name | Dim::Hostname | Dim::Page)
	}
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Dims([String; Dim::COUNT]);

impl Index<Dim> for Dims {
	type Output = String;

	fn index(&self, dim: Dim) -> &String {
		&self.0[dim as usize]
	}
}

impl IndexMut<Dim> for Dims {
	fn index_mut(&mut self, dim: Dim) -> &mut String {
		&mut self.0[dim as usize]
	}
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Event {
	pub site: u64,
	pub ts: i64,
	pub offset: i32,
	pub visitor: u64,
	pub session: u64,
	pub interactive: bool,
	pub dims: Dims,
	pub scroll_depth: Option<u8>,
	pub engagement_ms: u32,
	pub props: Vec<(String, String)>,
	// hundredths of the currency unit
	pub revenue: Option<(u32, String)>,
}

impl Event {
	pub fn day(&self) -> i32 {
		(self.ts + i64::from(self.offset)).div_euclid(86_400) as i32
	}
}
