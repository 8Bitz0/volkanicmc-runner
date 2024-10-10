use axum::{
    Router,
    routing::get,
};
use tracing::info;

#[derive(Debug, thiserror::Error)]
pub enum HttpServerError {
    #[error("HTTP I/O error: {0}")]
    Io(std::io::Error),
}

pub async fn serve(addr: String, port: u16) -> Result<(), HttpServerError> {
    let app = Router::new()
        .route("/", get(root));

    info!("Binding to {}:{}", addr, port);

    let listener = tokio::net::TcpListener::bind((addr, port)).await.unwrap();
    axum::serve(listener, app).await.map_err(HttpServerError::Io)?;

    Ok(())
}

async fn root() -> &'static str {
    "😳"
}
