use reqwest::Client;
use serde::de::DeserializeOwned;
use std::time::Duration;

use crate::config::Config;
use crate::error::AppError;

/// User-Agent pool for rotation.
const USER_AGENTS: &[&str] = &[
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.2 Safari/605.1.15",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:121.0) Gecko/20100101 Firefox/121.0",
];

/// Max retry attempts for transient errors.
const MAX_RETRIES: u32 = 3;

/// HTTP status codes that trigger retry.
fn is_retryable_status(status: reqwest::StatusCode) -> bool {
    matches!(
        status.as_u16(),
        408 | 429 | 500 | 502 | 503 | 504
    )
}

/// Shared HTTP client with UA rotation, timeout, and retry.
pub struct HttpClient {
    inner: Client,
    timeout: Duration,
}

impl HttpClient {
    pub fn new(config: &Config) -> Self {
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .build()
            .expect("failed to build reqwest client");

        Self {
            inner: client,
            timeout: Duration::from_secs(config.request_timeout),
        }
    }

    /// Create a client that bypasses system proxy (useful for tests with wiremock).
    pub fn new_no_proxy(config: &Config) -> Self {
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .no_proxy()
            .build()
            .expect("failed to build reqwest client");

        Self {
            inner: client,
            timeout: Duration::from_secs(config.request_timeout),
        }
    }

    /// GET a URL and return the response body as a string.
    pub async fn get(&self, url: &str) -> Result<String, AppError> {
        let response = self.get_with_retry(url).await?;
        response.text().await.map_err(AppError::Http)
    }

    /// GET a URL and deserialize the JSON response.
    pub async fn get_json<T: DeserializeOwned>(&self, url: &str) -> Result<T, AppError> {
        let response = self.get_with_retry(url).await?;
        response.json::<T>().await.map_err(AppError::Http)
    }

    async fn get_with_retry(&self, url: &str) -> Result<reqwest::Response, AppError> {
        let mut last_err = None;

        for attempt in 0..=MAX_RETRIES {
            if attempt > 0 {
                // Exponential backoff: 200ms, 400ms, 800ms
                let delay = Duration::from_millis(200 * (1 << (attempt - 1)));
                tokio::time::sleep(delay).await;
            }

            let ua = random_ua();
            let result = self
                .inner
                .get(url)
                .header("User-Agent", ua)
                .timeout(self.timeout)
                .send()
                .await;

            match result {
                Ok(resp) => {
                    if resp.status().is_success() {
                        return Ok(resp);
                    }
                    if is_retryable_status(resp.status()) && attempt < MAX_RETRIES {
                        last_err = Some(AppError::Internal(format!(
                            "HTTP {} from {}",
                            resp.status(),
                            url
                        )));
                        continue;
                    }
                    // Non-retryable error status
                    return Err(AppError::Internal(format!(
                        "HTTP {} from {}",
                        resp.status(),
                        url
                    )));
                }
                Err(e) => {
                    if attempt < MAX_RETRIES {
                        last_err = Some(AppError::Http(e));
                        continue;
                    }
                    return Err(AppError::Http(e));
                }
            }
        }

        Err(last_err.unwrap_or_else(|| AppError::Internal("request failed".into())))
    }
}

fn random_ua() -> &'static str {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let idx = COUNTER.fetch_add(1, Ordering::Relaxed) % USER_AGENTS.len();
    USER_AGENTS[idx]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_ua_rotates() {
        let ua1 = random_ua();
        let ua2 = random_ua();
        // After enough calls, we should cycle through different UAs
        let mut seen = std::collections::HashSet::new();
        for _ in 0..10 {
            seen.insert(random_ua());
        }
        assert!(seen.len() > 1);
        // All returned UAs should be valid
        assert!(!ua1.is_empty());
        assert!(!ua2.is_empty());
    }

    #[test]
    fn retryable_statuses() {
        assert!(is_retryable_status(reqwest::StatusCode::TOO_MANY_REQUESTS));
        assert!(is_retryable_status(reqwest::StatusCode::INTERNAL_SERVER_ERROR));
        assert!(is_retryable_status(reqwest::StatusCode::BAD_GATEWAY));
        assert!(is_retryable_status(reqwest::StatusCode::SERVICE_UNAVAILABLE));
        assert!(is_retryable_status(reqwest::StatusCode::GATEWAY_TIMEOUT));
        assert!(is_retryable_status(reqwest::StatusCode::REQUEST_TIMEOUT));
        // Not retryable
        assert!(!is_retryable_status(reqwest::StatusCode::NOT_FOUND));
        assert!(!is_retryable_status(reqwest::StatusCode::FORBIDDEN));
        assert!(!is_retryable_status(reqwest::StatusCode::OK));
    }

    #[test]
    fn client_builds() {
        let config = Config {
            port: 0,
            cache_expire: 300,
            cache_type: crate::config::CacheType::Memory,
            redis_url: None,
            access_key: None,
            request_timeout: 30,
            item_limit: 50,
        };
        let _client = HttpClient::new(&config);
    }
}
