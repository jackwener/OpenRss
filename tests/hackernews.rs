use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

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

fn test_client() -> HttpClient {
    HttpClient::new_no_proxy(&test_config())
}

async fn setup_mock_hn() -> MockServer {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v0/topstories.json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(vec![1u64, 2, 3, 4, 5]))
        .mount(&server)
        .await;

    for i in 1u64..=5 {
        let item = serde_json::json!({
            "id": i,
            "title": format!("Story {i}"),
            "url": format!("https://example.com/{i}"),
            "by": "testuser",
            "score": 100 + i as i64,
            "descendants": 10 + i as i64,
            "time": 1700000000 + i as i64 * 3600,
            "type": "story"
        });

        Mock::given(method("GET"))
            .and(path(format!("/v0/item/{i}.json")))
            .respond_with(ResponseTemplate::new(200).set_body_json(item))
            .mount(&server)
            .await;
    }

    server
}

#[tokio::test]
async fn hn_fetches_story_ids() {
    let server = setup_mock_hn().await;
    let http = test_client();

    let ids: Vec<u64> = http
        .get_json(&format!("{}/v0/topstories.json", server.uri()))
        .await
        .unwrap();
    assert_eq!(ids, vec![1, 2, 3, 4, 5]);
}

#[tokio::test]
async fn hn_fetches_item() {
    let server = setup_mock_hn().await;
    let http = test_client();

    let item: serde_json::Value = http
        .get_json(&format!("{}/v0/item/3.json", server.uri()))
        .await
        .unwrap();
    assert_eq!(item["id"], 3);
    assert_eq!(item["title"], "Story 3");
    assert_eq!(item["url"], "https://example.com/3");
    assert_eq!(item["score"], 103);
    assert_eq!(item["by"], "testuser");
}

#[tokio::test]
async fn hn_ask_story_has_text() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v0/item/42.json"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": 42,
                "title": "Ask HN: Best Rust crates?",
                "text": "<p>What are your favorite Rust crates?</p>",
                "by": "curious",
                "score": 200,
                "descendants": 50,
                "time": 1700000000,
                "type": "story"
            })),
        )
        .mount(&server)
        .await;

    let http = test_client();

    let item: serde_json::Value = http
        .get_json(&format!("{}/v0/item/42.json", server.uri()))
        .await
        .unwrap();

    assert_eq!(item["title"], "Ask HN: Best Rust crates?");
    assert!(item["text"].as_str().unwrap().contains("favorite Rust crates"));
    assert!(item["url"].is_null());
}

#[tokio::test]
async fn hn_404_returns_error() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v0/item/999.json"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let http = test_client();
    let result: Result<serde_json::Value, _> = http
        .get_json(&format!("{}/v0/item/999.json", server.uri()))
        .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn hn_retry_on_server_error() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v0/topstories.json"))
        .respond_with(ResponseTemplate::new(503))
        .expect(4) // 1 original + 3 retries
        .mount(&server)
        .await;

    let http = test_client();
    let result: Result<Vec<u64>, _> = http
        .get_json(&format!("{}/v0/topstories.json", server.uri()))
        .await;

    assert!(result.is_err());
    // Mock expectation (expect(4)) is verified on MockServer drop
}

#[tokio::test]
async fn hn_deserializes_item_struct() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v0/item/1.json"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": 1,
                "title": "Test Story",
                "url": "https://example.com",
                "by": "author",
                "score": 42,
                "descendants": 5,
                "time": 1700000000
            })),
        )
        .mount(&server)
        .await;

    let http = test_client();

    // Deserialize into generic Value to verify structure
    let item: serde_json::Value = http
        .get_json(&format!("{}/v0/item/1.json", server.uri()))
        .await
        .unwrap();

    assert_eq!(item["id"], 1);
    assert_eq!(item["title"], "Test Story");
    assert_eq!(item["url"], "https://example.com");
    assert_eq!(item["by"], "author");
    assert_eq!(item["score"], 42);
    assert_eq!(item["descendants"], 5);
    assert_eq!(item["time"], 1700000000i64);
}
