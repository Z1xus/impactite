use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Semaphore, mpsc, oneshot};
use tokio::task::JoinHandle;
use tokio::time::{Instant, interval, sleep_until, timeout};

use crate::app::now;
use crate::notify::{self, Note};
use crate::uptime::calc::span;
use crate::uptime::log::Check;
use crate::uptime::probe::{self, Clients, OVERDUE, Outcome, REPORTED};
use crate::uptime::store::Past;
use crate::uptime::{Kind, Live, Monitor, State, visits_missed};
use crate::web::Web;

const PROBES: usize = 32;
const CERT_SOON: i64 = 14 * 86_400;
// after a failure the next checks come quickly, then the interval again
const RECHECKS: [u64; 3] = [5, 10, 20];
const SPREAD: u64 = 30;
const TICK: u64 = 5;
const SYNC_TICKS: u64 = 12;

pub enum Ping {
	Up(Option<u32>),
	Down(String),
	Start,
}

pub enum Msg {
	Sync,
	Ping(i64, Ping),
	Check(i64, oneshot::Sender<Outcome>),
	Done {
		id: i64,
		turn: u64,
		out: Outcome,
		took: Duration,
		reply: Option<oneshot::Sender<Outcome>>,
	},
}

struct Mon {
	spec: Monitor,
	live: Live,
	turn: u64,
	fails: u8,
	first: Option<Check>,
	incident: Option<(i64, i64)>,
	last_ping: i64,
	started: Option<Instant>,
	cert_until: i64,
	cert_warned: i64,
}

struct Sched {
	web: Arc<Web>,
	tx: mpsc::UnboundedSender<Msg>,
	mons: HashMap<i64, Mon>,
	heap: BinaryHeap<Reverse<(Instant, i64, u64)>>,
	windows: HashMap<i64, Vec<(i64, i64)>>,
	permits: Arc<Semaphore>,
	clients: Arc<Clients>,
}

pub fn start(web: Arc<Web>, rx: mpsc::UnboundedReceiver<Msg>) -> JoinHandle<()> {
	let sched = Sched {
		tx: web.uptime.tx.clone(),
		clients: web.uptime.clients.clone(),
		web,
		mons: HashMap::new(),
		heap: BinaryHeap::new(),
		windows: HashMap::new(),
		permits: Arc::new(Semaphore::new(PROBES)),
	};
	tokio::spawn(sched.run(rx))
}

fn active(windows: &HashMap<i64, Vec<(i64, i64)>>, id: i64, at: i64) -> Option<i64> {
	windows
		.get(&id)?
		.iter()
		.find(|(start, end)| (*start..*end).contains(&at))
		.map(|(_, end)| *end)
}

impl Sched {
	async fn run(mut self, mut rx: mpsc::UnboundedReceiver<Msg>) {
		self.sync();
		let mut tick = interval(Duration::from_secs(TICK));
		let mut ticks = 0u64;
		loop {
			let next = self.heap.peek().map_or_else(
				|| Instant::now() + Duration::from_secs(3600),
				|Reverse((due, ..))| *due,
			);
			tokio::select! {
				Some(msg) = rx.recv() => self.handle(msg),
				() = sleep_until(next) => self.fire(),
				_ = tick.tick() => {
					ticks += 1;
					self.tick(ticks);
				}
			}
		}
	}

	fn handle(&mut self, msg: Msg) {
		match msg {
			Msg::Sync => self.sync(),
			Msg::Ping(id, ping) => self.ping(id, ping),
			Msg::Check(id, reply) => {
				if self
					.mons
					.get(&id)
					.is_some_and(|mon| mon.spec.kind != Kind::Heartbeat)
				{
					self.probe(id, Some(reply));
				}
			}
			Msg::Done {
				id,
				turn,
				out,
				took,
				reply,
			} => self.done(id, turn, out, took, reply),
		}
	}

	fn tick(&mut self, ticks: u64) {
		let web = self.web.clone();
		tokio::task::spawn_blocking(move || {
			if let Err(err) = web.uptime.log.flush() {
				tracing::warn!(%err, "uptime log");
			}
		});
		let at = now();
		let mut ended = Vec::new();
		for (id, windows) in &self.windows {
			let Some(mon) = self.mons.get_mut(id) else {
				continue;
			};
			let inside = windows
				.iter()
				.any(|(start, end)| (*start..*end).contains(&at));
			if inside && !mon.spec.paused && mon.live.state != State::Maintenance {
				enter(mon, State::Maintenance, at);
				self.web.uptime.publish(mon.spec.team, *id, mon.live);
			} else if !inside && mon.live.state == State::Maintenance {
				ended.push(*id);
			}
		}
		for id in ended {
			self.schedule(id, Duration::ZERO);
		}
		if ticks.is_multiple_of(SYNC_TICKS) {
			self.sync();
		}
	}

