use axum::{middleware, routing::{get, post}, Router};
use tokio::sync::oneshot;
use tower_http::trace::{self, TraceLayer};
use tracing::{info, Level};

use crate::AppState;

use super::routes;

pub const PROTOCOL_VER: u32 = 1;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("HTTP I/O error: {0}")]
    Io(std::io::Error),
}

pub async fn serve(addr: String, port: u16, state: AppState) -> oneshot::Receiver<Result<(), Error>> {
    let (tx, rx) = oneshot::channel();

    tokio::spawn(async move {
        let tx = tx;

        let app = Router::new()
            .route("/", get(root))
            .route("/check", get(routes::heartbeat::heartbeat))
            .route("/info", get(routes::info::info))
            .route("/auth", get(routes::auth::login))
            .route("/events", get(routes::event::global_event_sub))
            .route("/instance/list", get(routes::instance::get::list_instances))
            .route("/instance/new", post(routes::instance::modify::new_instance))
            .route("/instance/:id/delete", post(routes::instance::del::del_instance))
            .route("/instance/:id/start", post(routes::instance::trigger_status::start_instance))
            .route("/instance/:id/stop", post(routes::instance::trigger_status::stop_instance))
            .route("/internal/host/auth", post(routes::host::auth))
            .route("/internal/host/check", post(routes::host::heartbeat))
            .route("/internal/host/def", get(routes::host::definition::get_def))
            .layer(
                TraceLayer::new_for_http()
                    .on_request(trace::DefaultOnRequest::new()
                        .level(Level::DEBUG)
                    )
                    .on_response(trace::DefaultOnResponse::new()
                        .level(Level::DEBUG)
                    )
                    .on_failure(trace::DefaultOnFailure::new()
                        .level(Level::ERROR)
                    )
            )
            .layer(middleware::from_fn_with_state(state.clone(), super::middleware::latency))
            .with_state(state);

        info!("Binding to {}:{}", addr, port);

        let listener = tokio::net::TcpListener::bind((addr, port)).await.unwrap();
        let r = axum::serve(listener, app).await.map_err(Error::Io);

        info!("HTTP server closed");

        let _ = tx.send(r);
    });

    rx
}

async fn root() -> &'static str {
    "VolkanicMC Runner is active\n\nFor more details, check: https://github.com/8Bitz0/volkanicmc-runner"
}
