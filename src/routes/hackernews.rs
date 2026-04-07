use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use futures::stream::{self, StreamExt};
use serde::Deserialize;

use crate::data::{Data, DataItem};
use crate::error::AppError;
use crate::registry::{AppState, RouteDefinition};

const HN_API_BASE: &str = "https://hacker-news.firebaseio.com/v0";
const CONCURRENCY_LIMIT: usize = 15;

#[derive(Deserialize)]
struct HnItem {
    id: u64,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    by: Option<String>,
    #[serde(default)]
    score: Option<i64>,
    #[serde(default)]
    descendants: Option<i64>,
    #[serde(default)]
    time: Option<i64>,
    #[serde(default)]
    dead: Option<bool>,
    #[serde(default)]
    deleted: Option<bool>,
}

pub fn routes() -> Vec<RouteDefinition> {
    vec![RouteDefinition {
        path: "/hackernews/{category}",
        name: "hackernews",
        example: "/hackernews/top",
        handler: hn_handler,
    }]
}

/// Map category slug to HN API endpoint name.
fn category_to_endpoint(category: &str) -> Option<&'static str> {
    match category {
        "top" => Some("topstories"),
        "new" => Some("newstories"),
        "best" => Some("beststories"),
        "ask" => Some("askstories"),
        "show" => Some("showstories"),
        "jobs" => Some("jobstories"),
        _ => None,
    }
}

/// Map category slug to feed title.
fn category_title(category: &str) -> &'static str {
    match category {
        "top" => "Hacker News - Top Stories",
        "new" => "Hacker News - New Stories",
        "best" => "Hacker News - Best Stories",
        "ask" => "Hacker News - Ask HN",
        "show" => "Hacker News - Show HN",
        "jobs" => "Hacker News - Jobs",
        _ => "Hacker News",
    }
}

fn hn_handler(
    state: Arc<AppState>,
    path: HashMap<String, String>,
    _query: HashMap<String, String>,
) -> Pin<Box<dyn Future<Output = Result<Data, AppError>> + Send>> {
    Box::pin(async move {
        let category = path
            .get("category")
            .ok_or_else(|| AppError::Internal("missing category param".into()))?;

        let endpoint = category_to_endpoint(category)
            .ok_or_else(|| AppError::RouteNotFound(format!("hackernews/{category}")))?;

        let limit = state.config.item_limit;

        // Fetch story IDs
        let ids_url = format!("{HN_API_BASE}/{endpoint}.json");
        let ids: Vec<u64> = state.http.get_json(&ids_url).await?;

        // Take only `limit` IDs
        let ids: Vec<u64> = ids.into_iter().take(limit).collect();

        // Fetch items concurrently
        let items: Vec<DataItem> = stream::iter(ids)
            .map(|id| {
                let http = &state.http;
                async move {
                    let url = format!("{HN_API_BASE}/item/{id}.json");
                    let hn_item: Result<HnItem, _> = http.get_json(&url).await;
                    hn_item.ok().and_then(|item| map_hn_item(&item))
                }
            })
            .buffer_unordered(CONCURRENCY_LIMIT)
            .filter_map(|opt| async { opt })
            .collect()
            .await;

        let mut data = Data::new(category_title(category));
        data.link = Some("https://news.ycombinator.com".into());
        data.description = Some(format!("{} via OpenRss", category_title(category)));
        data.language = Some("en".into());
        data.items = items;

        Ok(data)
    })
}

