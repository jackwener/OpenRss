use moka::future::Cache;
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use super::CacheBackend;

/// In-memory LRU cache backed by moka.
///
/// TTL is set globally at construction time. Per-entry TTL override
/// is deferred to Phase 2 (requires moka `Expiry` trait).
pub struct MemoryCache {
    inner: Cache<String, String>,
}

impl MemoryCache {
    pub fn new(max_capacity: u64, ttl_secs: u64) -> Self {
        Self {
            inner: Cache::builder()
                .max_capacity(max_capacity)
                .time_to_live(Duration::from_secs(ttl_secs))
                .build(),
        }
    }
}

impl CacheBackend for MemoryCache {
    fn get(&self, key: &str) -> Pin<Box<dyn Future<Output = Option<String>> + Send + '_>> {
        let key = key.to_string();
        Box::pin(async move { self.inner.get(&key).await })
    }

    fn set(
        &self,
        key: &str,
        value: &str,
        _ttl_secs: u64,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        let key = key.to_string();
        let value = value.to_string();
        Box::pin(async move {
            self.inner.insert(key, value).await;
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn memory_cache_get_set() {
        let cache = MemoryCache::new(100, 300);
        assert!(cache.get("k1").await.is_none());

        cache.set("k1", "v1", 60).await;
        assert_eq!(cache.get("k1").await.as_deref(), Some("v1"));
    }

    #[tokio::test]
    async fn memory_cache_overwrites() {
        let cache = MemoryCache::new(100, 300);
        cache.set("k1", "v1", 60).await;
        cache.set("k1", "v2", 60).await;
        assert_eq!(cache.get("k1").await.as_deref(), Some("v2"));
    }
}
