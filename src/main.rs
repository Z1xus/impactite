mod accounts;
mod admin;
mod app;
mod assets;
mod cli;
mod demo;
mod export;
mod funnels;
mod geo;
mod goals;
mod icons;
mod import;
mod ingest;
mod keys;
mod mail;
mod net;
mod passkeys;
mod prefs;
mod profile;
mod segments;
mod server;
mod shields;
mod sites;
mod stats;
mod teams;
mod tracker;
mod webauthn;

use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, bail};
use axum_server::tls_rustls::RustlsConfig;
use clap::{Args, Parser, Subcommand};
use crater::Db;
use futures_util::StreamExt;
use mimalloc::MiMalloc;
use rustls_acme::AcmeConfig;
use rustls_acme::caches::DirCache;
use tokio::net::TcpListener;
use tokio::signal::unix::{SignalKind, signal};
use tracing_subscriber::EnvFilter;

use crate::accounts::Web;
use crate::app::App;
use crate::geo::Geo;
use crate::ingest::Ingest;
use crate::mail::Mail;
use crate::net::Proxies;
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
	Serve(Serve),
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
	Import {
		#[command(subcommand)]
		source: ImportSource,
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
enum ImportSource {
	Plausible {
		source: String,
		#[arg(long)]
		site: String,
		#[arg(long)]
		plausible_site_id: Option<u64>,
		#[arg(long, default_value = "UTC")]
		timezone: String,
	},
	Goatcounter {
		file: PathBuf,
		#[arg(long)]
		site: String,
		#[arg(long, default_value = "UTC")]
		timezone: String,
	},
	Impactite {
		file: PathBuf,
		#[arg(long)]
		site: String,
		#[arg(long, default_value = "UTC")]
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

#[derive(Args)]
struct Serve {
	#[arg(long, env = "IMPACTITE_LISTEN", default_value = "127.0.0.1:8000")]
	listen: SocketAddr,
	#[arg(long, env = "IMPACTITE_URL")]
	url: Option<String>,
	#[arg(long, env = "IMPACTITE_GEOIP")]
	geoip: Option<PathBuf>,
	#[arg(long, env = "IMPACTITE_TRUSTED_PROXIES")]
	trusted_proxies: Option<String>,
	#[arg(long, env = "IMPACTITE_ACME", conflicts_with_all = ["cert", "key"])]
	acme: bool,
	#[arg(long, env = "IMPACTITE_ACME_EMAIL")]
	acme_email: Option<String>,
	#[arg(long, env = "IMPACTITE_ACME_STAGING", hide = true)]
	acme_staging: bool,
	#[arg(long, env = "IMPACTITE_CERT", requires = "key")]
	cert: Option<PathBuf>,
	#[arg(long, env = "IMPACTITE_KEY", requires = "cert")]
	key: Option<PathBuf>,
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
		Command::Serve(serve_args) => serve(app, mail, &cli.data, serve_args).await,
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
			let site = app.add_site(&domain, &timezone, default_team(&app)?)?;
			println!("{}  {}  {}", site.id, site.domain, site.timezone);
			Ok(())
		}
		Command::Import { source } => {
			let (domain, timezone) = match &source {
				ImportSource::Plausible { site, timezone, .. }
				| ImportSource::Goatcounter { site, timezone, .. }
				| ImportSource::Impactite { site, timezone, .. } => (site, timezone),
			};
			let site = find_or_create(&app, domain, timezone)?;
			let db = Db::open(cli.data.join("events")).context("open event store")?;
			let started = std::time::Instant::now();
			let (count, unit) = match source {
				ImportSource::Plausible {
					source,
					plausible_site_id,
					..
				} if source.starts_with("http") => {
					let id = plausible_site_id
						.context("--plausible-site-id is needed for clickhouse")?;
					let (count, first) =
						import::clickhouse::import(&db, &site, &source, id).await?;
					if let Some(first) = first {
						app.backdate(site.id, first)?;
					}
					(count, "events")
				}
				ImportSource::Plausible { source, .. } => (
					import::plausible_zip(&db, &app, &site, Path::new(&source))?,
					"days",
				),
				ImportSource::Goatcounter { file, .. } => {
					(import::goatcounter_csv(&db, &app, &site, &file)?, "events")
				}
				ImportSource::Impactite { file, .. } => {
					(import::native_zip(&db, &app, &site, &file)?, "events")
				}
			};
			db.sync()?;
			db.close()?;
			cli::say(
				'+',
				&format!(
					"{count} {unit}  |  {}  |  {:.1?}",
					site.domain,
					started.elapsed()
				),
			);
			Ok(())
		}
		Command::Demo {
			domain,
			days,
			timezone,
		} => {
			let site = find_or_create(&app, &domain, &timezone)?;
			let db = Db::open(cli.data.join("events")).context("open event store")?;
			let count = demo::seed(&db, site.id, &site.domain, &site.timezone, days);
			db.sync()?;
			db.close()?;
			app.backdate(site.id, jiff::Timestamp::now().as_second() - days * 86_400)?;
			println!("{count} events  {}  {days} days", site.domain);
			Ok(())
		}
		Command::Key {
			command: KeyCommand::Create { name },
		} => {
			println!("{}", app.create_key(&name, None)?);
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

async fn serve(app: Arc<App>, mail: Arc<Mail>, data: &Path, args: Serve) -> anyhow::Result<()> {
	let scheme = if args.acme || args.cert.is_some() {
		"https"
	} else {
		"http"
	};
	let url = args
		.url
		.clone()
		.unwrap_or_else(|| format!("{scheme}://{}", args.listen));
	if args.acme && !url.starts_with("https://") {
		bail!("--acme needs an https --url");
	}
	let proxies = Proxies::parse(args.trusted_proxies.as_deref())?;
	let db = Arc::new(Db::open(data.join("events")).context("open event store")?);
	let geo = Arc::new(Geo::default());
	let geo_task = match &args.geoip {
		Some(path) => {
			geo.load(path).context("load geoip database")?;
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
	let ingest = Arc::new(Ingest::new(db.clone(), app.clone(), geo, proxies));
	let web = Arc::new(Web {
		app,
		stats: stats.clone(),
		ingest: ingest.clone(),
		mail,
		http: reqwest::Client::new(),
		data: data.to_path_buf(),
		rp: webauthn::Relying::from_url(&url).context("invalid --url")?,
		url: url.clone(),
		setup: Mutex::new(setup),
		ceremonies: Mutex::default(),
	});

	let sweeper = tokio::spawn({
		let ingest = ingest.clone();
		let app = web.app.clone();
		let db = db.clone();
		async move {
			let mut interval = tokio::time::interval(Duration::from_secs(60));
			let mut minutes = 0u64;
			loop {
				interval.tick().await;
				ingest.sweep();
				if minutes.is_multiple_of(60) {
					prune(&app, &db);
				}
				minutes += 1;
			}
		}
	});

	let handle = axum_server::Handle::new();
	tokio::spawn({
		let handle = handle.clone();
		async move {
			shutdown().await;
			handle.graceful_shutdown(Some(Duration::from_secs(3)));
		}
	});
	let service =
		server::router(ingest, stats, web).into_make_service_with_connect_info::<SocketAddr>();
	tracing::info!(listen = %args.listen, %url, "listening");

	if let (Some(cert), Some(key)) = (&args.cert, &args.key) {
		let config = RustlsConfig::from_pem_file(cert, key)
			.await
			.context("load tls certificate")?;
		tokio::spawn({
			let (config, cert, key) = (config.clone(), cert.clone(), key.clone());
			async move {
				loop {
					tokio::time::sleep(Duration::from_secs(6 * 3600)).await;
					if let Err(err) = config.reload_from_pem_file(&cert, &key).await {
						tracing::warn!(%err, "tls certificate reload");
					}
				}
			}
		});
		axum_server::bind_rustls(args.listen, config)
			.handle(handle)
			.serve(service)
			.await?;
	} else if args.acme {
		let host = url::Url::parse(&url)?
			.host_str()
			.context("--url has no host")?
			.to_owned();
		let mut state = AcmeConfig::new([host])
			.contact(
				args.acme_email
					.iter()
					.map(|email| format!("mailto:{email}")),
			)
			.cache(DirCache::new(data.join("acme")))
			.directory_lets_encrypt(!args.acme_staging)
			.state();
		let acceptor = state.axum_acceptor(state.default_rustls_config());
		tokio::spawn(async move {
			while let Some(event) = state.next().await {
				match event {
					Ok(event) => tracing::info!(?event, "acme"),
					Err(err) => tracing::warn!(%err, "acme"),
				}
			}
		});
		tokio::spawn(redirect(url.clone()));
		axum_server::bind(args.listen)
			.acceptor(acceptor)
			.handle(handle)
			.serve(service)
			.await?;
	} else {
		axum_server::bind(args.listen)
			.handle(handle)
			.serve(service)
			.await?;
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

fn find_or_create(app: &App, domain: &str, timezone: &str) -> anyhow::Result<app::Site> {
	if let Some(site) = app.site(domain)? {
		return Ok(site);
	}
	Ok(app.add_site(domain, timezone, default_team(app)?)?)
}

// before setup sites stay without a team, setup adopts them
fn default_team(app: &App) -> anyhow::Result<Option<i64>> {
	Ok(match app.first_user()? {
		Some(user) => Some(app.personal_team(user)?),
		None => None,
	})
}

// drops days older than each site keeps
fn prune(app: &App, db: &Db) {
	let Ok(sites) = app.retentions() else {
		return;
	};
	for (site, days) in sites {
		let tz = jiff::tz::TimeZone::get(&site.timezone).unwrap_or(jiff::tz::TimeZone::UTC);
		let today = jiff::Timestamp::now().to_zoned(tz).date();
		let before = stats::day_number(today) - days as i32;
		if let Err(err) = db.prune(site.id, before) {
			tracing::warn!(%err, site = site.domain, "prune");
		}
	}
}

async fn redirect(url: String) {
	let base = url.trim_end_matches('/').to_owned();
	let app = axum::Router::new().fallback(move |uri: axum::http::Uri| {
		let target = format!("{base}{uri}");
		async move { axum::response::Redirect::permanent(&target) }
	});
	match TcpListener::bind(("0.0.0.0", 80)).await {
		Ok(listener) => {
			let _ = axum::serve(listener, app).await;
		}
		Err(err) => tracing::warn!(%err, "http redirect on port 80"),
	}
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
