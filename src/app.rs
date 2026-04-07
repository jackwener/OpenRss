use axum::{middleware, routing::get, Router};
use std::sync::Arc;

use crate::cache::memory::MemoryCache;
use crate::config::Config;
use crate::http::client::HttpClient;
use crate::middleware::{cache::CacheState, header, template};
use crate::registry::{self, AppState};
use crate::routes;

/// Build the Axum application with all routes and middleware.
pub fn build_app(config: &Config) -> Router {
    let cache_backend = Arc::new(MemoryCache::new(1000, config.cache_expire));
    let config = Arc::new(config.clone());

    let http_client = HttpClient::new(&config);

    let app_state = Arc::new(AppState {
        config: config.clone(),
        cache: cache_backend.clone(),
        http: http_client,
    });

    let cache_state = Arc::new(CacheState {
        backend: cache_backend,
        config: config.clone(),
    });

    // Register routes
    let app = Router::new()
        .route("/", get(index))
        .route("/healthz", get(healthz));

    let app = registry::register_routes(app, routes::test::routes());
    let app = registry::register_routes(app, routes::hackernews::routes());

    // Middleware stack — last .layer() = outermost (runs first for request).
    //
    // Request flow:  access_control → header.before → template.before → parameter.before → cache → handler
    // Response flow: handler → cache.after → parameter.after → template.after → header.after → access_control
    //
    // cache is innermost so that on HIT, Data in extensions flows through
    // parameter (filtering), template (rendering), and header (ETag/CORS).
    app.layer(middleware::from_fn_with_state(
        cache_state,
        crate::middleware::cache::cache_middleware,
    ))
    .layer(middleware::from_fn(parameter_mw))
    .layer(middleware::from_fn(template::template))
    .layer(middleware::from_fn_with_state(
        config.clone(),
        header::header_middleware,
    ))
    .layer(middleware::from_fn_with_state(
        config,
        crate::middleware::access_control::access_control,
    ))
    .with_state(app_state)
}

/// Parameter middleware that applies filter/sort/limit from query params.
async fn parameter_mw(
    request: axum::extract::Request,
    next: middleware::Next,
) -> axum::response::Response {
    let query = request.uri().query().map(|q| q.to_string());
    let mut response = next.run(request).await;

    if let Some(data) = response.extensions_mut().remove::<crate::data::Data>() {
        let params =
            crate::middleware::parameter::FilterParams::from_query(query.as_deref());
        let mut filtered_data = data;
        crate::middleware::parameter::apply_filters(&mut filtered_data, &params);
        response.extensions_mut().insert(filtered_data);
    }

    response
}

async fn index() -> &'static str {
    "OpenRss — Make any website your RSS feed."
}

async fn healthz() -> &'static str {
    "ok"
}
