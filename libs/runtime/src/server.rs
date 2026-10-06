//! Axum serving: listen address, request instrumentation, graceful shutdown.
use std::{future::Future, net::SocketAddr};

use axum::{Router, extract::MatchedPath, http::Request};
use sentry::integrations::tower::{NewSentryLayer, SentryHttpLayer};
use tower_http::trace::TraceLayer;
use tracing::{error, info};

use crate::{ConfigError, Exit, Telemetry};

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub listen_addr: SocketAddr,
    pub telemetry: Telemetry,
}

impl ServerConfig {
    /// `LISTEN_ADDR` | `default_listen_addr` plus [`Telemetry::from_env`].
    pub fn from_env(default_listen_addr: &str) -> Result<Self, ConfigError> {
        let raw = crate::var("LISTEN_ADDR").unwrap_or_else(|| default_listen_addr.into());
        let listen_addr = raw
            .parse()
            .map_err(|e| ConfigError::new(format!("LISTEN_ADDR {raw:?} is not host:port: {e}")))?;
        Ok(Self {
            listen_addr,
            telemetry: Telemetry::from_env()?,
        })
    }
}

/// Wraps `router` in request tracing and Sentry. Routes added after this call (e.g. `/healthz`) bypass both.
pub fn instrument<S>(router: Router<S>) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    router
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|req: &Request<_>| {
                    let matched_path = req.extensions().get::<MatchedPath>().map(MatchedPath::as_str);
                    tracing::debug_span!("request", method = %req.method(), uri = %req.uri(), matched_path)
                })
                // ignore 5xx - the handler's responsibility to report with context.
                .on_failure(()),
        )
        .layer(SentryHttpLayer::new().enable_transaction())
        // Outermost: a fresh Sentry hub per request so scope data does not leak across concurrent requests.
        .layer(NewSentryLayer::<Request<_>>::new_from_top())
}

/// Builds a Tokio runtime, binds `addr`, and serves `app` until `Ctrl+C` or `SIGTERM`.
///
/// Returns [`Exit::Success`] after a graceful shutdown and [`Exit::Failure`] if the runtime, bind or server fails.
pub fn serve<F>(addr: SocketAddr, app: F) -> Exit
where
    F: Future<Output = Router>,
{
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(err) => {
            error!(%err, "failed to start Tokio runtime");
            return Exit::Failure;
        }
    };

    runtime.block_on(async {
        let app = app.await;
        let listener = match tokio::net::TcpListener::bind(addr).await {
            Ok(listener) => listener,
            Err(err) => {
                error!(%addr, %err, "failed to bind");
                return Exit::Failure;
            }
        };
        info!(%addr, "listening");

        match axum::serve(listener, app)
            .with_graceful_shutdown(shutdown_signal())
            .await
        {
            Ok(()) => {
                info!("shutdown complete");
                Exit::Success
            }
            Err(err) => {
                error!(%err, "server error");
                Exit::Failure
            }
        }
    })
}

/// Resolves on Ctrl+C (SIGINT) or, on Unix, SIGTERM (`docker stop`).
async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(err) = tokio::signal::ctrl_c().await {
            error!(%err, "failed to listen for Ctrl+C");
            std::future::pending::<()>().await;
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(err) => {
                error!(%err, "failed to listen for SIGTERM");
                std::future::pending::<()>().await;
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => info!("received SIGINT (Ctrl+C), shutting down"),
        () = terminate => info!("received SIGTERM, shutting down"),
    }
}
