use clap::Parser;
use tracing::error;

mod net;

const DEBUG_MODE_VAR: &str = "VK_DEBUG";

#[derive(Debug, Parser)]
struct Args {
    #[arg(short = 'a', long)]
    pub address: Option<String>,
    #[arg(short = 'p', long)]
    pub port: Option<u16>,
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

    let address = args.address.unwrap_or(String::from("127.0.0.1"));
    let port = args.port.unwrap_or(8080);

    match net::http::serve(address, port).await {
        Ok(_) => {},
        Err(e) => {
            error!("HTTP server error: {}", e);
        }
    };
}
