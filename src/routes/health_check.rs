use axum::http;
use uuid::Uuid;

#[tracing::instrument(
    name = "Health check",
    fields(request_id = %Uuid::new_v4())
)]
pub async fn health_check() -> http::StatusCode {
    http::StatusCode::OK
}
