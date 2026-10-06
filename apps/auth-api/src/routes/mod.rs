use axum::http::header::{CACHE_CONTROL, HeaderValue};
use axum::response::IntoResponse;

pub async fn get_healthz() -> impl IntoResponse {
    (
        [(CACHE_CONTROL, HeaderValue::from_static("no-store"))],
        "ok",
    )
}
