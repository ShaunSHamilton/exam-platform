use std::{ffi::OsString, path::PathBuf};

use axum::{Router, routing::get};
use runtime::ConfigError;
use tower_http::services::{ServeDir, ServeFile};

use crate::routes;

/// `WEB_DIR`: the built client, `client/dist`. Required in release builds, so an image without the
/// client fails to start instead of passing its health check while `/` answers 404. Optional in
/// debug builds, where Astro's dev server serves the client.
pub fn web_dir() -> Result<Option<PathBuf>, ConfigError> {
    parse_web_dir(std::env::var_os("WEB_DIR"))
}

fn parse_web_dir(raw: Option<OsString>) -> Result<Option<PathBuf>, ConfigError> {
    let Some(dir) = raw.filter(|v| !v.is_empty()).map(PathBuf::from) else {
        return if cfg!(debug_assertions) {
            Ok(None)
        } else {
            Err(ConfigError::new(
                "WEB_DIR must be set in release builds: the built client's directory",
            ))
        };
    };
    if !dir.join("index.html").is_file() {
        return Err(ConfigError::new(format!(
            "WEB_DIR {} has no index.html: build the client",
            dir.display()
        )));
    }
    Ok(Some(dir))
}

pub async fn app(web_dir: Option<PathBuf>) -> Router {
    let router = runtime::instrument(Router::new())
        // After instrumentation: probes stay out of request traces and Sentry.
        .route("/healthz", get(routes::get_healthz));
    match web_dir {
        // Also after instrumentation, as static files. A path with no file answers 404, with the
        // client's 404 page once it has one.
        Some(dir) => router.fallback_service(
            ServeDir::new(&dir).not_found_service(ServeFile::new(dir.join("404.html"))),
        ),
        None => router,
    }
}

#[cfg(test)]
mod tests {
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    use super::*;

    fn fixture() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/web")
    }

    async fn get(path: &str) -> (StatusCode, String) {
        let request = Request::get(path).body(Body::empty()).unwrap();
        let response = app(Some(fixture())).await.oneshot(request).await.unwrap();
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        (status, String::from_utf8(body.to_vec()).unwrap())
    }

    #[tokio::test]
    async fn serves_the_client_at_root() {
        let (status, body) = get("/").await;
        assert_eq!(status, StatusCode::OK);
        assert!(body.contains("<title>eDd fixture</title>"), "{body}");
    }

    #[tokio::test]
    async fn serves_client_assets() {
        let (status, body) = get("/_astro/app.js").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, "export {};\n");
    }

    #[tokio::test]
    async fn missing_file_is_404() {
        assert_eq!(get("/nope").await.0, StatusCode::NOT_FOUND);
        assert_eq!(get("/../Cargo.toml").await.0, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn healthz_is_not_a_file() {
        assert_eq!(get("/healthz").await, (StatusCode::OK, "ok".into()));
    }

    #[test]
    fn web_dir_needs_index_html() {
        assert_eq!(
            parse_web_dir(Some(fixture().into())).unwrap(),
            Some(fixture())
        );
        let empty = fixture().join("_astro");
        assert!(parse_web_dir(Some(empty.into())).is_err());
        assert!(parse_web_dir(Some("/does/not/exist".into())).is_err());
    }

    #[test]
    fn web_dir_is_optional_in_debug_builds_only() {
        let unset = parse_web_dir(None);
        let empty = parse_web_dir(Some("".into()));
        if cfg!(debug_assertions) {
            assert_eq!(unset.unwrap(), None);
            assert_eq!(empty.unwrap(), None);
        } else {
            assert!(unset.is_err() && empty.is_err());
        }
    }
}
