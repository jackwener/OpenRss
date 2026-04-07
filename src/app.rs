use axum::{routing::get, Router};

use crate::config::Config;

/// Build the Axum application with all routes and middleware.
pub fn build_app(_config: &Config) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/healthz", get(healthz))
}

async fn index() -> &'static str {
    "OpenRss — Make any website your RSS feed."
}

async fn healthz() -> &'static str {
    "ok"
}
