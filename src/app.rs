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
	"
	create table users (
		id integer primary key,
		email text not null unique collate nocase,
		name text not null,
		password text not null,
		created_at integer not null
	);
	create table teams (
		id integer primary key,
		name text not null,
		personal integer not null default 0,
		created_at integer not null
	);
	create table members (
		team integer not null references teams (id) on delete cascade,
		user integer not null references users (id) on delete cascade,
		role text not null,
		primary key (team, user)
	);
	create table sessions (
		hash text primary key,
		user integer not null references users (id) on delete cascade,
		expires_at integer not null
	);
	create table invites (
		hash text primary key,
		team integer not null references teams (id) on delete cascade,
		email text not null,
		role text not null,
		expires_at integer not null
	);
	alter table sites add column team integer references teams (id) on delete cascade;
	",
	"
	create table passkeys (
		id blob primary key,
		user integer not null references users (id) on delete cascade,
		alg integer not null,
		key blob not null,
		counter integer not null default 0,
		name text not null,
		created_at integer not null,
		used_at integer
	);
	",
	"
	create table goals (
		id integer primary key,
		site integer not null references sites (id) on delete cascade,
		page integer not null,
		value text not null,
		name text not null,
		created_at integer not null,
		unique (site, page, value)
	);
	",
	"
	create table imports (
		id integer primary key,
		site integer not null references sites (id) on delete cascade,
		source text not null,
		first_day text not null,
		last_day text not null,
		created_at integer not null
	);
	",
	"
	alter table api_keys add column user integer references users (id) on delete cascade;
	",
	"
	create table guests (
		site integer not null references sites (id) on delete cascade,
		user integer not null references users (id) on delete cascade,
		role text not null,
		primary key (site, user)
	);
	alter table invites add column site integer references sites (id) on delete cascade;
	",
	"
	alter table sessions add column created_at integer not null default 0;
	alter table sessions add column seen_at integer not null default 0;
	alter table sessions add column device text not null default '';
	create table logins (
		hash text primary key,
		user integer not null references users (id) on delete cascade,
		expires_at integer not null
	);
	",
	"
	alter table users add column admin integer not null default 0;
	update users set admin = 1 where id = (select min(id) from users);
	create table settings (
		key text primary key,
		value text not null
	);
	",
	"
	create table segments (
		id integer primary key,
		site integer not null references sites (id) on delete cascade,
		user integer not null references users (id) on delete cascade,
		name text not null,
		filters text not null,
		shared integer not null,
		created_at integer not null
	);
	",
	"
	update sessions set created_at = expires_at - 2592000, seen_at = expires_at - 2592000
	where created_at = 0;
	",
	"
	create table prefs (
		user integer not null references users (id) on delete cascade,
		key text not null,
		value text not null,
		primary key (user, key)
	);
	",
	"
	create table goals_next (
		id integer primary key,
		site integer not null references sites (id) on delete cascade,
		page integer not null,
		value text not null,
		name text not null,
		scroll integer not null default 0,
		currency text not null default '',
		created_at integer not null,
		unique (site, page, value, scroll)
	);
	insert into goals_next (id, site, page, value, name, created_at)
	select id, site, page, value, name, created_at from goals;
	drop table goals;
	alter table goals_next rename to goals;
	",
	"
	create table funnels (
		id integer primary key,
		site integer not null references sites (id) on delete cascade,
		name text not null,
		steps text not null,
		created_at integer not null
	);
	",
	"
	alter table sites add column retention integer not null default 0;
	",
	"
	create table shields (
		id integer primary key,
		site integer not null references sites (id) on delete cascade,
		kind text not null,
		value text not null,
		unique (site, kind, value)
	);
	",
	"
	create table shares (
		id integer primary key,
		site integer not null references sites (id) on delete cascade,
		slug text not null unique,
		name text not null,
		password text not null,
		created_at integer not null
	);
	",
	"
	create table reports (
		id integer primary key,
		site integer not null references sites (id) on delete cascade,
		kind text not null,
		email text not null,
		threshold integer not null,
		last integer not null,
		unique (site, kind, email)
	);
	",
	"alter table sites add column counter integer not null default 0;",
	"
	create table titles (
		site integer not null references sites (id) on delete cascade,
		path text not null,
		title text not null,
		primary key (site, path)
	) without rowid;
	",
	"
	create table google (
		site integer primary key references sites (id) on delete cascade,
		refresh text not null,
		property text not null default ''
	);
	",
	"alter table sites add column skip text not null default '';",
	"
	alter table users add column totp text not null default '';
	alter table users add column totp_step integer not null default 0;
	alter table users add column recovery text not null default '';
	",
	"
	create table email_changes (
		hash text primary key,
		user integer not null references users (id) on delete cascade,
		email text not null,
		expires_at integer not null
	);
	",
	"
	create table ignored_events (
		site integer not null references sites (id) on delete cascade,
		name text not null,
		primary key (site, name)
	);
	",
	"
	create table monitors (
		id integer primary key,
		team integer not null references teams (id) on delete cascade,
		site integer references sites (id) on delete cascade,
		name text not null,
		kind text not null,
		target text not null,
		config text not null default '{\"v\":1}',
		interval integer not null default 60,
		paused integer not null default 0,
		muted integer not null default 0,
		token text unique,
		created_by integer references users (id) on delete set null,
		cert_until integer not null default 0,
		cert_warned integer not null default 0,
		created_at integer not null
	);
	create index monitors_team on monitors (team);
	create index monitors_site on monitors (site);
	create table pauses (
		monitor integer not null references monitors (id) on delete cascade,
		from_at integer not null,
		to_at integer
	);
	create index pauses_monitor on pauses (monitor);
	create table incidents (
		id integer primary key,
		monitor integer not null references monitors (id) on delete cascade,
		started_at integer not null,
		ended_at integer,
		cause text not null,
		detail text not null default '',
		notified integer not null default 0
	);
	create index incidents_monitor on incidents (monitor, started_at);
	create table incident_notes (
		id integer primary key,
		incident integer not null references incidents (id) on delete cascade,
		at integer not null,
		text text not null,
		public integer not null default 1
	);
	create index incident_notes_incident on incident_notes (incident);
	create table maintenance (
		id integer primary key,
		team integer not null references teams (id) on delete cascade,
		starts_at integer not null,
		ends_at integer not null,
		note text not null default ''
	);
	create table maintenance_monitors (
		maintenance integer not null references maintenance (id) on delete cascade,
		monitor integer not null references monitors (id) on delete cascade,
		primary key (maintenance, monitor)
	);
	create table alerts (
		id integer primary key,
		team integer not null references teams (id) on delete cascade,
		target text not null,
		created_at integer not null
	);
	create table status_pages (
		id integer primary key,
		team integer not null references teams (id) on delete cascade,
		slug text not null unique,
		name text not null,
		password text not null default '',
		monitors text not null default '[]',
		created_at integer not null
	);
	insert into monitors (team, site, name, kind, target, interval, created_at)
	select team, id, domain, 'http', 'https://' || domain || '/', 60, unixepoch()
	from sites where team is not null and domain not like '~%' and domain != 'localhost';
	",
	"
	alter table alerts add column sent_at integer;
	alter table alerts add column failure text not null default '';
	",
	"alter table status_pages add column look text not null default '{}';",
];

