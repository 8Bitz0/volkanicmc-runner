// #![deny(warnings)]
#![forbid(unsafe_code)]

use clap::Parser;
use std::{path::PathBuf, sync::Arc};
use tokio::sync::{broadcast, Mutex};
use tracing::error;

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
}

#[derive(Clone)]
struct AppState {
    pub g_event_tx: broadcast::Sender<global_event::GlobalEvent>,
    pub instances: Arc<Mutex<instance::DockerInstanceProvider>>,
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

    let app_config = Arc::new(Mutex::new(
        match config::ConfigFile::new(args.config_path).await {
            Ok(o) => o,
            Err(e) => {
                error!("Config error: {}", e);
                std::process::exit(1);
            }
        }
    ));

    let address = args.address.unwrap_or(String::from("127.0.0.1"));
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
            /* app_config.clone(), */
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
    };

    let http_handle = net::http::serve(address, port, app_state).await;

    match http_handle.await {
        Ok(_) => {},
        Err(e) => {
            error!("HTTP server error: {}", e);
        }
    }
}
