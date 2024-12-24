use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse
};

use crate::AppState;

pub async fn start_instance(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    tokio::spawn(async move {
        let instances_lock = state.instances.lock().await;

        instances_lock.start_instance(&id).await;
    });

    (
        StatusCode::ACCEPTED,
        "",
    )
}

pub async fn stop_instance(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    tokio::spawn(async move {
        let instances_lock = state.instances.lock().await;

        instances_lock.stop_instance(&id).await;
    });

    (
        StatusCode::ACCEPTED,
        "",
    )
}
