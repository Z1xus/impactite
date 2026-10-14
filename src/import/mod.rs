use crater::{Db, Event};

pub mod clickhouse;
pub mod goatcounter;

fn insert(db: &Db, event: Event) {
	let mut event = Box::new(event);
	while let Err(back) = db.insert(event) {
		event = back;
		std::thread::yield_now();
	}
}
