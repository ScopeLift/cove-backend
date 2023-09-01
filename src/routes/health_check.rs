use axum::http;

/// Health check route that returns a 200 OK status code if the server is running.
pub async fn health_check() -> http::StatusCode {
    http::StatusCode::OK
}
