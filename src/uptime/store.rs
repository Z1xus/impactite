use std::collections::HashMap;

use rusqlite::{OptionalExtension, Row, params};
use serde_json::{Value, json};

use crate::app::App;
use crate::uptime::calc::Spans;
use crate::uptime::{Config, Kind, Monitor};

const MONITOR: &str = "m.id, m.team, m.site, m.name, m.kind, m.target, m.config, m.interval,
	m.paused, m.muted, m.token, m.created_by, m.created_at";

pub enum Scope {
	Team(i64),
	Monitor(i64),
	Page(i64),
}

const PAGE: &str = "m.team = (select team from status_pages where id = ?1)
	and m.id in (select value from json_each((select monitors from status_pages where id = ?1)))";

impl Scope {
	fn filter(&self) -> (&'static str, i64) {
		match self {
			Self::Team(id) => ("m.team = ?1", *id),
			Self::Monitor(id) => ("m.id = ?1", *id),
			Self::Page(id) => (PAGE, *id),
		}
	}
}

type Listed = (i64, i64, i64, Option<i64>, String);

pub type Incident = (i64, i64, Option<i64>, String, String);

#[derive(Default)]
pub struct Past {
	pub incident: Option<(i64, i64)>,
	pub ended: Option<i64>,
	pub paused: Option<i64>,
	pub cert_until: i64,
	pub cert_warned: i64,
}

fn monitor(row: &Row) -> rusqlite::Result<Monitor> {
	let config: String = row.get(6)?;
	Ok(Monitor {
		id: row.get(0)?,
		team: row.get(1)?,
		site: row.get(2)?,
		name: row.get(3)?,
		kind: Kind::parse(&row.get::<_, String>(4)?).unwrap_or(Kind::Http),
		target: row.get(5)?,
		config: serde_json::from_str(&config).unwrap_or_default(),
		interval: row.get(7)?,
		paused: row.get(8)?,
		muted: row.get(9)?,
		token: row.get(10)?,
		created_by: row.get(11)?,
		created_at: row.get(12)?,
	})
}

fn config(config: &Config) -> String {
	serde_json::to_string(config).unwrap_or_default()
}

impl App {
	pub fn monitors(&self) -> rusqlite::Result<Vec<Monitor>> {
		let conn = self.conn();
		let mut stmt = conn.prepare(&format!("select {MONITOR} from monitors m"))?;
		stmt.query_map([], monitor)?.collect()
	}

	pub fn monitor(&self, id: i64) -> rusqlite::Result<Option<Monitor>> {
		self.conn()
			.query_row(
				&format!("select {MONITOR} from monitors m where m.id = ?1"),
				[id],
				monitor,
			)
			.optional()
	}

	pub fn team_monitors(&self, team: i64) -> rusqlite::Result<Vec<(Monitor, Option<String>)>> {
		let conn = self.conn();
		let mut stmt = conn.prepare(&format!(
			"select {MONITOR}, s.domain from monitors m left join sites s on s.id = m.site
			where m.team = ?1 order by m.id"
		))?;
		stmt.query_map([team], |row| Ok((monitor(row)?, row.get(13)?)))?
			.collect()
	}

	pub fn site_monitor(&self, site: u64) -> rusqlite::Result<Option<Monitor>> {
		self.conn()
			.query_row(
				&format!(
					"select {MONITOR} from monitors m where m.site = ?1 order by m.id limit 1"
				),
				[site as i64],
				monitor,
			)
			.optional()
	}

	pub fn monitor_domain(&self, site: i64) -> Option<String> {
		self.conn()
			.query_row("select domain from sites where id = ?1", [site], |row| {
				row.get(0)
			})
			.ok()
	}

