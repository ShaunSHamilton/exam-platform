//! Tracing and Sentry.
//!
//! | Environment            | stdio                         | Sentry                         |
//! |------------------------|-------------------------------|--------------------------------|
//! | development            | stdout, `RUST_LOG` (`info`)   | only if `SENTRY_DSN` is set    |
//! | staging, production    | stderr, `ERROR` only          | required; `RUST_LOG` (`info`)  |
//!
//! Sentry receives `ERROR` as events and logs, `WARN`/`INFO` as breadcrumbs and logs, and
//! drops `DEBUG`/`TRACE`.
use std::{sync::Arc, time::Duration};

use sentry::{
    ClientInitGuard, ClientOptions, Transport, TransportFactory, TransportOptions,
    integrations::tracing::EventFilter, transports::ReqwestHttpTransportOptions, types::Dsn,
};
use tracing::Level;
use tracing_subscriber::{
    EnvFilter, Layer, filter::LevelFilter, layer::SubscriberExt, util::SubscriberInitExt,
};

use crate::{ConfigError, Environment};

pub const DEFAULT_TRACES_SAMPLE_RATE: f32 = 0.2;
/// Bounds on every request to Sentry. Without them an unreachable Sentry holds the process
/// open at exit for the OS TCP connect timeout (minutes), past `docker stop`'s grace period.
const SENTRY_CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
const SENTRY_REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone)]
pub struct Telemetry {
    pub environment: Environment,
    pub sentry_dsn: Option<Dsn>,
    /// Fraction of transactions sent to Sentry, `0.0..=1.0`.
    pub traces_sample_rate: f32,
}

impl Telemetry {
    /// From `ENVIRONMENT`, `SENTRY_DSN` and `SENTRY_TRACES_SAMPLE_RATE`.
    pub fn from_env() -> Result<Self, ConfigError> {
        let traces_sample_rate = match crate::var("SENTRY_TRACES_SAMPLE_RATE") {
            Some(v) => v
                .parse()
                .map_err(|_| ConfigError::new("SENTRY_TRACES_SAMPLE_RATE must be a number"))?,
            None => DEFAULT_TRACES_SAMPLE_RATE,
        };
        Self::new(
            Environment::from_env()?,
            crate::var("SENTRY_DSN").as_deref(),
            traces_sample_rate,
        )
    }

    /// From values the caller already holds, e.g. baked in at compile time.
    /// An empty `sentry_dsn` counts as unset.
    pub fn new(
        environment: Environment,
        sentry_dsn: Option<&str>,
        traces_sample_rate: f32,
    ) -> Result<Self, ConfigError> {
        let sentry_dsn = sentry_dsn
            .filter(|dsn| !dsn.is_empty())
            .map(|dsn| {
                dsn.parse::<Dsn>()
                    .map_err(|e| ConfigError::new(format!("SENTRY_DSN is not a valid DSN: {e}")))
            })
            .transpose()?;
        if sentry_dsn.is_none() && !environment.is_development() {
            return Err(ConfigError::new(format!(
                "SENTRY_DSN is required when ENVIRONMENT={environment}"
            )));
        }
        if !(0.0..=1.0).contains(&traces_sample_rate) {
            return Err(ConfigError::new(
                "SENTRY_TRACES_SAMPLE_RATE must be within 0.0..=1.0",
            ));
        }
        Ok(Self {
            environment,
            sentry_dsn,
            traces_sample_rate,
        })
    }
}

/// Initialises Sentry, then the global tracing subscriber.
///
/// Call once, first thing in `main` and before any async runtime starts, so every thread
/// inherits the Sentry hub. Keep the guard alive until exit: dropping it flushes events.
#[must_use = "dropping the guard flushes and disables Sentry"]
pub fn init(release: &'static str, telemetry: &Telemetry) -> Option<ClientInitGuard> {
    let guard = telemetry.sentry_dsn.clone().map(|dsn| {
        let mut options = ClientOptions::new()
            .release(release)
            .environment(telemetry.environment.as_str())
            .traces_sample_rate(telemetry.traces_sample_rate)
            // Candidate data is sensitive personal data: never attach IPs, cookies or bodies.
            .send_default_pii(false)
            .attach_stacktrace(true)
            .transport(BoundedTransport);
        options.dsn = Some(dsn);
        sentry::init(options)
    });

    let stdio = if telemetry.environment.is_development() {
        tracing_subscriber::fmt::layer()
            .with_filter(env_filter())
            .boxed()
    } else {
        tracing_subscriber::fmt::layer()
            .with_writer(std::io::stderr)
            // Collected by log drivers, not read in a terminal.
            .with_ansi(false)
            .with_filter(LevelFilter::ERROR)
            .boxed()
    };

    let sentry = guard.is_some().then(|| {
        sentry::integrations::tracing::layer()
            .event_filter(|md| match *md.level() {
                Level::ERROR => EventFilter::Event | EventFilter::Log,
                Level::WARN | Level::INFO => EventFilter::Breadcrumb | EventFilter::Log,
                Level::DEBUG | Level::TRACE => EventFilter::Ignore,
            })
            .with_filter(env_filter())
    });

    tracing_subscriber::registry()
        .with(stdio)
        .with(sentry)
        .init();

    guard
}

/// Sentry's reqwest transport with [`SENTRY_CONNECT_TIMEOUT`] and [`SENTRY_REQUEST_TIMEOUT`].
struct BoundedTransport;

impl TransportFactory for BoundedTransport {
    fn create_transport_with_options(&self, options: TransportOptions) -> Arc<dyn Transport> {
        let transport = ReqwestHttpTransportOptions::from(options);
        let transport = match reqwest::Client::builder()
            .connect_timeout(SENTRY_CONNECT_TIMEOUT)
            .timeout(SENTRY_REQUEST_TIMEOUT)
            .build()
        {
            Ok(client) => transport.with_client(client),
            // Unbounded, but still delivering, beats dropping every event.
            Err(err) => {
                eprintln!("sentry: bounded HTTP client unavailable, using default: {err}");
                transport
            }
        };
        Arc::new(transport.build())
    }
}

fn env_filter() -> EnvFilter {
    EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const DSN: &str = "https://public@sentry.example.com/1";

    #[test]
    fn development_does_not_need_sentry() {
        let t = Telemetry::new(Environment::Development, None, 0.2).unwrap();
        assert!(t.sentry_dsn.is_none());
        let t = Telemetry::new(Environment::Development, Some(""), 0.2).unwrap();
        assert!(t.sentry_dsn.is_none(), "empty DSN counts as unset");
    }

    #[test]
    fn staging_and_production_require_sentry() {
        for env in [Environment::Staging, Environment::Production] {
            assert!(Telemetry::new(env, None, 0.2).is_err());
            assert!(Telemetry::new(env, Some(""), 0.2).is_err());
            assert!(
                Telemetry::new(env, Some(DSN), 0.2)
                    .unwrap()
                    .sentry_dsn
                    .is_some()
            );
        }
    }

    #[test]
    fn rejects_invalid_dsn_and_sample_rate() {
        assert!(Telemetry::new(Environment::Development, Some("not a dsn"), 0.2).is_err());
        assert!(Telemetry::new(Environment::Production, Some(DSN), 1.5).is_err());
        assert!(Telemetry::new(Environment::Production, Some(DSN), -0.1).is_err());
    }
}
