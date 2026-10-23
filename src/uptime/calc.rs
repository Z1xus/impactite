use serde_json::{Value, json};

use crate::uptime::State;

#[derive(Default)]
pub struct Spans {
	pub incidents: Vec<(i64, Option<i64>)>,
	skipped: Vec<(i64, i64)>,
}

impl Spans {
	pub fn new(incidents: Vec<(i64, Option<i64>)>, mut skipped: Vec<(i64, i64)>) -> Self {
		skipped.sort_unstable();
		let mut merged: Vec<(i64, i64)> = Vec::new();
		for (start, end) in skipped {
			match merged.last_mut() {
				Some(last) if start <= last.1 => last.1 = last.1.max(end),
				_ => merged.push((start, end)),
			}
		}
		Self {
			incidents,
			skipped: merged,
		}
	}
}

fn overlap(a: (i64, i64), b: (i64, i64)) -> i64 {
	(a.1.min(b.1) - a.0.max(b.0)).max(0)
}

pub fn uptime(spans: &Spans, created: i64, from: i64, to: i64) -> Option<f64> {
	let window = (from.max(created), to);
	let paused: i64 = spans
		.skipped
		.iter()
		.map(|skip| overlap(*skip, window))
		.sum();
	let watched = window.1 - window.0 - paused;
	if watched < 60 {
		return None;
	}
	let down: i64 = spans
		.incidents
		.iter()
		.map(|(start, end)| {
			let part = (*start.max(&window.0), end.unwrap_or(to).min(window.1));
			let covered: i64 = spans.skipped.iter().map(|skip| overlap(*skip, part)).sum();
			(part.1 - part.0).max(0) - covered
		})
		.sum();
	Some((1.0 - down as f64 / watched as f64).clamp(0.0, 1.0))
}

pub fn round(fraction: f64) -> f64 {
	(fraction * 100_000.0).round() / 100_000.0
}

pub fn strip(spans: &Spans, created: i64, now: i64, slow: &[bool; 90]) -> Vec<Value> {
	let today = now / 86_400 * 86_400;
	(0..90)
		.map(|i| {
			let start = today - (89 - i as i64) * 86_400;
			let end = (start + 86_400).min(now);
			match uptime(spans, created, start, end) {
				None => Value::Null,
				Some(fraction) => {
					let worst = if fraction < 1.0 {
						State::Down
					} else if slow[i] {
						State::Slow
					} else {
						State::Up
					};
					let incidents = spans
						.incidents
						.iter()
						.filter(|(from, to)| *from < end && to.unwrap_or(now) > start)
						.count();
					json!([round(fraction), worst, incidents])
				}
			}
		})
		.collect()
}

pub fn span(secs: i64) -> String {
	let secs = secs.max(0);
	let (days, hours, minutes) = (secs / 86_400, secs % 86_400 / 3600, secs % 3600 / 60);
	let join = |big: i64, unit: &str, small: i64, small_unit: &str| {
		if small == 0 {
			format!("{big} {unit}")
		} else {
			format!("{big} {unit} {small} {small_unit}")
		}
	};
	if days > 0 {
		join(days, "d", hours, "h")
	} else if hours > 0 {
		join(hours, "h", minutes, "min")
	} else if minutes > 0 {
		format!("{minutes} min")
	} else {
		format!("{secs} s")
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn skipped_time_leaves_the_watched_time_and_the_incident() {
		// created at 1000, watched to 2000, paused 1200 to 1300, maintenance 1250 to 1400 covers part of the incident
		let spans = Spans::new(
			vec![(1350, Some(1450)), (1900, None)],
			vec![(1200, 1300), (1250, 1400)],
		);
		// watched 1000 to 2000 minus 200 skipped, down 50 + 100
		let fraction = uptime(&spans, 1000, 0, 2000).unwrap();
		assert!((fraction - (1.0 - 150.0 / 800.0)).abs() < 1e-9);
		assert_eq!(uptime(&spans, 1000, 0, 1030), None);
	}
}