	fn schedule(&mut self, id: i64, delay: Duration) {
		if let Some(mon) = self.mons.get_mut(&id) {
			mon.turn += 1;
			self.heap
				.push(Reverse((Instant::now() + delay, id, mon.turn)));
		}
	}

	fn sync(&mut self) {
		let at = now();
		let Ok(rows) = self.web.app.monitors() else {
			return;
		};
		if let Ok(windows) = self.web.app.windows(at) {
			self.windows = windows;
		}
		let known: HashSet<i64> = rows.iter().map(|spec| spec.id).collect();
		let gone: Vec<i64> = self
			.mons
			.keys()
			.filter(|id| !known.contains(id))
			.copied()
			.collect();
		for id in gone {
			self.mons.remove(&id);
			self.web.uptime.forget(id);
		}
		let mut pasts = None;
		for spec in rows {
			match self.mons.get(&spec.id) {
				Some(mon) if mon.spec == spec => {}
				Some(_) => self.update(spec, at),
				None => {
					let pasts =
						pasts.get_or_insert_with(|| self.web.app.pasts().unwrap_or_default());
					let past = pasts.remove(&spec.id).unwrap_or_default();
					self.add(spec, past, at);
				}
			}
		}
	}

	fn add(&mut self, spec: Monitor, past: Past, at: i64) {
		let last = self.web.uptime.log.last(spec.id, at);
		let cert = (past.cert_until > 0).then_some(past.cert_until);
		let live = if spec.paused {
			Live {
				state: State::Paused,
				since: past.paused.unwrap_or(at),
				latency: None,
				at,
				cert,
			}
		} else if let Some((_, started)) = past.incident {
			Live {
				state: State::Down,
				since: started,
				latency: None,
				at: last.map_or(started, |check| check.at),
				cert,
			}
		} else if let Some(check) = last.filter(|check| check.state != State::Down) {
			Live {
				state: check.state,
				since: spec.created_at.max(past.ended.unwrap_or(0)),
				latency: (check.ms > 0).then_some(check.ms),
				at: check.at,
				cert,
			}
		} else {
			Live {
				state: State::Pending,
				since: spec.created_at,
				latency: None,
				at: spec.created_at,
				cert,
			}
		};
		let (id, team, paused) = (spec.id, spec.team, spec.paused);
		let delay = if spec.kind == Kind::Heartbeat {
			spec.interval + spec.config.grace
		} else {
			(id as u64 % SPREAD.min(spec.interval as u64)) as i64
		};
		self.web.uptime.publish(team, id, live);
		self.mons.insert(
			id,
			Mon {
				spec,
				live,
				turn: 0,
				fails: if past.incident.is_some() { 2 } else { 0 },
				first: None,
				incident: past.incident,
				last_ping: at,
				started: None,
				cert_until: past.cert_until,
				cert_warned: past.cert_warned,
			},
		);
		if !paused {
			self.schedule(id, Duration::from_secs(delay as u64));
		}
	}

	fn update(&mut self, spec: Monitor, at: i64) {
		let id = spec.id;
		let Some(mon) = self.mons.get_mut(&id) else {
			return;
		};
		let was_paused = mon.spec.paused;
		let paused = spec.paused;
		let spread = id as u64 % SPREAD.min(spec.interval as u64);
		mon.spec = spec;
		mon.turn += 1;
		if paused {
			if !was_paused {
				if let Some((incident, _)) = mon.incident.take()
					&& let Err(err) = self.web.app.close_incident(incident, at)
				{
					tracing::warn!(%err, "close incident");
				}
				mon.fails = 0;
				mon.first = None;
				enter(mon, State::Paused, at);
				self.web.uptime.publish(mon.spec.team, id, mon.live);
			}
			return;
		}
		let delay = if was_paused {
			enter(mon, State::Pending, at);
			mon.last_ping = at;
			self.web.uptime.publish(mon.spec.team, id, mon.live);
			1
		} else {
			spread
		};
		self.schedule(id, Duration::from_secs(delay));
	}

	fn fire(&mut self) {
		let cutoff = Instant::now();
		while let Some(Reverse((due, id, turn))) = self.heap.peek().copied() {
			if due > cutoff {
				break;
			}
			self.heap.pop();
			if self.mons.get(&id).is_some_and(|mon| mon.turn == turn) {
				self.due(id);
			}
		}
	}

