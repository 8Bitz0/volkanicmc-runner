// #![deny(warnings)]
#![forbid(unsafe_code)]

use clap::Parser;
use std::{
    path::PathBuf,
    sync::{Arc, atomic::{AtomicBool, Ordering}},
};
use tokio::sync::{broadcast, Mutex};
use tracing::{debug, info, error};

mod config;
mod global_event;
mod instance;
mod net;
mod storage;

const DEBUG_MODE_VAR: &str = "VK_DEBUG";

#[derive(Debug, Parser)]
struct Args {
    pub config_path: PathBuf,
    #[arg(short = 'a', long)]
    pub address: Option<String>,
    #[arg(short = 'p', long)]
    pub port: Option<u16>,
    #[arg(long)]
    pub add_latency: Option<u16>,
}

#[derive(Clone)]
struct AppState {
    pub g_event_tx: broadcast::Sender<global_event::GlobalEvent>,
    pub instances: Arc<Mutex<instance::DockerInstanceProvider>>,
    pub add_latency: Option<u16>,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let debug_mode = std::env::var(DEBUG_MODE_VAR) == Ok(String::from("true"));

    tracing_subscriber::fmt()
        .with_max_level({match debug_mode {
            true => tracing::Level::DEBUG,
            false => tracing::Level::INFO,
        }})
        .init();

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    }).unwrap();

    #[cfg(unix)]
    let shutdown = async {
        let mut interrupt = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt()).unwrap();
        let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).unwrap();

        tokio::select! {
            _ = interrupt.recv() => debug!("Received SIGINT"),
            _ = terminate.recv() => info!("Received termination signal"),
        }
    };

    #[cfg(windows)]
    let shutdown = async {
        tokio::signal::ctrl_c().await.unwrap();
        debug!("Received Ctrl+C");
    };

    tokio::select! {
        _ = shutdown => {
            running.store(false, Ordering::SeqCst);
        },
        _ = run(args) => {},
    };

    info!("Shutting down...");

    std::process::exit(0);
}

async fn run(args: Args) {
    let app_config = Arc::new(Mutex::new(
        match config::ConfigFile::new(args.config_path.clone()).await {
            Ok(o) => o,
            Err(e) => {
                error!("Config error: {}", e);
                std::process::exit(1);
            }
        }
    ));

    let address = args.address.clone().unwrap_or(app_config.lock().await.config.address.clone());
    let port = args.port.unwrap_or(app_config.lock().await.config.port);

    let g_event_tx = global_event::init_channel();

    let storage_provider = Arc::new(Mutex::new(
        match storage::JsonStorageProvider::new(app_config.lock().await.config.clone()).await {
            Ok(o) => o,
            Err(e) => {
                error!("Storage provider error: {}", e);
                std::process::exit(1);
            }
        }
    ));

    let instance_provider = Arc::new(Mutex::new(
        match instance::DockerInstanceProvider::new(
            app_config.clone(),
            g_event_tx.clone(),
            storage_provider.clone(),
        ).await {
            Ok(o) => o,
            Err(e) =>  {
                error!("Instance provider error: {}", e);
                std::process::exit(1);
            },
        }
    ));

    let app_state = AppState {
        g_event_tx,
        instances: instance_provider.clone(),
        add_latency: args.add_latency,
    };

    let http_handle = net::http::serve(address, port, app_state).await;

    match http_handle.await {
        Ok(_) => {},
        Err(e) => {
            error!("HTTP server error: {}", e);
        }
    }
}
