mod accounts;
mod app;
mod demo;
mod geo;
mod icons;
mod ingest;
mod mail;
mod server;
mod stats;

use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, bail};
use clap::{Parser, Subcommand};
use crater::Db;
use mimalloc::MiMalloc;
use tokio::net::TcpListener;
use tokio::signal::unix::{SignalKind, signal};
use tracing_subscriber::EnvFilter;

use crate::accounts::Web;
use crate::app::App;
use crate::geo::Geo;
use crate::ingest::Ingest;
use crate::mail::Mail;
use crate::stats::Stats;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

#[derive(Parser)]
#[command(version)]
struct Cli {
	#[arg(long, env = "IMPACTITE_DATA", default_value = "data")]
	data: PathBuf,
	#[arg(long, env = "IMPACTITE_MAIL")]
	mail: Option<String>,
	#[arg(
		long,
		env = "IMPACTITE_MAIL_FROM",
		default_value = "Impactite <impactite@localhost>"
	)]
	mail_from: String,
	#[command(subcommand)]
	command: Command,
}

#[derive(Subcommand)]
enum Command {
	Serve {
		#[arg(long, env = "IMPACTITE_LISTEN", default_value = "127.0.0.1:8000")]
		listen: SocketAddr,
		#[arg(long, env = "IMPACTITE_GEOIP")]
		geoip: Option<PathBuf>,
		#[arg(long, env = "IMPACTITE_URL")]
		url: Option<String>,
	},
	Site {
		#[command(subcommand)]
		command: SiteCommand,
	},
	Key {
		#[command(subcommand)]
		command: KeyCommand,
	},
	Mail {
		#[command(subcommand)]
		command: MailCommand,
	},
	Demo {
		domain: String,
		#[arg(long, default_value_t = 90)]
		days: i64,
		#[arg(long, default_value = "Europe/Berlin")]
		timezone: String,
	},
}

#[derive(Subcommand)]
enum MailCommand {
	Test { to: String },
}

#[derive(Subcommand)]
enum KeyCommand {
	Create { name: String },
}

#[derive(Subcommand)]
enum SiteCommand {
	Add {
		domain: String,
		#[arg(long, default_value = "UTC")]
		timezone: String,
	},
	List,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	tracing_subscriber::fmt()
		.with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
		.init();

	let cli = Cli::parse();
	fs::create_dir_all(&cli.data).context("create data dir")?;
	let app = Arc::new(App::open(&cli.data.join("impactite.db")).context("open app db")?);
	let mail = Arc::new(Mail::new(cli.mail.as_deref(), &cli.mail_from)?);

	match cli.command {
		Command::Serve { listen, geoip, url } => {
			let url = url.unwrap_or_else(|| format!("http://{listen}"));
			serve(app, mail, &cli.data, listen, geoip, url).await
		}
		Command::Mail {
			command: MailCommand::Test { to },
		} => {
			mail.send(
				&to,
				"Impactite mail works",
				"Mail is set up.",
				"<p>Mail is set up.</p>",
			)
			.await?;
			println!("sent to {to}");
			Ok(())
		}
		Command::Site {
			command: SiteCommand::Add { domain, timezone },
		} => {
			if jiff::tz::TimeZone::get(&timezone).is_err() {
				bail!("unknown timezone {timezone}");
			}
			let site = app.add_site(&domain, &timezone, None)?;
			println!("{}  {}  {}", site.id, site.domain, site.timezone);
			Ok(())
		}
		Command::Demo {
			domain,
			days,
			timezone,
		} => {
			let site = match app.site(&domain)? {
				Some(site) => site,
				None => {
					let team = match app.first_user()? {
						Some(user) => Some(app.personal_team(user)?),
						None => None,
					};
					app.add_site(&domain, &timezone, team)?
				}
			};
			let db = Db::open(cli.data.join("events")).context("open event store")?;
			let count = demo::seed(&db, site.id, &site.domain, &site.timezone, days);
			db.sync()?;
			db.close()?;
			println!("{count} events  {}  {days} days", site.domain);
			Ok(())
		}
		Command::Key {
			command: KeyCommand::Create { name },
		} => {
			println!("{}", app.create_key(&name)?);
			Ok(())
		}
		Command::Site {
			command: SiteCommand::List,
		} => {
			for site in app.sites()? {
				println!("{}  {}  {}", site.id, site.domain, site.timezone);
			}
			Ok(())
		}
	}
}

async fn serve(
	app: Arc<App>,
	mail: Arc<Mail>,
	data: &Path,
	listen: SocketAddr,
	geoip: Option<PathBuf>,
	url: String,
) -> anyhow::Result<()> {
	let db = Arc::new(Db::open(data.join("events")).context("open event store")?);
	let geo = Arc::new(Geo::default());
	let geo_task = match geoip {
		Some(path) => {
			geo.load(&path).context("load geoip database")?;
			None
		}
		None => Some(tokio::spawn({
			let geo = geo.clone();
			let dir = data.join("geo");
			async move { geo.keep_fresh(dir).await }
		})),
	};
	let stats = Arc::new(Stats {
		db: db.clone(),
		app: app.clone(),
	});
	let setup = match app.users_exist()? {
		true => None,
		false => Some(app::token()?),
	};
	if let Some(token) = &setup {
		tracing::info!(
			"finish setup at {}/setup?token={token}",
			url.trim_end_matches('/')
		);
	}
	let ingest = Arc::new(Ingest::new(db.clone(), app.clone(), geo));
	let web = Arc::new(Web {
		app,
		stats: stats.clone(),
		ingest: ingest.clone(),
		mail,
		http: reqwest::Client::new(),
		data: data.to_path_buf(),
		url,
		setup: Mutex::new(setup),
	});

	let sweeper = tokio::spawn({
		let ingest = ingest.clone();
		async move {
			let mut interval = tokio::time::interval(Duration::from_secs(60));
			loop {
				interval.tick().await;
				ingest.sweep();
			}
		}
	});

	let listener = TcpListener::bind(listen).await.context("bind")?;
	tracing::info!(%listen, "listening");
	let stop = Arc::new(tokio::sync::Notify::new());
	let server = axum::serve(
		listener,
		server::router(ingest.clone(), stats.clone(), web.clone())
			.into_make_service_with_connect_info::<SocketAddr>(),
	)
	.with_graceful_shutdown({
		let stop = stop.clone();
		async move {
			shutdown().await;
			stop.notify_one();
		}
	});
	tokio::select! {
		served = server => served?,
		() = async {
			stop.notified().await;
			tokio::time::sleep(Duration::from_secs(3)).await;
		} => {}
	}

	sweeper.abort();
	let _ = sweeper.await;
	if let Some(task) = geo_task {
		task.abort();
		let _ = task.await;
	}
	db.close().context("close event store")?;
	Ok(())
}

async fn shutdown() {
	let terminate = async {
		match signal(SignalKind::terminate()) {
			Ok(mut terminate) => {
				terminate.recv().await;
			}
			Err(_) => std::future::pending().await,
		}
	};
	tokio::select! {
		_ = tokio::signal::ctrl_c() => {}
		() = terminate => {}
	}
}