	fn due(&mut self, id: i64) {
		let at = now();
		let Some(mon) = self.mons.get_mut(&id) else {
			return;
		};
		if mon.spec.paused {
			return;
		}
		let interval = mon.spec.interval as u64;
		if let Some(end) = active(&self.windows, id, at) {
			if mon.live.state != State::Maintenance {
				enter(mon, State::Maintenance, at);
				self.web.uptime.publish(mon.spec.team, id, mon.live);
			}
			mon.last_ping = at;
			let wait = ((end - at).max(1) as u64).min(interval);
			self.schedule(id, Duration::from_secs(wait));
			return;
		}
		if mon.spec.kind != Kind::Heartbeat {
			self.probe(id, None);
			return;
		}
		let deadline = mon.last_ping + mon.spec.interval + mon.spec.config.grace;
		if at <= deadline {
			let wait = (deadline - at + 1) as u64;
			self.schedule(id, Duration::from_secs(wait));
			return;
		}
		let out = Outcome::down(
			OVERDUE,
			format!("No ping for {}", span(at - mon.last_ping)),
			"",
		);
		settle(&self.web, mon, &out, at, true);
		self.schedule(id, Duration::from_secs(interval));
	}

	fn probe(&self, id: i64, reply: Option<oneshot::Sender<Outcome>>) {
		let Some(mon) = self.mons.get(&id) else {
			return;
		};
		let (web, tx, permits, clients) = (
			self.web.clone(),
			self.tx.clone(),
			self.permits.clone(),
			self.clients.clone(),
		);
		let (spec, turn) = (mon.spec.clone(), mon.turn);
		tokio::spawn(async move {
			let due = Instant::now();
			let _permit = permits.acquire_owned().await;
			let local = spec.created_by.is_some_and(|user| web.app.is_admin(user));
			let out = probe::run(&clients, &spec, local).await;
			let _ = tx.send(Msg::Done {
				id,
				turn,
				out,
				took: due.elapsed(),
				reply,
			});
		});
	}

	fn done(
		&mut self,
		id: i64,
		turn: u64,
		out: Outcome,
		took: Duration,
		reply: Option<oneshot::Sender<Outcome>>,
	) {
		let manual = reply.is_some();
		if let Some(reply) = reply {
			let _ = reply.send(out.clone());
		}
		let at = now();
		let Some(mon) = self.mons.get_mut(&id) else {
			return;
		};
		if mon.spec.paused
			|| (!manual && turn != mon.turn)
			|| active(&self.windows, id, at).is_some()
		{
			return;
		}
		let delay = Duration::from_secs(settle(&self.web, mon, &out, at, false));
		self.schedule(id, delay.saturating_sub(took));
	}

	fn ping(&mut self, id: i64, ping: Ping) {
		let at = now();
		let Some(mon) = self.mons.get_mut(&id) else {
			return;
		};
		if mon.spec.paused || mon.spec.kind != Kind::Heartbeat {
			return;
		}
		if active(&self.windows, id, at).is_some() {
			mon.last_ping = at;
			return;
		}
		match ping {
			Ping::Start => {
				mon.started = Some(Instant::now());
				return;
			}
			Ping::Up(reported) => {
				let ms = mon
					.started
					.take()
					.map(|started| started.elapsed().as_millis() as u32)
					.or(reported)
					.unwrap_or(0)
					.min(u32::from(u16::MAX));
				mon.last_ping = at;
				settle(&self.web, mon, &Outcome::up(ms), at, true);
			}
			Ping::Down(msg) => {
				mon.started = None;
				mon.last_ping = at;
				let cause = if msg.is_empty() {
					"Reported a failure"
				} else {
					&msg
				};
				settle(
					&self.web,
					mon,
					&Outcome::down(REPORTED, cause, &msg),
					at,
					true,
				);
			}
		}
		let wait = (mon.spec.interval + mon.spec.config.grace + 1) as u64;
		self.schedule(id, Duration::from_secs(wait));
	}
}

fn enter(mon: &mut Mon, state: State, at: i64) {
	let was = mon.live.state;
	let available = |state| matches!(state, State::Up | State::Slow);
	if was != state && !(available(was) && available(state)) {
		mon.live.since = at;
	}
	mon.live.state = state;
}