pub(crate) const HOME_MONITOR: &str = "
	insert into monitors (team, site, name, kind, target, interval, created_at)
	select team, id, domain, 'http', 'https://' || domain || '/', 60, unixepoch()
	from sites
	where (?1 = 0 or id = ?1) and team is not null and domain not like '~%' and domain != 'localhost'
	and not exists (select 1 from monitors where monitors.site = sites.id)";

const SITE_TABLES: [&str; 14] = [
	"monitors",
	"goals",
	"imports",
	"salts",
	"funnels",
	"shields",
	"shares",
	"reports",
	"segments",
	"titles",
	"guests",
	"google",
	"ignored_events",
	"invites",
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
		migrate(&mut conn, path)?;
		Ok(Self {
			conn: Mutex::new(conn),
		})
	}

	pub(crate) fn conn(&self) -> MutexGuard<'_, Connection> {
		lock(&self.conn)
	}

	pub fn add_site(
		&self,
		domain: &str,
		timezone: &str,
		team: Option<i64>,
	) -> rusqlite::Result<Site> {
		let conn = self.conn();
		conn.execute(
			"insert into sites (domain, timezone, team, created_at) values (?1, ?2, ?3, unixepoch())",
			params![domain, timezone, team],
		)?;
		let id = conn.last_insert_rowid();
		conn.execute(HOME_MONITOR, [id])?;
		Ok(Site {
			id: id as u64,
			domain: domain.to_owned(),
			timezone: timezone.to_owned(),
			created_at: jiff::Timestamp::now().as_second(),
		})
	}

	pub fn add_import(
		&self,
		site: u64,
		source: &str,
		first: &str,
		last: &str,
	) -> rusqlite::Result<()> {
		self.conn().execute(
			"insert into imports (site, source, first_day, last_day, created_at) values (?1, ?2, ?3, ?4, unixepoch())",
			params![site as i64, source, first, last],
		)?;
		Ok(())
	}

	pub fn first_import(&self, site: u64) -> rusqlite::Result<Option<String>> {
		self.conn().query_row(
			"select min(first_day) from imports where site = ?1",
			[site as i64],
			|row| row.get(0),
		)
	}

	pub fn backdate(&self, site: u64, ts: i64) -> rusqlite::Result<()> {
		self.conn().execute(
			"update sites set created_at = min(created_at, ?2) where id = ?1",
			params![site as i64, ts],
		)?;
		Ok(())
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
		let mut stmt = conn.prepare(
			"select id, domain, timezone, created_at from sites where domain not like '~%' order by id",
		)?;
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

	pub fn create_key(&self, name: &str, user: Option<i64>) -> rusqlite::Result<String> {
		let key = token()?;
		self.conn().execute(
			"insert into api_keys (hash, name, prefix, user, created_at) values (?1, ?2, ?3, ?4, unixepoch())",
			params![key_hash(&key), name, &key[..8], user],
		)?;
		Ok(key)
	}

	pub fn key_owner(&self, key: &str) -> rusqlite::Result<Option<Option<i64>>> {
		self.conn()
			.query_row(
				"select user from api_keys where hash = ?1",
				[key_hash(key)],
				|row| row.get(0),
			)
			.optional()
	}

	pub fn keys(&self, user: i64) -> rusqlite::Result<Vec<(i64, String, String, i64)>> {
		let conn = self.conn();
		let mut stmt = conn.prepare(
			"select rowid, name, prefix, created_at from api_keys where user = ?1 order by rowid",
		)?;
		stmt.query_map([user], |row| {
			Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
		})?
		.collect()
	}

	pub fn remove_key(&self, user: i64, id: i64) -> rusqlite::Result<()> {
		self.conn().execute(
			"delete from api_keys where rowid = ?1 and user = ?2",
			params![id, user],
		)?;
		Ok(())
	}

	pub fn imports(&self, site: u64) -> rusqlite::Result<Vec<(i64, String, String, String)>> {
		let conn = self.conn();
		let mut stmt = conn.prepare(
			"select id, source, first_day, last_day from imports where site = ?1 order by id",
		)?;
		stmt.query_map([site as i64], |row| {
			Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
		})?
		.collect()
	}

	pub fn remove_import(&self, site: u64, id: i64) -> rusqlite::Result<Option<(String, String)>> {
		self.conn()
			.query_row(
				"delete from imports where site = ?1 and id = ?2 returning first_day, last_day",
				params![site as i64, id],
				|row| Ok((row.get(0)?, row.get(1)?)),
			)
			.optional()
	}

	pub fn delete_site(&self, site: u64) -> rusqlite::Result<()> {
		let mut conn = self.conn();
		let tx = conn.transaction()?;
		for table in SITE_TABLES {
			tx.execute(
				&format!("delete from {table} where site = ?1"),
				[site as i64],
			)?;
		}
		// the row stays so sqlite never hands its id, and crater's tombstone, to a new site
		tx.execute(
			"update sites set domain = '~' || id || '~' || domain, team = null where id = ?1",
			[site as i64],
		)?;
		tx.commit()
	}

	pub fn set_timezone(&self, site: u64, timezone: &str) -> rusqlite::Result<()> {
		self.conn().execute(
			"update sites set timezone = ?2 where id = ?1",
			params![site as i64, timezone],
		)?;
		Ok(())
	}
}

