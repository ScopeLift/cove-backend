use axum::http;

pub async fn health_check() -> http::StatusCode {
    http::StatusCode::OK
}
