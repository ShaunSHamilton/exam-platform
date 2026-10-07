// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::process::ExitCode;

use runtime::{Exit, Telemetry, telemetry::DEFAULT_TRACES_SAMPLE_RATE};

fn main() -> ExitCode {
    let telemetry = match env!("ENVIRONMENT").parse().and_then(|environment| {
        Telemetry::new(
            environment,
            Some(env!("SENTRY_DSN")),
            DEFAULT_TRACES_SAMPLE_RATE,
        )
    }) {
        Ok(telemetry) => telemetry,
        Err(err) => {
            eprintln!("{err}");
            return Exit::Config.into();
        }
    };
    let _sentry = runtime::telemetry::init(runtime::release!(), &telemetry);

    let app = match tauri::Builder::default().build(tauri::generate_context!()) {
        Ok(app) => app,
        Err(err) => {
            tracing::error!(%err, "failed to build Tauri application");
            return Exit::Failure.into();
        }
    };

    // `run_return`, not `run`: `run` exits the process itself, skipping the Sentry guard's flush. Tauri's own exit code passes through.
    let code = app.run_return(|_, _| {});
    ExitCode::from(u8::try_from(code).unwrap_or(Exit::Failure.code()))
}
