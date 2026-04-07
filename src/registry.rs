use axum::{
    extract::{Path, Query, State},
    response::Response,
    Router,
};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::data::Data;
use crate::error::AppError;

/// Shared application state available to route handlers.
pub struct AppState {
    pub config: Arc<crate::config::Config>,
    pub cache: Arc<dyn crate::cache::CacheBackend>,
    pub http: crate::http::client::HttpClient,
    /// Override API base URLs (for testing with wiremock).
    /// Key: identifier (e.g. "hackernews"), Value: base URL.
    pub base_urls: HashMap<String, String>,
}

impl AppState {
    /// Get the API base URL for a service, falling back to the default.
    pub fn base_url(&self, service: &str, default: &str) -> String {
        self.base_urls
            .get(service)
            .cloned()
            .unwrap_or_else(|| default.to_string())
    }
}

/// Type alias for route handler functions.
pub type HandlerFn = fn(
    Arc<AppState>,
    HashMap<String, String>,
    HashMap<String, String>,
) -> Pin<Box<dyn Future<Output = Result<Data, AppError>> + Send>>;

/// Definition of a single route.
pub struct RouteDefinition {
    pub path: &'static str,
    pub name: &'static str,
    pub example: &'static str,
    pub handler: HandlerFn,
}

/// Register routes onto an axum Router.
///
/// Each route handler receives path params and query params,
/// returns Data which is inserted into response extensions for
/// middleware to process (template, header, parameter).
pub fn register_routes(
    app: Router<Arc<AppState>>,
    routes: Vec<RouteDefinition>,
) -> Router<Arc<AppState>> {
    let mut router = app;

    for route in routes {
        let handler = route.handler;

        router = router.route(
            route.path,
            axum::routing::get(
                move |State(state): State<Arc<AppState>>,
                      Path(path_params): Path<HashMap<String, String>>,
                      Query(query_params): Query<HashMap<String, String>>| async move {
                    let data = (handler)(state, path_params, query_params).await?;

                    let mut response = Response::new(axum::body::Body::empty());
                    response.extensions_mut().insert(data);
                    Ok::<_, AppError>(response)
                },
            ),
        );
    }

    router
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_definition_fields() {
        fn dummy_handler(
            _state: Arc<AppState>,
            _path: HashMap<String, String>,
            _query: HashMap<String, String>,
        ) -> Pin<Box<dyn Future<Output = Result<Data, AppError>> + Send>> {
            Box::pin(async { Ok(Data::new("Test")) })
        }

        let route = RouteDefinition {
            path: "/test/example",
            name: "test/example",
            example: "/test/example",
            handler: dummy_handler,
        };

        assert_eq!(route.path, "/test/example");
        assert_eq!(route.name, "test/example");
    }

    #[test]
    fn app_state_has_http_client() {
        let config = crate::config::Config {
            port: 0,
            cache_expire: 300,
            cache_type: crate::config::CacheType::Memory,
            redis_url: None,
            access_key: None,
            request_timeout: 30,
            item_limit: 50,
        };
        let cache = Arc::new(crate::cache::memory::MemoryCache::new(100, 300));
        let http = crate::http::client::HttpClient::new(&config);
        let _state = AppState {
            config: Arc::new(config),
            cache,
            http,
            base_urls: HashMap::new(),
        };
    }

    #[test]
    fn base_url_returns_override() {
        let config = crate::config::Config {
            port: 0,
            cache_expire: 300,
            cache_type: crate::config::CacheType::Memory,
            redis_url: None,
            access_key: None,
            request_timeout: 30,
            item_limit: 50,
        };
        let cache = Arc::new(crate::cache::memory::MemoryCache::new(100, 300));
        let http = crate::http::client::HttpClient::new(&config);
        let mut urls = HashMap::new();
        urls.insert("hackernews".to_string(), "http://localhost:9999/v0".to_string());
        let state = AppState {
            config: Arc::new(config),
            cache,
            http,
            base_urls: urls,
        };
        assert_eq!(state.base_url("hackernews", "https://default.com"), "http://localhost:9999/v0");
        assert_eq!(state.base_url("other", "https://default.com"), "https://default.com");
    }
}
