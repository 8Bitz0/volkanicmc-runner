use axum::{
    extract::State, middleware::Next, response::Response
};
use hyper::Request;
use tokio::time::Duration;
use tracing::info;

use crate::AppState;

pub async fn latency(
    State(state): State<AppState>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Response {
    if let Some(latency) = state.add_latency {
        info!("Adding an additional {} ms of latency to request", latency);
        tokio::time::sleep(Duration::from_millis(latency as u64)).await;
    }
    next.run(request).await
}
