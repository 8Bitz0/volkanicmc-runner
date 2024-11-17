use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse
};
use tracing::{error, info};

use crate::AppState;

pub async fn del_instance(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    tokio::spawn(async move {
        info!("Instance deletion requested (\"{}\")", id);

        let instances_lock = state.instances.lock().await;

        match instances_lock.del_instance(&id).await {
            Ok(_) => {
                info!("Instance deleted: {:?}", id);
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