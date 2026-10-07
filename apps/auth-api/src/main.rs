//! aAPI: candidate identity and the attempt path. The only application candidate devices reach.
mod app;
mod routes;

use runtime::{Exit, ServerConfig};

fn main() -> Exit {
    let config = match ServerConfig::from_env("127.0.0.1:13001") {
        Ok(config) => config,
        Err(err) => {
            // Telemetry is not up yet: config decides where it goes.
            eprintln!("{err}");
            return Exit::Config;
        }
    };

    // Before the Tokio runtime starts, so every worker thread inherits the Sentry hub.
    let _sentry = runtime::telemetry::init(runtime::release!(), &config.telemetry);

    runtime::serve(config.listen_addr, app::app())
}
