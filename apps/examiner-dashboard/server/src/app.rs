use axum::{Router, routing::get};

use crate::routes;

pub async fn app() -> Router {
    runtime::instrument(Router::new())
        // After instrumentation: probes stay out of request traces and Sentry.
        .route("/healthz", get(routes::get_healthz))
}
