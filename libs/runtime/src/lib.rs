//! Shared bootstrap for the platform's Rust bins.
//!
//! - [`Environment`]: `development`, `staging` or `production`, from `ENVIRONMENT`.
//! - [`telemetry`]: tracing to stdio and Sentry. Development logs to stdout and Sentry is
//!   optional; staging and production require Sentry and write only errors to stderr.
//! - [`server`] (feature `server`): Axum serving with Ctrl+C/SIGTERM graceful shutdown.
//! - [`Exit`]: the exit codes every bin returns.
mod environment;
mod exit;
#[cfg(feature = "server")]
pub mod server;
pub mod telemetry;

pub use environment::Environment;
pub use exit::Exit;
pub use sentry;
#[cfg(feature = "server")]
pub use server::{ServerConfig, instrument, serve};
pub use telemetry::Telemetry;

/// Sentry release name of the calling crate, `name@version`.
#[macro_export]
macro_rules! release {
    () => {
        concat!(env!("CARGO_PKG_NAME"), "@", env!("CARGO_PKG_VERSION"))
    };
}

/// Configuration error: a missing or malformed environment variable.
#[derive(Debug)]
pub struct ConfigError(String);

impl ConfigError {
    pub fn new(msg: impl Into<String>) -> Self {
        Self(msg.into())
    }
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "configuration error: {}", self.0)
    }
}

impl std::error::Error for ConfigError {}

/// Reads `key`, treating an empty value as unset so compose's `${VAR:-}` means "not set".
pub(crate) fn var(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|v| !v.is_empty())
}