fn settle(web: &Arc<Web>, mon: &mut Mon, out: &Outcome, at: i64, immediate: bool) -> u64 {
	let interval = mon.spec.interval as u64;
	let uptime = &web.uptime;
	if out.ok {
		let soon = out.cert.is_some_and(|until| until - at < CERT_SOON);
		let slow = mon.spec.kind != Kind::Heartbeat && (out.ms > mon.spec.config.slow || soon);
		let state = if slow { State::Slow } else { State::Up };
		if let Some((incident, started)) = mon.incident.take() {
			if let Err(err) = web.app.close_incident(incident, at) {
				tracing::warn!(%err, "close incident");
			}
			let down = at - started;
			if web.app.incident_notified(incident) {
				let (web, spec) = (web.clone(), mon.spec.clone());
				tokio::spawn(async move {
					let missed =
						visits_missed(&web, spec.site, vec![(incident, started, Some(at))])
							.await
							.pop()
							.flatten();
					let mut line = format!("{} was down for {}.", spec.name, span(down));
					if let Some(count) = missed {
						line = format!(
							"{line} About {} visits missed.",
							notify::grouped(u64::from(count))
						);
					}
					announce(&web, &spec, "up", format!("{} is back up", spec.name), line);
				});
			}
		}
		mon.fails = 0;
		mon.first = None;
		uptime.log.push(
			mon.spec.id,
			Check {
				at,
				ms: out.ms,
				state,
				code: out.code,
			},
		);
		enter(mon, state, at);
		mon.live.at = at;
		mon.live.latency = (out.ms > 0).then_some(out.ms);
		if let Some(until) = out.cert {
			mon.live.cert = out.cert;
			warn_cert(web, mon, until, at);
		}
		uptime.publish(mon.spec.team, mon.spec.id, mon.live);
		return interval;
	}
	mon.fails = if immediate {
		mon.fails.max(2)
	} else {
		mon.fails.saturating_add(1)
	};
	let check = Check {
		at,
		ms: out.ms,
		state: State::Down,
		code: out.code,
	};
	if mon.fails == 1 {
		mon.first = Some(check);
		return RECHECKS[0].min(interval);
	}
	let started = mon.first.map_or(at, |first| first.at);
	if mon.incident.is_none() {
		let notified = announce(
			web,
			&mon.spec,
			"down",
			format!("{} is down", mon.spec.name),
			format!("{}.", out.cause),
		);
		match web
			.app
			.open_incident(mon.spec.id, started, &out.cause, &out.detail, notified)
		{
			Ok(incident) => mon.incident = Some((incident, started)),
			Err(err) => tracing::warn!(%err, "open incident"),
		}
		if let Some(first) = mon.first.take() {
			uptime.log.push(mon.spec.id, first);
		}
		enter(mon, State::Down, started);
	}
	uptime.log.push(mon.spec.id, check);
	mon.live.at = at;
	mon.live.latency = None;
	uptime.publish(mon.spec.team, mon.spec.id, mon.live);
	let step = usize::from(mon.fails) - 1;
	RECHECKS
		.get(step)
		.copied()
		.map_or(interval, |wait| wait.min(interval))
}

fn warn_cert(web: &Arc<Web>, mon: &mut Mon, until: i64, at: i64) {
	let before = (mon.cert_until, mon.cert_warned);
	if (until - mon.cert_until).abs() > 86_400 {
		mon.cert_until = until;
		mon.cert_warned = 0;
	}
	let days = (until - at) / 86_400;
	let level = match days {
		..=1 => 1,
		2..=7 => 7,
		8..=14 => 14,
		_ => 0,
	};
	if level > 0 && (mon.cert_warned == 0 || level < mon.cert_warned) {
		mon.cert_warned = level;
		announce(
			web,
			&mon.spec,
			"certificate",
			format!("Certificate for {} expires soon", mon.spec.name),
			format!(
				"The certificate for {} expires in {}.",
				mon.spec.name,
				span(until - at)
			),
		);
	}
	if before != (mon.cert_until, mon.cert_warned)
		&& let Err(err) = web
			.app
			.set_cert(mon.spec.id, mon.cert_until, mon.cert_warned)
	{
		tracing::warn!(%err, "save certificate");
	}
}

fn announce(
	web: &Arc<Web>,
	spec: &Monitor,
	kind: &'static str,
	subject: String,
	line: String,
) -> bool {
	if spec.muted {
		return false;
	}
	let targets = web.app.alerts(spec.team).unwrap_or_default();
	let link = format!("{}/uptime/{}", web.url.trim_end_matches('/'), spec.id);
	let note = Arc::new(Note::alert(subject, line, link));
	for alert in &targets {
		let (web, note, name) = (web.clone(), note.clone(), spec.name.clone());
		let (id, to) = (alert.id, alert.target.clone());
		tokio::spawn(async move {
			let sent = timeout(
				Duration::from_secs(30),
				notify::deliver(&web, &to, &note, &name, kind),
			)
			.await
			.unwrap_or_else(|_| Err(anyhow::anyhow!("Timed out")));
			let failure = sent
				.err()
				.map(|err| notify::failure(&err))
				.unwrap_or_default();
			if !failure.is_empty() {
				tracing::warn!(monitor = name, failure, "uptime alert");
			}
			if let Err(err) = web.app.mark_alert(id, &failure) {
				tracing::warn!(%err, "save alert");
			}
		});
	}
	!targets.is_empty()
}
