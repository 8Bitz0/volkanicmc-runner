use axum::{
    extract::State,
    http::StatusCode,
    Json,
    response::IntoResponse,
};
use tracing::{info, error};

use crate::{
    AppState,
    global_event::GlobalEvent,
    instance::InstanceRequest
};

pub async fn new_instance(
    State(state): State<AppState>,
    Json(payload): Json<InstanceRequest>,
) -> impl IntoResponse {
    let instances_lock = state.instances.lock().await;

    info!("New instance requested");

    let id = match instances_lock.new_instance(payload).await {
        Ok(id) => {
            info!("New instance created (\"{}\")", id);

            id.clone()
        }
        Err(e) => {
            error!("Error creating new instance: {}", e);
            
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "".to_string(),
            );
        }
    };

    // Drops the lock, preventing a deadlock
    drop(instances_lock);

    let new_instance = match state.instances.lock().await.get_instance(&id).await {
        Some(o) => o,
        None => {
            error!("Failed to get the newly created instance (ID: \"{}\")", id);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "".to_string(),
            );
        }
    };

    state.g_event_tx.send(GlobalEvent::ModifyInstance { id: id.clone(), instance: new_instance }).unwrap();

    (
        StatusCode::OK,
        id,
    )
}
