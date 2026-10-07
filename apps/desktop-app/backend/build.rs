//! Bakes `ENVIRONMENT` and `SENTRY_DSN` into the binary, as Vite does for the frontend: dApp
//! runs on candidate machines, so there is no deployment environment to read at runtime.
//! `ENVIRONMENT` defaults to `production`. Process env wins over `../.env`, so CI can inject
//! values without a file.
use std::{collections::HashMap, env};

const KEYS: [&str; 2] = ["ENVIRONMENT", "SENTRY_DSN"];
const DEFAULT_ENVIRONMENT: &str = "production";
const ENVIRONMENTS: [&str; 3] = ["development", "staging", "production"];

fn main() {
    println!("cargo:rerun-if-changed=../.env");
    for key in KEYS {
        println!("cargo:rerun-if-env-changed={key}");
    }

    let file: HashMap<String, String> = dotenvy::from_path_iter("../.env")
        .map(|iter| iter.filter_map(Result::ok).collect())
        .unwrap_or_default();
    let get = |key: &str| {
        env::var(key)
            .ok()
            .or_else(|| file.get(key).cloned())
            .filter(|value| !value.is_empty())
    };

    // Fail the build, not the candidate's launch.
    let environment = get("ENVIRONMENT").unwrap_or_else(|| DEFAULT_ENVIRONMENT.to_string());
    assert!(
        ENVIRONMENTS.contains(&environment.as_str()),
        "ENVIRONMENT must be one of {ENVIRONMENTS:?}, found {environment:?}"
    );
    let sentry_dsn = get("SENTRY_DSN").unwrap_or_default();
    assert!(
        environment == "development" || !sentry_dsn.is_empty(),
        "SENTRY_DSN is required when ENVIRONMENT={environment}"
    );

    println!("cargo:rustc-env=ENVIRONMENT={environment}");
    println!("cargo:rustc-env=SENTRY_DSN={sentry_dsn}");

    tauri_build::build()
}
