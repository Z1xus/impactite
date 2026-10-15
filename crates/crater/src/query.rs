use std::collections::HashSet;
use std::ops::RangeInclusive;
use std::sync::Arc;

use crate::dict::Dict;
use crate::event::Dim;
use crate::rollup::Metrics;

pub(crate) const MISSING: u32 = u32::MAX;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Field {
	Dim(Dim),
	Entry,
	Exit,
	Prop(String),
	// scroll depth of at least the first value, matched on engagement events
	Scroll,
	Currency,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Slot {
	Dim(Dim),
	Entry,
	Exit,
	Prop(u32),
	Scroll,
	Currency,
}

impl Slot {
	pub(crate) fn resolve(field: &Field, dict: &Dict) -> Self {
		match field {
			Field::Dim(dim) => Slot::Dim(*dim),
			Field::Entry => Slot::Entry,
			Field::Exit => Slot::Exit,
			Field::Prop(key) => Slot::Prop(dict.find(key).unwrap_or(MISSING)),
			Field::Scroll => Slot::Scroll,
			Field::Currency => Slot::Currency,
		}
	}

	pub(crate) fn is_event(self) -> bool {
		match self {
			Slot::Dim(dim) => dim.is_event(),
			Slot::Prop(_) | Slot::Scroll | Slot::Currency => true,
			Slot::Entry | Slot::Exit => false,
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Op {
	Is,
	IsNot,
	Contains,
	ContainsNot,
	Matches,
	MatchesNot,
}

#[derive(Debug, Clone, Hash)]
pub struct Filter {
	pub field: Field,
	pub op: Op,
	pub values: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Query {
	pub site: u64,
	pub days: RangeInclusive<i32>,
	pub filters: Vec<Filter>,
	pub breakdown: Option<Field>,
	pub limit: usize,
	pub daily: bool,
	pub imports: bool,
	// local seconds of day, keeps only later events and fills `Report::minutes`
	pub since: Option<u32>,
}

#[derive(Debug, Clone, Default)]
pub struct Report {
	pub total: Metrics,
	pub days: Vec<Metrics>,
	pub hours: [Metrics; 24],
	pub rows: Vec<(Arc<str>, Metrics)>,
	pub daily: Vec<Vec<(Arc<str>, Metrics)>>,
	pub minutes: Vec<u64>,
}

pub(crate) struct Matcher {
	pub(crate) slot: Slot,
	ids: HashSet<u32>,
	negate: bool,
	min: Option<u32>,
}

impl Matcher {
	pub(crate) fn new(filter: &Filter, dict: &Dict) -> Self {
		let any = |test: &dyn Fn(&str, &str) -> bool| {
			dict.matching(|s| filter.values.iter().any(|value| test(s, value)))
		};
		let ids = match filter.op {
			Op::Is | Op::IsNot => filter
				.values
				.iter()
				.filter_map(|value| dict.find(value))
				.collect(),
			Op::Contains | Op::ContainsNot => any(&|s, value| s.contains(value)),
			Op::Matches | Op::MatchesNot => any(&|s, pattern| glob(pattern, s)),
		};
		Self {
			slot: Slot::resolve(&filter.field, dict),
			ids,
			negate: matches!(filter.op, Op::IsNot | Op::ContainsNot | Op::MatchesNot),
			min: match filter.field {
				Field::Scroll => filter.values.first().and_then(|value| value.parse().ok()),
				_ => None,
			},
		}
	}

	pub(crate) fn matches(&self, id: u32) -> bool {
		match self.min {
			// scroll is stored as depth + 1
			Some(min) => (id > min) != self.negate,
			None => self.ids.contains(&id) != self.negate,
		}
	}
}

pub fn glob(pattern: &str, text: &str) -> bool {
	let mut parts = pattern.split('*');
	let first = parts.next().unwrap_or_default();
	let Some(mut rest) = text.strip_prefix(first) else {
		return false;
	};
	let parts: Vec<&str> = parts.collect();
	let Some((last, middle)) = parts.split_last() else {
		return rest.is_empty();
	};
	for part in middle {
		match rest.find(part) {
			Some(at) => rest = &rest[at + part.len()..],
			None => return false,
		}
	}
	rest.len() >= last.len() && rest.ends_with(last)
}
