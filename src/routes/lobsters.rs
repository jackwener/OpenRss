use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use serde::Deserialize;

use crate::data::{Data, DataItem};
use crate::error::AppError;
use crate::registry::{AppState, RouteDefinition};

const LOBSTERS_BASE: &str = "https://lobste.rs";

#[derive(Deserialize)]
struct LobstersStory {
    short_id: String,
    title: String,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    description: Option<String>,
    comments_url: String,
    comment_count: i64,
    score: i64,
    created_at: String,
    submitter_user: LobstersUser,
    #[serde(default)]
    tags: Vec<String>,
}

#[derive(Deserialize)]
struct LobstersUser {
    username: String,
}

pub fn routes() -> Vec<RouteDefinition> {
    vec![RouteDefinition {
        path: "/lobsters/{category}",
        name: "lobsters",
        example: "/lobsters/hottest",
        handler: lobsters_handler,
    }]
}

fn category_to_endpoint(category: &str) -> Option<&'static str> {
    match category {
        "hottest" | "hot" => Some("hottest"),
        "newest" | "new" => Some("newest"),
        "active" => Some("active"),
        _ => None,
    }
}

fn category_title(category: &str) -> &'static str {
    match category {
        "hottest" | "hot" => "Lobsters - Hottest",
        "newest" | "new" => "Lobsters - Newest",
        "active" => "Lobsters - Active",
        _ => "Lobsters",
    }
}

fn lobsters_handler(
    state: Arc<AppState>,
    path: HashMap<String, String>,
    _query: HashMap<String, String>,
) -> Pin<Box<dyn Future<Output = Result<Data, AppError>> + Send>> {
    Box::pin(async move {
        let category = path
            .get("category")
            .ok_or_else(|| AppError::Internal("missing category param".into()))?;

        let endpoint = category_to_endpoint(category)
            .ok_or_else(|| AppError::RouteNotFound(format!("lobsters/{category}")))?;

        let base = state.base_url("lobsters", LOBSTERS_BASE);
        let limit = state.config.item_limit;

        let url = format!("{base}/{endpoint}.json");
        let stories: Vec<LobstersStory> = state.http.get_json(&url).await?;

        let items: Vec<DataItem> = stories
            .into_iter()
            .take(limit)
            .map(|s| map_lobsters_story(&s))
            .collect();

        let mut data = Data::new(category_title(category));
        data.link = Some("https://lobste.rs".into());
        data.description = Some(format!("{} via OpenRss", category_title(category)));
        data.language = Some("en".into());
        data.items = items;

        Ok(data)
    })
}

fn map_lobsters_story(story: &LobstersStory) -> DataItem {
    let mut item = DataItem::new(&story.title);

    // Link: prefer external URL, fall back to comments page
    item.link = Some(
        story
            .url
            .as_deref()
            .filter(|u| !u.is_empty())
            .unwrap_or(&story.comments_url)
            .to_string(),
    );

    // Description: user description (if any) + score/comments
    let mut desc_parts = Vec::new();
    if let Some(ref desc) = story.description {
        if !desc.is_empty() {
            desc_parts.push(desc.clone());
        }
    }
    desc_parts.push(format!(
        "<p>Score: {} | <a href=\"{}\">Comments: {}</a></p>",
        story.score, story.comments_url, story.comment_count
    ));
    item.description = Some(desc_parts.join("<br/><br/>"));

    item.guid = Some(format!("lobsters-{}", story.short_id));
    item.author = Some(story.submitter_user.username.clone());
    item.category = story.tags.clone();

    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&story.created_at) {
        item.pub_date = Some(dt.with_timezone(&chrono::Utc));
    }

    item
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn category_mapping() {
        assert_eq!(category_to_endpoint("hottest"), Some("hottest"));
        assert_eq!(category_to_endpoint("hot"), Some("hottest"));
        assert_eq!(category_to_endpoint("newest"), Some("newest"));
        assert_eq!(category_to_endpoint("new"), Some("newest"));
        assert_eq!(category_to_endpoint("active"), Some("active"));
        assert_eq!(category_to_endpoint("invalid"), None);
    }

    #[test]
    fn map_story_full() {
        let story = LobstersStory {
            short_id: "abc123".into(),
            title: "Rust is great".into(),
            url: Some("https://example.com/rust".into()),
            description: Some("An article about Rust".into()),
            comments_url: "https://lobste.rs/s/abc123".into(),
            comment_count: 15,
            score: 42,
            created_at: "2025-01-15T12:00:00.000-05:00".into(),
            submitter_user: LobstersUser {
                username: "rustfan".into(),
            },
            tags: vec!["rust".into(), "programming".into()],
        };
        let item = map_lobsters_story(&story);
        assert_eq!(item.title, "Rust is great");
        assert_eq!(item.link.as_deref(), Some("https://example.com/rust"));
        assert_eq!(item.author.as_deref(), Some("rustfan"));
        assert_eq!(item.guid.as_deref(), Some("lobsters-abc123"));
        assert_eq!(item.category, vec!["rust", "programming"]);
        assert!(item.pub_date.is_some());
        let desc = item.description.unwrap();
        assert!(desc.contains("An article about Rust"));
        assert!(desc.contains("Score: 42"));
        assert!(desc.contains("Comments: 15"));
    }

    #[test]
    fn map_story_no_url_falls_back_to_comments() {
        let story = LobstersStory {
            short_id: "xyz".into(),
            title: "Ask Lobsters".into(),
            url: None,
            description: None,
            comments_url: "https://lobste.rs/s/xyz".into(),
            comment_count: 5,
            score: 10,
            created_at: "2025-01-15T12:00:00.000-05:00".into(),
            submitter_user: LobstersUser {
                username: "user1".into(),
            },
            tags: vec![],
        };
        let item = map_lobsters_story(&story);
        assert_eq!(item.link.as_deref(), Some("https://lobste.rs/s/xyz"));
    }

    #[test]
    fn map_story_empty_url_falls_back() {
        let story = LobstersStory {
            short_id: "e".into(),
            title: "Empty URL".into(),
            url: Some("".into()),
            description: None,
            comments_url: "https://lobste.rs/s/e".into(),
            comment_count: 0,
            score: 1,
            created_at: "2025-01-15T12:00:00.000-05:00".into(),
            submitter_user: LobstersUser {
                username: "u".into(),
            },
            tags: vec![],
        };
        let item = map_lobsters_story(&story);
        assert_eq!(item.link.as_deref(), Some("https://lobste.rs/s/e"));
    }

    #[test]
    fn routes_has_one_entry() {
        let r = routes();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].path, "/lobsters/{category}");
    }
}
