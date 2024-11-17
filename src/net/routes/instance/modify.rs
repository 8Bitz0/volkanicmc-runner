use axum::{
    extract::State,
    http::StatusCode,
    Json,
    response::IntoResponse,
};
use tracing::{info, error};

use crate::{
    AppState,
    instance::InstanceRequest
};

pub async fn new_instance(
    State(state): State<AppState>,
    Json(payload): Json<InstanceRequest>,
) -> impl IntoResponse {
    info!("New instance requested");

    tokio::spawn(async move {
        let instances_lock = state.instances.lock().await;
        
        match instances_lock.new_instance(payload).await {
            Ok(id) => {
                info!("New instance created (\"{}\")", id);
            }
            Err(e) => {
                error!("Error creating new instance: {}", e);
            }
        };
    });

    (
        StatusCode::ACCEPTED,
        "",
    )
}
