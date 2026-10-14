mod app;
mod ingest;
mod server;

use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, bail};
use clap::{Parser, Subcommand};
use crater::Db;
use mimalloc::MiMalloc;
use tokio::net::TcpListener;
use tokio::signal::unix::{SignalKind, signal};
use tracing_subscriber::EnvFilter;

use crate::app::App;
use crate::ingest::Ingest;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

#[derive(Parser)]
#[command(version)]
struct Cli {
	#[arg(long, env = "IMPACTITE_DATA", default_value = "data")]
	data: PathBuf,
	#[command(subcommand)]
	command: Command,
}

#[derive(Subcommand)]
enum Command {
	Serve {
		#[arg(long, env = "IMPACTITE_LISTEN", default_value = "127.0.0.1:8000")]
		listen: SocketAddr,
	},
	Site {
		#[command(subcommand)]
		command: SiteCommand,
	},
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

	match cli.command {
		Command::Serve { listen } => serve(app, &cli.data, listen).await,
		Command::Site {
			command: SiteCommand::Add { domain, timezone },
		} => {
			if jiff::tz::TimeZone::get(&timezone).is_err() {
				bail!("unknown timezone {timezone}");
			}
			let site = app.add_site(&domain, &timezone)?;
			println!("{}  {}  {}", site.id, site.domain, site.timezone);
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

async fn serve(app: Arc<App>, data: &Path, listen: SocketAddr) -> anyhow::Result<()> {
	let db = Arc::new(Db::open(data.join("events")).context("open event store")?);
	let ingest = Arc::new(Ingest::new(db.clone(), app));

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
	axum::serve(
		listener,
		server::router(ingest.clone()).into_make_service_with_connect_info::<SocketAddr>(),
	)
	.with_graceful_shutdown(shutdown())
	.await?;

	sweeper.abort();
	let _ = sweeper.await;
	drop(ingest);
	let db = Arc::try_unwrap(db).map_err(|_| anyhow::anyhow!("event store still in use"))?;
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