pub(crate) fn token() -> rusqlite::Result<String> {
	let mut secret = [0; 32];
	getrandom::fill(&mut secret)
		.map_err(|err| rusqlite::Error::ToSqlConversionFailure(err.into()))?;
	Ok(hex(&secret))
}

pub(crate) fn now() -> i64 {
	jiff::Timestamp::now().as_second()
}

pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
	mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

pub(crate) fn hex(bytes: &[u8]) -> String {
	bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

// compared as digests, so timing says nothing about where two secrets differ
pub(crate) fn same(a: &str, b: &str) -> bool {
	blake3::hash(a.as_bytes()) == blake3::hash(b.as_bytes())
}

pub(crate) fn key_hash(key: &str) -> String {
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

// an existing database is copied aside before a new schema touches it
fn migrate(conn: &mut Connection, path: &Path) -> rusqlite::Result<()> {
	let version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
	if version > 0 && (version as usize) < MIGRATIONS.len() {
		let copy = path.with_extension(format!("v{version}.bak"));
		if !copy.exists() {
			conn.execute("vacuum into ?1", [copy.to_string_lossy()])?;
		}
	}
	for (i, sql) in MIGRATIONS.iter().enumerate().skip(version as usize) {
		let tx = conn.transaction()?;
		tx.execute_batch(sql)?;
		tx.pragma_update(None, "user_version", i as i64 + 1)?;
		tx.commit()?;
	}
	Ok(())
}
