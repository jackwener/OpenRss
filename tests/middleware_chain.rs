use axum_test::TestServer;
use openrss::app::build_app;
use openrss::config::Config;

fn test_config() -> Config {
    Config {
        port: 0,
        cache_expire: 300,
        cache_type: openrss::config::CacheType::Memory,
        redis_url: None,
        access_key: None,
        request_timeout: 30,
        item_limit: 50,
    }
}

fn test_config_with_key(key: &str) -> Config {
    Config {
        access_key: Some(key.to_string()),
        ..test_config()
    }
}

fn server(config: &Config) -> TestServer {
    TestServer::new(build_app(config))
}

#[tokio::test]
async fn health_check() {
    let s = server(&test_config());
    let res: axum_test::TestResponse = s.get("/healthz").await;
    res.assert_status_ok();
    res.assert_text("ok");
}

#[tokio::test]
async fn index_page() {
    let s = server(&test_config());
    let res: axum_test::TestResponse = s.get("/").await;
    res.assert_status_ok();
    assert!(res.text().contains("OpenRss"));
}

#[tokio::test]
async fn test_route_returns_rss_by_default() {
    let s = server(&test_config());
    let res: axum_test::TestResponse = s.get("/test/example").await;
    res.assert_status_ok();

    let body = res.text();
    assert!(body.contains("<rss version=\"2.0\""));
    assert!(body.contains("<title>Test Feed</title>"));
    assert!(body.contains("<generator>OpenRss</generator>"));
    assert!(body.contains("Test Item"));
}

#[tokio::test]
async fn test_route_returns_atom() {
    let s = server(&test_config());
    let res: axum_test::TestResponse = s.get("/test/example?format=atom").await;
    res.assert_status_ok();

    let content_type = res.header("content-type").to_str().unwrap().to_string();
    assert!(content_type.contains("atom+xml"));

    let body = res.text();
    assert!(body.contains("<feed xmlns=\"http://www.w3.org/2005/Atom\""));
    assert!(body.contains("<title>Test Feed</title>"));
}

#[tokio::test]
async fn test_route_returns_json_feed() {
    let s = server(&test_config());
    let res: axum_test::TestResponse = s.get("/test/example?format=json").await;
    res.assert_status_ok();

    let content_type = res.header("content-type").to_str().unwrap().to_string();
    assert!(content_type.contains("feed+json"));

    let body = res.text();
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["title"], "Test Feed");
    assert_eq!(v["items"].as_array().unwrap().len(), 5);
}

#[tokio::test]
async fn cors_headers() {
    let s = server(&test_config());
    let res: axum_test::TestResponse = s.get("/test/example").await;

    assert_eq!(
        res.header("access-control-allow-origin").to_str().unwrap(),
        "*"
    );
    assert_eq!(
        res.header("access-control-allow-methods").to_str().unwrap(),
        "GET"
    );
}

#[tokio::test]
async fn cache_control_header() {
    let s = server(&test_config());
    let res: axum_test::TestResponse = s.get("/test/example").await;

    let cc = res.header("cache-control").to_str().unwrap().to_string();
    assert!(cc.contains("public"));
    assert!(cc.contains("max-age=300"));
}

#[tokio::test]
async fn etag_is_present() {
    let s = server(&test_config());
    let res: axum_test::TestResponse = s.get("/test/example").await;

    let etag = res.header("etag").to_str().unwrap().to_string();
    assert!(etag.starts_with('"'));
    assert!(etag.ends_with('"'));
}

#[tokio::test]
async fn etag_304_flow() {
    let s = server(&test_config());

    // First request: get ETag
    let res1: axum_test::TestResponse = s.get("/test/example").await;
    let etag = res1.header("etag").to_str().unwrap().to_string();

    // Second request with If-None-Match: should get 304
    let res2: axum_test::TestResponse = s
        .get("/test/example")
        .add_header(axum::http::header::IF_NONE_MATCH, axum::http::HeaderValue::from_str(&etag).unwrap())
        .await;
    res2.assert_status(axum::http::StatusCode::NOT_MODIFIED);
}

#[tokio::test]
async fn parameter_limit() {
    let s = server(&test_config());
    let res: axum_test::TestResponse = s.get("/test/example?format=json&limit=2").await;
    res.assert_status_ok();

    let v: serde_json::Value = serde_json::from_str(&res.text()).unwrap();
    assert_eq!(v["items"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn parameter_filter_title() {
    let s = server(&test_config());
    let res: axum_test::TestResponse = s
        .get("/test/example?format=json&filter_title=Item%201")
        .await;
    res.assert_status_ok();

    let v: serde_json::Value = serde_json::from_str(&res.text()).unwrap();
    let items = v["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["title"], "Test Item 1");
}

#[tokio::test]
async fn cache_hit_on_second_request() {
    let s = server(&test_config());

    // First request: MISS
    let res1: axum_test::TestResponse = s.get("/test/example").await;
    assert_eq!(res1.header("x-openrss-cache").to_str().unwrap(), "MISS");

    // Second request: HIT
    let res2: axum_test::TestResponse = s.get("/test/example").await;
    assert_eq!(res2.header("x-openrss-cache").to_str().unwrap(), "HIT");
}

#[tokio::test]
async fn access_control_rejects_without_key() {
    let s = server(&test_config_with_key("secret123"));
    let res: axum_test::TestResponse = s.get("/test/example").expect_failure().await;
    res.assert_status(axum::http::StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn access_control_accepts_valid_key() {
    let s = server(&test_config_with_key("secret123"));
    let res: axum_test::TestResponse = s.get("/test/example?key=secret123").await;
    res.assert_status_ok();
}

#[tokio::test]
async fn access_control_accepts_valid_code() {
    let key = "secret123";
    let path = "/test/example";
    let code = openrss::middleware::access_control::compute_access_code(path, key);

    let s = server(&test_config_with_key(key));
    let res: axum_test::TestResponse = s.get(&format!("/test/example?code={code}")).await;
    res.assert_status_ok();
}

#[tokio::test]
async fn access_control_passes_whitelist() {
    let s = server(&test_config_with_key("secret123"));

    let res: axum_test::TestResponse = s.get("/healthz").await;
    res.assert_status_ok();

    let res: axum_test::TestResponse = s.get("/").await;
    res.assert_status_ok();
}

#[tokio::test]
async fn items_sorted_newest_first() {
    let s = server(&test_config());
    let res: axum_test::TestResponse = s.get("/test/example?format=json").await;
    res.assert_status_ok();

    let v: serde_json::Value = serde_json::from_str(&res.text()).unwrap();
    let items = v["items"].as_array().unwrap();

    // Test items are Jan 11-15, should be sorted newest (Item 5) first
    assert!(items[0]["title"].as_str().unwrap().contains("5"));
    assert!(items[4]["title"].as_str().unwrap().contains("1"));
}
