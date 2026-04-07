use std::env;

/// Application configuration, loaded from environment variables.
#[derive(Debug, Clone)]
pub struct Config {
    /// Server listen port (default: 1200, same as RSSHub)
    pub port: u16,
    /// Cache expiry in seconds (default: 300 = 5 minutes)
    pub cache_expire: u64,
    /// Cache backend: "memory" or "redis"
    pub cache_type: CacheType,
    /// Redis URL (when cache_type = Redis)
    pub redis_url: Option<String>,
    /// Access key for authentication (None = open access)
    pub access_key: Option<String>,
    /// Request timeout in seconds
    pub request_timeout: u64,
    /// Max items per feed (default: 50)
    pub item_limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CacheType {
    Memory,
    Redis,
}

impl Config {
    /// Load configuration from environment variables.
    ///
    /// Variable names follow RSSHub conventions where possible.
    pub fn from_env() -> Self {
        // Load .env file if present, ignore errors
        let _ = dotenvy::dotenv();

        Self {
            port: env_parse("PORT", 1200),
            cache_expire: env_parse("CACHE_EXPIRE", 300),
            cache_type: match env::var("CACHE_TYPE").as_deref() {
                Ok("redis") => CacheType::Redis,
                _ => CacheType::Memory,
            },
            redis_url: env::var("REDIS_URL").ok(),
            access_key: env::var("ACCESS_KEY").ok().filter(|s| !s.is_empty()),
            request_timeout: env_parse("REQUEST_TIMEOUT", 30),
            item_limit: env_parse("MAX_ITEM_COUNT", 50),
        }
    }
}

fn env_parse<T: std::str::FromStr>(key: &str, default: T) -> T {
    env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_parse_returns_default_when_missing() {
        env::remove_var("__OPENRSS_TEST_MISSING__");
        assert_eq!(env_parse::<u16>("__OPENRSS_TEST_MISSING__", 1200), 1200);
    }

    #[test]
    fn env_parse_reads_value() {
        env::set_var("__OPENRSS_TEST_PORT__", "8080");
        assert_eq!(env_parse::<u16>("__OPENRSS_TEST_PORT__", 1200), 8080);
        env::remove_var("__OPENRSS_TEST_PORT__");
    }

    #[test]
    fn env_parse_returns_default_on_bad_value() {
        env::set_var("__OPENRSS_TEST_BAD__", "not_a_number");
        assert_eq!(env_parse::<u16>("__OPENRSS_TEST_BAD__", 1200), 1200);
        env::remove_var("__OPENRSS_TEST_BAD__");
    }

    #[test]
    fn cache_type_from_env() {
        assert_eq!(
            match "redis" { "redis" => CacheType::Redis, _ => CacheType::Memory },
            CacheType::Redis
        );
        assert_eq!(
            match "memory" { "redis" => CacheType::Redis, _ => CacheType::Memory },
            CacheType::Memory
        );
    }
}
