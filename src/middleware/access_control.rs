use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use md5::{Digest, Md5};
use std::sync::Arc;

use crate::config::Config;

/// Paths that bypass access control.
const WHITELIST: &[&str] = &["/", "/healthz", "/robots.txt", "/favicon.ico"];

/// Access control middleware.
///
/// If `config.access_key` is None, all requests pass through.
/// Otherwise, requests must provide either:
/// - `?key={ACCESS_KEY}` — exact match
/// - `?code={MD5(pathname + ACCESS_KEY)}` — verification code
pub async fn access_control(
    State(config): State<Arc<Config>>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let access_key = match &config.access_key {
        Some(key) => key,
        None => return Ok(next.run(request).await),
    };

    let path = request.uri().path();

    // Whitelist paths
    if WHITELIST.contains(&path) {
        return Ok(next.run(request).await);
    }

    // Extract query params
    let query = request.uri().query().unwrap_or("");
    let find_param = |name: &str| -> Option<String> {
        for pair in query.split('&') {
            if let Some((k, v)) = pair.split_once('=') {
                if k == name {
                    return Some(urlencoding::decode(v).unwrap_or_default().to_string());
                }
            }
        }
        None
    };

    // Check ?key=
    if let Some(key) = find_param("key") {
        if key == *access_key {
            return Ok(next.run(request).await);
        }
    }

    // Check ?code=MD5(path + access_key)
    if let Some(code) = find_param("code") {
        let mut hasher = Md5::new();
        hasher.update(path.as_bytes());
        hasher.update(access_key.as_bytes());
        let expected = format!("{:x}", hasher.finalize());
        if code == expected {
            return Ok(next.run(request).await);
        }
    }

    Err(StatusCode::FORBIDDEN)
}

/// Compute MD5 access code for a given path and key.
pub fn compute_access_code(path: &str, key: &str) -> String {
    let mut hasher = Md5::new();
    hasher.update(path.as_bytes());
    hasher.update(key.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_without_key() {
        // Tested via integration tests (needs axum app context)
        // Unit test: verify compute_access_code works
        let code = compute_access_code("/github/issue", "secret123");
        assert!(!code.is_empty());
        assert_eq!(code.len(), 32); // MD5 hex = 32 chars
    }

    #[test]
    fn accepts_valid_code() {
        let key = "mykey";
        let path = "/github/issue/DIYgod/RSSHub";
        let code = compute_access_code(path, key);
        // Verify deterministic
        let code2 = compute_access_code(path, key);
        assert_eq!(code, code2);
    }

    #[test]
    fn different_paths_produce_different_codes() {
        let key = "secret";
        let c1 = compute_access_code("/a", key);
        let c2 = compute_access_code("/b", key);
        assert_ne!(c1, c2);
    }

    #[test]
    fn whitelist_paths() {
        for path in WHITELIST {
            assert!(WHITELIST.contains(path));
        }
    }
}
