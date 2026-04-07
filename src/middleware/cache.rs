use axum::{
    extract::{Request, State},
    http::header,
    middleware::Next,
    response::Response,
};
use std::sync::Arc;
use xxhash_rust::xxh64::xxh64;

use crate::cache::CacheBackend;
use crate::config::Config;
use crate::data::Data;

/// Shared state for cache middleware.
pub struct CacheState {
    pub backend: Arc<dyn CacheBackend>,
    pub config: Arc<Config>,
}

/// Cache middleware: checks cache before handler, stores result after.
///
/// Cache key: `openrss:route:` + XXH64(path + ":" + format + ":" + limit)
pub async fn cache_middleware(
    State(state): State<Arc<CacheState>>,
    request: Request,
    next: Next,
) -> Response {
    // Skip cache if Cache-Control: no-cache
    let skip_cache = request
        .headers()
        .get(header::CACHE_CONTROL)
        .and_then(|v| v.to_str().ok())
        .map_or(false, |v| v.contains("no-cache"));

    let cache_key = compute_cache_key(request.uri());

    // Check cache (unless no-cache)
    if !skip_cache {
        if let Some(cached) = state.backend.get(&cache_key).await {
            if let Ok(data) = serde_json::from_str::<Data>(&cached) {
                let mut response = Response::new(axum::body::Body::empty());
                response.extensions_mut().insert(data);
                response.headers_mut().insert(
                    "X-OpenRss-Cache",
                    "HIT".parse().unwrap(),
                );
                return response;
            }
        }
    }

    // Cache MISS — run handler
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert("X-OpenRss-Cache", "MISS".parse().unwrap());

    // Store Data in cache if present
    if let Some(data) = response.extensions().get::<Data>() {
        if let Ok(serialized) = serde_json::to_string(data) {
            state
                .backend
                .set(&cache_key, &serialized, state.config.cache_expire)
                .await;
        }
    }

    response
}

/// Compute cache key from URI.
fn compute_cache_key(uri: &axum::http::Uri) -> String {
    let path = uri.path();
    let query = uri.query().unwrap_or("");

    // Extract format and limit from query for key computation
    let mut format = "rss";
    let mut limit = "";
    for pair in query.split('&') {
        if let Some(v) = pair.strip_prefix("format=") {
            format = v;
        } else if let Some(v) = pair.strip_prefix("limit=") {
            limit = v;
        }
    }

    let key_input = format!("{}:{}:{}", path, format, limit);
    let hash = xxh64(key_input.as_bytes(), 0);
    format!("openrss:route:{:016x}", hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_key_format() {
        let uri: axum::http::Uri = "/github/issue?format=atom&limit=10".parse().unwrap();
        let key = compute_cache_key(&uri);
        assert!(key.starts_with("openrss:route:"));
        assert_eq!(key.len(), "openrss:route:".len() + 16);
    }

    #[test]
    fn cache_key_deterministic() {
        let uri: axum::http::Uri = "/test/example?format=json".parse().unwrap();
        assert_eq!(compute_cache_key(&uri), compute_cache_key(&uri));
    }

    #[test]
    fn cache_key_differs_by_format() {
        let u1: axum::http::Uri = "/test?format=rss".parse().unwrap();
        let u2: axum::http::Uri = "/test?format=atom".parse().unwrap();
        assert_ne!(compute_cache_key(&u1), compute_cache_key(&u2));
    }

    #[test]
    fn cache_key_differs_by_path() {
        let u1: axum::http::Uri = "/a".parse().unwrap();
        let u2: axum::http::Uri = "/b".parse().unwrap();
        assert_ne!(compute_cache_key(&u1), compute_cache_key(&u2));
    }

    #[test]
    fn cache_key_default_format() {
        let u1: axum::http::Uri = "/test".parse().unwrap();
        let u2: axum::http::Uri = "/test?format=rss".parse().unwrap();
        // Both should produce the same key since default is rss
        assert_eq!(compute_cache_key(&u1), compute_cache_key(&u2));
    }
}
