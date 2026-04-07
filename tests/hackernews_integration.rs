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

async fn setup_hn_mock() -> MockServer {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v0/topstories.json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(vec![1u64, 2, 3]))
        .mount(&server)
        .await;

    for i in 1u64..=3 {
        Mock::given(method("GET"))
            .and(path(format!("/v0/item/{i}.json")))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "id": i,
                    "title": format!("HN Story {i}"),
                    "url": format!("https://example.com/{i}"),
                    "by": "testuser",
                    "score": 100 + i as i64,
                    "descendants": 10 + i as i64,
                    "time": 1700000000 + i as i64 * 3600
                })),
            )
            .mount(&server)
            .await;
    }

    server
}

fn build_test_server(mock_url: &str, service: &str) -> TestServer {
    let config = test_config();
    let http = HttpClient::new_no_proxy(&config);
    let mut base_urls = HashMap::new();
    base_urls.insert(service.to_string(), format!("{mock_url}/v0"));
    TestServer::new(build_app_with_overrides(&config, http, base_urls))
}

#[tokio::test]
async fn hn_full_chain_rss() {
    let mock = setup_hn_mock().await;
    let s = build_test_server(&mock.uri(), "hackernews");

    let res: axum_test::TestResponse = s.get("/hackernews/top").await;
    res.assert_status_ok();

    let body = res.text();
    assert!(body.contains("<rss version=\"2.0\""));
    assert!(body.contains("<title>Hacker News - Top Stories</title>"));
    assert!(body.contains("HN Story 1"));
    assert!(body.contains("HN Story 2"));
    assert!(body.contains("HN Story 3"));
    assert!(body.contains("<author>testuser</author>"));
}

#[tokio::test]
async fn hn_full_chain_json() {
    let mock = setup_hn_mock().await;
    let s = build_test_server(&mock.uri(), "hackernews");

    let res: axum_test::TestResponse = s.get("/hackernews/top?format=json").await;
    res.assert_status_ok();

    let v: serde_json::Value = serde_json::from_str(&res.text()).unwrap();
    assert_eq!(v["title"], "Hacker News - Top Stories");
    let items = v["items"].as_array().unwrap();
    assert_eq!(items.len(), 3);
    assert_eq!(items[0]["title"], "HN Story 3"); // Newest first (sorted by pub_date desc)
}

#[tokio::test]
async fn hn_invalid_category_returns_404() {
    let mock = setup_hn_mock().await;
    let s = build_test_server(&mock.uri(), "hackernews");

    let res: axum_test::TestResponse = s.get("/hackernews/invalid").expect_failure().await;
    res.assert_status(axum::http::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn hn_has_cache_and_etag_headers() {
    let mock = setup_hn_mock().await;
    let s = build_test_server(&mock.uri(), "hackernews");

    let res: axum_test::TestResponse = s.get("/hackernews/top").await;
    assert_eq!(res.header("x-openrss-cache").to_str().unwrap(), "MISS");
    assert!(res.header("etag").to_str().unwrap().starts_with('"'));
    assert!(res
        .header("cache-control")
        .to_str()
        .unwrap()
        .contains("public"));
}

#[tokio::test]
async fn hn_filters_dead_items() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v0/topstories.json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(vec![1u64, 2]))
        .mount(&server)
        .await;

    // Item 1: normal
    Mock::given(method("GET"))
        .and(path("/v0/item/1.json"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": 1,
                "title": "Live Story",
                "by": "user",
                "score": 10,
                "time": 1700000000
            })),
        )
        .mount(&server)
        .await;

    // Item 2: dead
    Mock::given(method("GET"))
        .and(path("/v0/item/2.json"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": 2,
                "dead": true
            })),
        )
        .mount(&server)
        .await;

    let config = test_config();
    let http = HttpClient::new_no_proxy(&config);
    let mut base_urls = HashMap::new();
    base_urls.insert("hackernews".to_string(), format!("{}/v0", server.uri()));
    let s = TestServer::new(build_app_with_overrides(&config, http, base_urls));

    let res: axum_test::TestResponse = s.get("/hackernews/top?format=json").await;
    res.assert_status_ok();

    let v: serde_json::Value = serde_json::from_str(&res.text()).unwrap();
    let items = v["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["title"], "Live Story");
}