	pub fn add_monitor(&self, m: &Monitor) -> rusqlite::Result<i64> {
		let conn = self.conn();
		conn.execute(
			"insert into monitors (team, name, kind, target, config, interval, muted, token, created_by, created_at)
			values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, unixepoch())",
			params![
				m.team,
				m.name,
				m.kind.as_str(),
				m.target,
				config(&m.config),
				m.interval,
				m.muted,
				m.token,
				m.created_by
			],
		)?;
		Ok(conn.last_insert_rowid())
	}

	pub fn save_monitor(&self, m: &Monitor) -> rusqlite::Result<()> {
		self.conn().execute(
			"update monitors set name = ?2, target = ?3, config = ?4, interval = ?5, muted = ?6, created_by = ?7 where id = ?1",
			params![
				m.id,
				m.name,
				m.target,
				config(&m.config),
				m.interval,
				m.muted,
				m.created_by
			],
		)?;
		Ok(())
	}

	pub fn remove_monitor(&self, id: i64) -> rusqlite::Result<()> {
		self.conn()
			.execute("delete from monitors where id = ?1", [id])?;
		Ok(())
	}

	pub fn monitor_of_token(&self, token: &str) -> Option<i64> {
		self.conn()
			.query_row("select id from monitors where token = ?1", [token], |row| {
				row.get(0)
			})
			.ok()
	}

	pub fn set_paused(&self, id: i64, paused: bool) -> rusqlite::Result<bool> {
		let mut conn = self.conn();
		let tx = conn.transaction()?;
		let changed = tx.execute(
			"update monitors set paused = ?2 where id = ?1 and paused != ?2",
			params![id, paused],
		)? > 0;
		if changed && paused {
			tx.execute(
				"insert into pauses (monitor, from_at) values (?1, unixepoch())",
				[id],
			)?;
		} else if changed {
			tx.execute(
				"update pauses set to_at = unixepoch() where monitor = ?1 and to_at is null",
				[id],
			)?;
		}
		tx.commit()?;
		Ok(changed)
	}

	pub fn pasts(&self) -> rusqlite::Result<HashMap<i64, Past>> {
		let conn = self.conn();
		let mut stmt = conn.prepare(
			"select m.id,
			(select id from incidents where monitor = m.id and ended_at is null order by id desc limit 1),
			(select started_at from incidents where monitor = m.id and ended_at is null order by id desc limit 1),
			(select max(ended_at) from incidents where monitor = m.id),
			(select max(from_at) from pauses where monitor = m.id and to_at is null),
			m.cert_until, m.cert_warned
			from monitors m",
		)?;
		stmt.query_map([], |row| {
			let incident: Option<i64> = row.get(1)?;
			let started: Option<i64> = row.get(2)?;
			Ok((
				row.get(0)?,
				Past {
					incident: incident.zip(started),
					ended: row.get(3)?,
					paused: row.get(4)?,
					cert_until: row.get(5)?,
					cert_warned: row.get(6)?,
				},
			))
		})?
		.collect()
	}

	pub fn set_cert(&self, monitor: i64, until: i64, warned: i64) -> rusqlite::Result<()> {
		self.conn().execute(
			"update monitors set cert_until = ?2, cert_warned = ?3 where id = ?1",
			params![monitor, until, warned],
		)?;
		Ok(())
	}

	pub fn open_incident(
		&self,
		monitor: i64,
		started: i64,
		cause: &str,
		detail: &str,
		notified: bool,
	) -> rusqlite::Result<i64> {
		let conn = self.conn();
		conn.execute(
			"insert into incidents (monitor, started_at, cause, detail, notified) values (?1, ?2, ?3, ?4, ?5)",
			params![monitor, started, cause, detail, notified],
		)?;
		Ok(conn.last_insert_rowid())
	}

	pub fn close_incident(&self, id: i64, ended: i64) -> rusqlite::Result<()> {
		self.conn().execute(
			"update incidents set ended_at = ?2 where id = ?1",
			params![id, ended],
		)?;
		Ok(())
	}

	pub fn incident_notified(&self, id: i64) -> bool {
		self.conn()
			.query_row(
				"select notified from incidents where id = ?1",
				[id],
				|row| row.get(0),
			)
			.unwrap_or(false)
	}

	pub fn spans(&self, scope: &Scope, since: i64) -> rusqlite::Result<HashMap<i64, Spans>> {
		let (filter, id) = scope.filter();
		let conn = self.conn();
		let mut incidents: HashMap<i64, Vec<(i64, Option<i64>)>> = HashMap::new();
		let mut skipped: HashMap<i64, Vec<(i64, i64)>> = HashMap::new();
		let mut stmt = conn.prepare(&format!(
			"select i.monitor, i.started_at, i.ended_at from incidents i join monitors m on m.id = i.monitor
			where {filter} and (i.ended_at is null or i.ended_at > ?2) order by i.started_at"
		))?;
		for row in stmt.query_map(params![id, since], |row| {
			Ok((row.get(0)?, (row.get(1)?, row.get(2)?)))
		})? {
			let (monitor, span) = row?;
			incidents.entry(monitor).or_default().push(span);
		}
		let mut stmt = conn.prepare(&format!(
			"select p.monitor, p.from_at, coalesce(p.to_at, 9223372036854775807) from pauses p
			join monitors m on m.id = p.monitor where {filter} and (p.to_at is null or p.to_at > ?2)
			union all
			select mm.monitor, x.starts_at, x.ends_at from maintenance_monitors mm
			join maintenance x on x.id = mm.maintenance join monitors m on m.id = mm.monitor
			where {filter} and x.ends_at > ?2"
		))?;
		for row in stmt.query_map(params![id, since], |row| {
			Ok((row.get(0)?, (row.get(1)?, row.get(2)?)))
		})? {
			let (monitor, span) = row?;
			skipped.entry(monitor).or_default().push(span);
		}
		let mut out: HashMap<i64, Spans> = HashMap::new();
		for (monitor, list) in incidents {
			let skip = skipped.remove(&monitor).unwrap_or_default();
			out.insert(monitor, Spans::new(list, skip));
		}
		for (monitor, skip) in skipped {
			out.insert(monitor, Spans::new(Vec::new(), skip));
		}
		Ok(out)
	}

	pub fn page_monitors(&self, page: i64) -> rusqlite::Result<Vec<Monitor>> {
		let conn = self.conn();
		let mut stmt = conn.prepare(&format!("select {MONITOR} from monitors m where {PAGE}"))?;
		stmt.query_map([page], monitor)?.collect()
	}

	pub fn page_incidents(&self, page: i64, since: i64) -> rusqlite::Result<Vec<Listed>> {
		let conn = self.conn();
		let mut stmt = conn.prepare(&format!(
			"select i.id, i.monitor, i.started_at, i.ended_at, i.cause from incidents i
			join monitors m on m.id = i.monitor
			where {PAGE} and (i.ended_at is null or i.ended_at > ?2) order by i.started_at desc"
		))?;
		stmt.query_map(params![page, since], |row| {
			Ok((
				row.get(0)?,
				row.get(1)?,
				row.get(2)?,
				row.get(3)?,
				row.get(4)?,
			))
		})?
		.collect()
	}

	pub fn public_notes(&self, incidents: &[i64]) -> rusqlite::Result<HashMap<i64, Vec<Value>>> {
		let conn = self.conn();
		let mut stmt = conn.prepare(
			"select at, text from incident_notes where incident = ?1 and public = 1 order by at",
		)?;
		let mut out = HashMap::new();
		for incident in incidents {
			let notes = stmt
				.query_map([incident], |row| {
					Ok(json!({ "at": row.get::<_, i64>(0)?, "text": row.get::<_, String>(1)? }))
				})?
				.collect::<rusqlite::Result<Vec<_>>>()?;
			if !notes.is_empty() {
				out.insert(*incident, notes);
			}
		}
		Ok(out)
	}

	pub fn monitor_incidents(&self, monitor: i64, since: i64) -> rusqlite::Result<Vec<Incident>> {
		let conn = self.conn();
		let mut stmt = conn.prepare(
			"select id, started_at, ended_at, cause, detail from incidents
			where monitor = ?1 and (ended_at is null or ended_at > ?2) order by started_at desc",
		)?;
		stmt.query_map(params![monitor, since], |row| {
			Ok((
				row.get(0)?,
				row.get(1)?,
				row.get(2)?,
				row.get(3)?,
				row.get(4)?,
			))
		})?
		.collect()
	}

	pub fn incidents_between(
		&self,
		monitor: i64,
		from: i64,
		to: i64,
	) -> rusqlite::Result<Vec<Incident>> {
		let conn = self.conn();
		let mut stmt = conn.prepare(
			"select id, started_at, ended_at, cause, detail from incidents
			where monitor = ?1 and started_at < ?3 and (ended_at is null or ended_at > ?2)
			order by started_at",
		)?;
		stmt.query_map(params![monitor, from, to], |row| {
			Ok((
				row.get(0)?,
				row.get(1)?,
				row.get(2)?,
				row.get(3)?,
				row.get(4)?,
			))
		})?
		.collect()
	}

	pub fn notes(&self, incidents: &[i64]) -> rusqlite::Result<HashMap<i64, Vec<Value>>> {
		let conn = self.conn();
		let mut stmt = conn.prepare(
			"select id, at, text, public from incident_notes where incident = ?1 order by at",
		)?;
		let mut out = HashMap::new();
		for incident in incidents {
			let notes = stmt
				.query_map([incident], |row| {
					Ok(note(
						row.get(0)?,
						row.get(1)?,
						&row.get::<_, String>(2)?,
						row.get(3)?,
					))
				})?
				.collect::<rusqlite::Result<Vec<_>>>()?;
			if !notes.is_empty() {
				out.insert(*incident, notes);
			}
		}
		Ok(out)
	}

	pub fn incident_team(&self, incident: i64) -> Option<i64> {
		self.conn()
			.query_row(
				"select m.team from incidents i join monitors m on m.id = i.monitor where i.id = ?1",
				[incident],
				|row| row.get(0),
			)
			.ok()
	}

	pub fn add_note(&self, incident: i64, text: &str, public: bool) -> rusqlite::Result<Value> {
		let conn = self.conn();
		conn.execute(
			"insert into incident_notes (incident, at, text, public) values (?1, unixepoch(), ?2, ?3)",
			params![incident, text, public],
		)?;
		let id = conn.last_insert_rowid();
		conn.query_row(
			"select id, at, text, public from incident_notes where id = ?1",
			[id],
			|row| {
				Ok(note(
					row.get(0)?,
					row.get(1)?,
					&row.get::<_, String>(2)?,
					row.get(3)?,
				))
			},
		)
	}

	pub fn remove_note(&self, incident: i64, id: i64) -> rusqlite::Result<()> {
		self.conn().execute(
			"delete from incident_notes where id = ?1 and incident = ?2",
			params![id, incident],
		)?;
		Ok(())
	}

	pub fn windows(&self, now: i64) -> rusqlite::Result<HashMap<i64, Vec<(i64, i64)>>> {
		let conn = self.conn();
		let mut stmt = conn.prepare(
			"select mm.monitor, x.starts_at, x.ends_at from maintenance_monitors mm
			join maintenance x on x.id = mm.maintenance where x.ends_at > ?1",
		)?;
		let mut out: HashMap<i64, Vec<(i64, i64)>> = HashMap::new();
		for row in stmt.query_map([now], |row| Ok((row.get(0)?, (row.get(1)?, row.get(2)?))))? {
			let (monitor, window) = row?;
			out.entry(monitor).or_default().push(window);
		}
		Ok(out)
	}

	pub fn add_maintenance(
		&self,
		team: i64,
		starts: i64,
		ends: i64,
		text: &str,
		monitors: &[i64],
	) -> rusqlite::Result<()> {
		let mut conn = self.conn();
		let tx = conn.transaction()?;
		tx.execute(
			"insert into maintenance (team, starts_at, ends_at, note) values (?1, ?2, ?3, ?4)",
			params![team, starts, ends, text],
		)?;
		let id = tx.last_insert_rowid();
		for monitor in monitors {
			tx.execute(
				"insert or ignore into maintenance_monitors (maintenance, monitor) values (?1, ?2)",
				params![id, monitor],
			)?;
		}
		tx.commit()
	}

	pub fn maintenance(&self, scope: &Scope, since: i64) -> rusqlite::Result<Vec<Value>> {
		let (filter, id) = match scope {
			Scope::Team(id) => ("x.team = ?1", *id),
			Scope::Monitor(id) => (
				"x.id in (select maintenance from maintenance_monitors where monitor = ?1)",
				*id,
			),
			Scope::Page(id) => (
				"x.team = (select team from status_pages where id = ?1) and x.id in
				(select maintenance from maintenance_monitors where monitor in
				(select value from json_each((select monitors from status_pages where id = ?1))))",
				*id,
			),
		};
		let conn = self.conn();
		let mut stmt = conn.prepare(&format!(
			"select x.id, x.starts_at, x.ends_at, x.note,
			(select group_concat(monitor) from maintenance_monitors where maintenance = x.id)
			from maintenance x where {filter} and x.ends_at > ?2 order by x.starts_at"
		))?;
		stmt.query_map(params![id, since], |row| {
			let monitors: Option<String> = row.get(4)?;
			let monitors: Vec<i64> = monitors
				.iter()
				.flat_map(|list| list.split(','))
				.filter_map(|id| id.parse().ok())
				.collect();
			Ok(json!({
				"id": row.get::<_, i64>(0)?,
				"starts_at": row.get::<_, i64>(1)?,
				"ends_at": row.get::<_, i64>(2)?,
				"note": row.get::<_, String>(3)?,
				"monitors": monitors,
			}))
		})?
		.collect()
	}

	pub fn remove_maintenance(&self, team: i64, id: i64) -> rusqlite::Result<()> {
		self.conn().execute(
			"delete from maintenance where id = ?1 and team = ?2",
			params![id, team],
		)?;
		Ok(())
	}

	pub fn alerts(&self, team: i64) -> rusqlite::Result<Vec<Alert>> {
		let conn = self.conn();
		let mut stmt = conn.prepare(
			"select id, target, sent_at, failure from alerts where team = ?1 order by id",
		)?;
		stmt.query_map([team], |row| {
			Ok(Alert {
				id: row.get(0)?,
				target: row.get(1)?,
				sent_at: row.get(2)?,
				failure: row.get(3)?,
			})
		})?
		.collect()
	}

	pub fn mark_alert(&self, id: i64, failure: &str) -> rusqlite::Result<()> {
		self.conn().execute(
			"update alerts set sent_at = unixepoch(), failure = ?2 where id = ?1",
			params![id, failure],
		)?;
		Ok(())
	}

	pub fn alert(&self, team: i64, id: i64) -> Option<String> {
		self.conn()
			.query_row(
				"select target from alerts where id = ?1 and team = ?2",
				params![id, team],
				|row| row.get(0),
			)
			.ok()
	}

	pub fn add_alert(&self, team: i64, target: &str) -> rusqlite::Result<()> {
		let conn = self.conn();
		let known: bool = conn.query_row(
			"select exists (select 1 from alerts where team = ?1 and target = ?2)",
			params![team, target],
			|row| row.get(0),
		)?;
		if !known {
			conn.execute(
				"insert into alerts (team, target, created_at) values (?1, ?2, unixepoch())",
				params![team, target],
			)?;
		}
		Ok(())
	}

	pub fn remove_alert(&self, team: i64, id: i64) -> rusqlite::Result<()> {
		self.conn().execute(
			"delete from alerts where id = ?1 and team = ?2",
			params![id, team],
		)?;
		Ok(())
	}
}

pub struct Alert {
	pub id: i64,
	pub target: String,
	pub sent_at: Option<i64>,
	pub failure: String,
}

fn note(id: i64, at: i64, text: &str, public: bool) -> Value {
	json!({ "id": id, "at": at, "text": text, "public": public })
}
