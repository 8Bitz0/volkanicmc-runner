use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse
};
use tracing::{error, info};

use crate::{AppState, global_event::GlobalEvent};

pub async fn del_instance(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    tokio::spawn(async move {
        info!("Instance deletion requested (\"{}\")", id);

        let instances_lock = state.instances.lock().await;

        match instances_lock.del_instance(&id).await {
            Ok(deleted) => {
                if deleted {
                    state.g_event_tx.send(GlobalEvent::DeleteInstance { id }).unwrap();
                }
            }
            Err(e) => {
                error!("Error deleting instance: {}", e);
            }
        };
    });

    (
        StatusCode::ACCEPTED,
        "",
    )
}