use std::collections::HashMap;

use axum_test::TestServer;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use openrss::app::build_app_with_overrides;
use openrss::config::Config;
use openrss::http::client::HttpClient;

fn test_config() -> Config {
    Config {
        port: 0,
        cache_expire: 300,
        cache_type: openrss::config::CacheType::Memory,
        redis_url: None,
        access_key: None,
        request_timeout: 10,
        item_limit: 3,
    }
}

fn lobsters_stories() -> serde_json::Value {
    serde_json::json!([
        {
            "short_id": "abc123",
            "title": "Rust 2025 Edition",
            "url": "https://blog.rust-lang.org/2025",
            "description": "What's new in Rust",
            "comments_url": "https://lobste.rs/s/abc123",
            "comment_count": 25,
            "score": 80,
            "created_at": "2025-01-15T12:00:00.000-05:00",
            "submitter_user": { "username": "rustfan" },
            "tags": ["rust", "programming"]
        },
        {
            "short_id": "def456",
            "title": "SQLite Internals",
            "url": "https://example.com/sqlite",
            "description": "",
            "comments_url": "https://lobste.rs/s/def456",
            "comment_count": 10,
            "score": 45,
            "created_at": "2025-01-16T08:00:00.000-05:00",
            "submitter_user": { "username": "dbguru" },
            "tags": ["databases"]
        }
    ])
}

fn build_test_server(mock_url: &str) -> TestServer {
    let config = test_config();
    let http = HttpClient::new_no_proxy(&config);
    let mut base_urls = HashMap::new();
    base_urls.insert("lobsters".to_string(), mock_url.to_string());
    TestServer::new(build_app_with_overrides(&config, http, base_urls))
}

#[tokio::test]
async fn lobsters_full_chain_rss() {
    let mock = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/hottest.json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(lobsters_stories()))
        .mount(&mock)
        .await;

    let s = build_test_server(&mock.uri());

    let res: axum_test::TestResponse = s.get("/lobsters/hottest").await;
    res.assert_status_ok();

    let body = res.text();
    assert!(body.contains("<rss version=\"2.0\""));
    assert!(body.contains("<title>Lobsters - Hottest</title>"));
    assert!(body.contains("Rust 2025 Edition"));
    assert!(body.contains("SQLite Internals"));
    assert!(body.contains("<author>rustfan</author>"));
}

#[tokio::test]
async fn lobsters_full_chain_json() {
    let mock = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/hottest.json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(lobsters_stories()))
        .mount(&mock)
        .await;

    let s = build_test_server(&mock.uri());

    let res: axum_test::TestResponse = s.get("/lobsters/hottest?format=json").await;
    res.assert_status_ok();

    let v: serde_json::Value = serde_json::from_str(&res.text()).unwrap();
    assert_eq!(v["title"], "Lobsters - Hottest");
    let items = v["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    // Sorted by pub_date desc — SQLite (Jan 16) before Rust (Jan 15)
    assert_eq!(items[0]["title"], "SQLite Internals");
    assert_eq!(items[1]["title"], "Rust 2025 Edition");
    // Tags mapped to categories
    assert!(items[1]["tags"].as_array().unwrap().contains(&serde_json::json!("rust")));
}

#[tokio::test]
async fn lobsters_newest() {
    let mock = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/newest.json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(lobsters_stories()))
        .mount(&mock)
        .await;

    let s = build_test_server(&mock.uri());

    let res: axum_test::TestResponse = s.get("/lobsters/newest").await;
    res.assert_status_ok();
    assert!(res.text().contains("Lobsters - Newest"));
}

#[tokio::test]
async fn lobsters_alias_hot() {
    let mock = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/hottest.json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(lobsters_stories()))
        .mount(&mock)
        .await;

    let s = build_test_server(&mock.uri());

    // "hot" should alias to "hottest"
    let res: axum_test::TestResponse = s.get("/lobsters/hot").await;
    res.assert_status_ok();
    assert!(res.text().contains("Lobsters - Hottest"));
}

#[tokio::test]
async fn lobsters_invalid_category() {
    let mock = MockServer::start().await;
    let s = build_test_server(&mock.uri());

    let res: axum_test::TestResponse = s.get("/lobsters/invalid").expect_failure().await;
    res.assert_status(axum::http::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn lobsters_has_headers() {
    let mock = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/hottest.json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(lobsters_stories()))
        .mount(&mock)
        .await;

    let s = build_test_server(&mock.uri());

    let res: axum_test::TestResponse = s.get("/lobsters/hottest").await;
    assert_eq!(res.header("x-openrss-cache").to_str().unwrap(), "MISS");
    assert!(res.header("etag").to_str().unwrap().starts_with('"'));
    assert_eq!(
        res.header("access-control-allow-origin").to_str().unwrap(),
        "*"
    );
}
