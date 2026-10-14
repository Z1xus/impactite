use std::collections::HashSet;
use std::ops::RangeInclusive;
use std::sync::Arc;

use crate::dict::Dict;
use crate::event::Dim;
use crate::rollup::Metrics;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Op {
	Is,
	IsNot,
	Contains,
	ContainsNot,
}

#[derive(Debug, Clone, Hash)]
pub struct Filter {
	pub dim: Dim,
	pub op: Op,
	pub values: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Query {
	pub site: u64,
	pub days: RangeInclusive<i32>,
	pub filters: Vec<Filter>,
	pub breakdown: Option<Dim>,
	pub limit: usize,
}

#[derive(Debug, Clone, Default)]
pub struct Report {
	pub total: Metrics,
	pub days: Vec<Metrics>,
	pub hours: [Metrics; 24],
	pub rows: Vec<(Arc<str>, Metrics)>,
}

pub(crate) struct Matcher {
	pub(crate) dim: Dim,
	ids: HashSet<u32>,
	negate: bool,
}

impl Matcher {
	pub(crate) fn new(filter: &Filter, dict: &Dict) -> Self {
		let ids = match filter.op {
			Op::Is | Op::IsNot => filter
				.values
				.iter()
				.filter_map(|value| dict.find(value))
				.collect(),
			Op::Contains | Op::ContainsNot => {
				dict.matching(|s| filter.values.iter().any(|value| s.contains(value.as_str())))
			}
		};
		Self {
			dim: filter.dim,
			ids,
			negate: matches!(filter.op, Op::IsNot | Op::ContainsNot),
		}
	}

	pub(crate) fn matches(&self, id: u32) -> bool {
		self.ids.contains(&id) != self.negate
	}
}