/// Convert an HN API item to a DataItem. Returns None for dead/deleted items.
fn map_hn_item(item: &HnItem) -> Option<DataItem> {
    if item.dead.unwrap_or(false) || item.deleted.unwrap_or(false) {
        return None;
    }

    let title = item.title.as_deref()?;
    let mut data_item = DataItem::new(title);

    // Link: prefer external URL, fall back to HN item page
    let hn_url = format!("https://news.ycombinator.com/item?id={}", item.id);
    data_item.link = Some(item.url.clone().unwrap_or_else(|| hn_url.clone()));

    // Description: text (if any) + score/comments info
    let mut desc_parts = Vec::new();
    if let Some(ref text) = item.text {
        desc_parts.push(text.clone());
    }
    if let Some(score) = item.score {
        let comments = item.descendants.unwrap_or(0);
        desc_parts.push(format!(
            "<p>Score: {score} | <a href=\"{hn_url}\">Comments: {comments}</a></p>"
        ));
    }
    if !desc_parts.is_empty() {
        data_item.description = Some(desc_parts.join("<br/><br/>"));
    }

    data_item.guid = Some(format!("hn-{}", item.id));
    data_item.author = item.by.clone();

    if let Some(ts) = item.time {
        data_item.pub_date = chrono::DateTime::from_timestamp(ts, 0);
    }

    Some(data_item)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn category_mapping() {
        assert_eq!(category_to_endpoint("top"), Some("topstories"));
        assert_eq!(category_to_endpoint("new"), Some("newstories"));
        assert_eq!(category_to_endpoint("best"), Some("beststories"));
        assert_eq!(category_to_endpoint("ask"), Some("askstories"));
        assert_eq!(category_to_endpoint("show"), Some("showstories"));
        assert_eq!(category_to_endpoint("jobs"), Some("jobstories"));
        assert_eq!(category_to_endpoint("invalid"), None);
    }

    #[test]
    fn map_hn_item_full() {
        let item = HnItem {
            id: 12345,
            title: Some("Show HN: My Project".into()),
            url: Some("https://example.com".into()),
            text: Some("Check out my project".into()),
            by: Some("testuser".into()),
            score: Some(42),
            descendants: Some(10),
            time: Some(1700000000),
            dead: None,
            deleted: None,
        };
        let result = map_hn_item(&item).unwrap();
        assert_eq!(result.title, "Show HN: My Project");
        assert_eq!(result.link.as_deref(), Some("https://example.com"));
        assert_eq!(result.author.as_deref(), Some("testuser"));
        assert_eq!(result.guid.as_deref(), Some("hn-12345"));
        assert!(result.pub_date.is_some());
        let desc = result.description.unwrap();
        assert!(desc.contains("Check out my project"));
        assert!(desc.contains("Score: 42"));
        assert!(desc.contains("Comments: 10"));
        // Text and score separated by <br/><br/>, not \n
        assert!(desc.contains("<br/><br/>"));
    }

    #[test]
    fn map_hn_item_no_url_falls_back_to_hn() {
        let item = HnItem {
            id: 99,
            title: Some("Ask HN: Question?".into()),
            url: None,
            text: None,
            by: None,
            score: None,
            descendants: None,
            time: None,
            dead: None,
            deleted: None,
        };
        let result = map_hn_item(&item).unwrap();
        assert_eq!(
            result.link.as_deref(),
            Some("https://news.ycombinator.com/item?id=99")
        );
    }

    #[test]
    fn map_hn_item_no_title_returns_none() {
        let item = HnItem {
            id: 1,
            title: None,
            url: None,
            text: None,
            by: None,
            score: None,
            descendants: None,
            time: None,
            dead: None,
            deleted: None,
        };
        assert!(map_hn_item(&item).is_none());
    }

    #[test]
    fn map_hn_item_dead_returns_none() {
        let item = HnItem {
            id: 1,
            title: Some("Dead Story".into()),
            url: None,
            text: None,
            by: None,
            score: None,
            descendants: None,
            time: None,
            dead: Some(true),
            deleted: None,
        };
        assert!(map_hn_item(&item).is_none());
    }

    #[test]
    fn map_hn_item_deleted_returns_none() {
        let item = HnItem {
            id: 1,
            title: Some("Deleted Story".into()),
            url: None,
            text: None,
            by: None,
            score: None,
            descendants: None,
            time: None,
            dead: None,
            deleted: Some(true),
        };
        assert!(map_hn_item(&item).is_none());
    }

    #[test]
    fn routes_has_one_entry() {
        let r = routes();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].path, "/hackernews/{category}");
    }
}
