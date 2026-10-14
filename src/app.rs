use std::path::Path;
use std::sync::{Mutex, MutexGuard, PoisonError};

use rusqlite::{Connection, OptionalExtension, params};

const MIGRATIONS: &[&str] = &[
	"
	create table sites (
		id integer primary key,
		domain text not null unique,
		timezone text not null,
		created_at integer not null
	);
	create table salts (
		site integer not null references sites (id) on delete cascade,
		day integer not null,
		salt blob not null,
		primary key (site, day)
	);
	",
	"
	create table api_keys (
		hash text primary key,
		name text not null,
		prefix text not null,
		created_at integer not null
	);
	",
];

#[derive(Debug, Clone)]
pub struct Site {
	pub id: u64,
	pub domain: String,
	pub timezone: String,
	pub created_at: i64,
}

pub struct App {
	conn: Mutex<Connection>,
}

impl App {
	pub fn open(path: &Path) -> rusqlite::Result<Self> {
		let mut conn = Connection::open(path)?;
		conn.pragma_update(None, "journal_mode", "wal")?;
		conn.pragma_update(None, "foreign_keys", "on")?;
		migrate(&mut conn)?;
		Ok(Self {
			conn: Mutex::new(conn),
		})
	}

	fn conn(&self) -> MutexGuard<'_, Connection> {
		self.conn.lock().unwrap_or_else(PoisonError::into_inner)
	}

	pub fn add_site(&self, domain: &str, timezone: &str) -> rusqlite::Result<Site> {
		let conn = self.conn();
		conn.execute(
			"insert into sites (domain, timezone, created_at) values (?1, ?2, unixepoch())",
			params![domain, timezone],
		)?;
		Ok(Site {
			id: conn.last_insert_rowid() as u64,
			domain: domain.to_owned(),
			timezone: timezone.to_owned(),
			created_at: jiff::Timestamp::now().as_second(),
		})
	}

	pub fn site(&self, domain: &str) -> rusqlite::Result<Option<Site>> {
		self.conn()
			.query_row(
				"select id, domain, timezone, created_at from sites where domain = ?1",
				[domain],
				site,
			)
			.optional()
	}

	pub fn sites(&self) -> rusqlite::Result<Vec<Site>> {
		let conn = self.conn();
		let mut stmt =
			conn.prepare("select id, domain, timezone, created_at from sites order by id")?;
		stmt.query_map([], site)?.collect()
	}

	pub fn salt(&self, site: u64, day: i32) -> rusqlite::Result<[u8; 32]> {
		let conn = self.conn();
		let mut fresh = [0; 32];
		getrandom::fill(&mut fresh)
			.map_err(|err| rusqlite::Error::ToSqlConversionFailure(err.into()))?;
		conn.execute(
			"insert or ignore into salts (site, day, salt) values (?1, ?2, ?3)",
			params![site as i64, day, fresh],
		)?;
		conn.execute(
			"delete from salts where site = ?1 and day < ?2 - 1",
			params![site as i64, day],
		)?;
		conn.query_row(
			"select salt from salts where site = ?1 and day = ?2",
			params![site as i64, day],
			|row| row.get(0),
		)
	}

	pub fn create_key(&self, name: &str) -> rusqlite::Result<String> {
		let mut secret = [0; 32];
		getrandom::fill(&mut secret)
			.map_err(|err| rusqlite::Error::ToSqlConversionFailure(err.into()))?;
		let key: String = secret.iter().map(|byte| format!("{byte:02x}")).collect();
		self.conn().execute(
			"insert into api_keys (hash, name, prefix, created_at) values (?1, ?2, ?3, unixepoch())",
			params![key_hash(&key), name, &key[..8]],
		)?;
		Ok(key)
	}

	pub fn key_valid(&self, key: &str) -> rusqlite::Result<bool> {
		self.conn()
			.query_row(
				"select 1 from api_keys where hash = ?1",
				[key_hash(key)],
				|_| Ok(()),
			)
			.optional()
			.map(|found| found.is_some())
	}
}

fn key_hash(key: &str) -> String {
	blake3::hash(key.as_bytes()).to_hex().to_string()
}

fn site(row: &rusqlite::Row) -> rusqlite::Result<Site> {
	Ok(Site {
		id: row.get::<_, i64>(0)? as u64,
		domain: row.get(1)?,
		timezone: row.get(2)?,
		created_at: row.get(3)?,
	})
}

fn migrate(conn: &mut Connection) -> rusqlite::Result<()> {
	let version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
	for (i, sql) in MIGRATIONS.iter().enumerate().skip(version as usize) {
		let tx = conn.transaction()?;
		tx.execute_batch(sql)?;
		tx.pragma_update(None, "user_version", i as i64 + 1)?;
		tx.commit()?;
	}
	Ok(())
}
