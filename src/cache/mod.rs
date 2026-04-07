pub mod memory;

use std::future::Future;
use std::pin::Pin;

/// Cache backend trait for route-level and content-level caching.
pub trait CacheBackend: Send + Sync + 'static {
    fn get(&self, key: &str) -> Pin<Box<dyn Future<Output = Option<String>> + Send + '_>>;
    fn set(
        &self,
        key: &str,
        value: &str,
        ttl_secs: u64,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + '_>>;
}
