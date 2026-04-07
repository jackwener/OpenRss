use serde::Serialize;

use crate::data::Data;
use crate::error::AppError;

/// JSON Feed 1.1 structure.
/// Spec: https://www.jsonfeed.org/version/1.1/
#[derive(Debug, Serialize)]
struct JsonFeed {
    version: &'static str,
    title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    home_page_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    feed_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    icon: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    language: Option<String>,
    items: Vec<JsonFeedItem>,
}

#[derive(Debug, Serialize)]
struct JsonFeedItem {
    id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<String>,
    title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content_html: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    date_published: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    authors: Option<Vec<JsonFeedAuthor>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    attachments: Option<Vec<JsonFeedAttachment>>,
}

#[derive(Debug, Serialize)]
struct JsonFeedAuthor {
    name: String,
}

#[derive(Debug, Serialize)]
struct JsonFeedAttachment {
    url: String,
    mime_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    size_in_bytes: Option<u64>,
}

/// Render `Data` as a JSON Feed 1.1 document.
pub fn render(data: &Data) -> Result<String, AppError> {
    let feed = JsonFeed {
        version: "https://jsonfeed.org/version/1.1",
        title: data.title.clone(),
        home_page_url: data.link.clone(),
        feed_url: data.link.clone(),
        description: data.description.clone(),
        icon: data.image.clone(),
        language: data.language.clone(),
        items: data
            .items
            .iter()
            .map(|item| {
                let id = item
                    .guid
                    .clone()
                    .or_else(|| item.link.clone())
                    .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

                let authors = item
                    .author
                    .as_ref()
                    .map(|a| vec![JsonFeedAuthor { name: a.clone() }]);

                let tags = if item.category.is_empty() {
                    None
                } else {
                    Some(item.category.clone())
                };

                let attachments = item.enclosure_url.as_ref().map(|url| {
                    vec![JsonFeedAttachment {
                        url: url.clone(),
                        mime_type: item
                            .enclosure_type
                            .clone()
                            .unwrap_or_else(|| "application/octet-stream".into()),
                        size_in_bytes: item.enclosure_length,
                    }]
                });

                JsonFeedItem {
                    id,
                    url: item.link.clone(),
                    title: item.title.clone(),
                    content_html: item.description.clone(),
                    date_published: item.pub_date.map(|d| d.to_rfc3339()),
                    summary: None,
                    authors,
                    tags,
                    attachments,
                }
            })
            .collect(),
    };

    serde_json::to_string_pretty(&feed).map_err(|e| AppError::Render(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{Data, DataItem};
    use chrono::{TimeZone, Utc};

    fn sample_data() -> Data {
        let mut data = Data::new("Test Feed");
        data.link = Some("https://example.com".into());
        data.description = Some("A test feed".into());
        data.language = Some("en".into());

        let mut item = DataItem::new("Item 1");
        item.link = Some("https://example.com/1".into());
        item.description = Some("<p>Hello</p>".into());
        item.pub_date = Some(Utc.with_ymd_and_hms(2025, 1, 15, 12, 0, 0).unwrap());
        item.author = Some("Alice".into());
        item.category = vec!["tech".into()];
        item.guid = Some("guid-001".into());
        data.items.push(item);

        data
    }

    #[test]
    fn json_feed_valid_structure() {
        let json_str = render(&sample_data()).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(v["version"], "https://jsonfeed.org/version/1.1");
        assert_eq!(v["title"], "Test Feed");
        assert!(v["items"].is_array());
    }

    #[test]
    fn json_feed_includes_item_fields() {
        let json_str = render(&sample_data()).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        let item = &v["items"][0];
        assert_eq!(item["id"], "guid-001");
        assert_eq!(item["title"], "Item 1");
        assert_eq!(item["url"], "https://example.com/1");
        assert_eq!(item["content_html"], "<p>Hello</p>");
    }

    #[test]
    fn json_feed_includes_authors() {
        let json_str = render(&sample_data()).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        let authors = &v["items"][0]["authors"];
        assert!(authors.is_array());
        assert_eq!(authors[0]["name"], "Alice");
    }

    #[test]
    fn json_feed_includes_tags() {
        let json_str = render(&sample_data()).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        let tags = &v["items"][0]["tags"];
        assert!(tags.is_array());
        assert_eq!(tags[0], "tech");
    }

    #[test]
    fn json_feed_includes_attachments() {
        let mut data = Data::new("Podcast");
        let mut item = DataItem::new("Episode");
        item.enclosure_url = Some("https://example.com/ep.mp3".into());
        item.enclosure_type = Some("audio/mpeg".into());
        item.enclosure_length = Some(99999);
        data.items.push(item);

        let json_str = render(&data).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        let att = &v["items"][0]["attachments"][0];
        assert_eq!(att["url"], "https://example.com/ep.mp3");
        assert_eq!(att["mime_type"], "audio/mpeg");
        assert_eq!(att["size_in_bytes"], 99999);
    }

    #[test]
    fn json_feed_handles_empty_items() {
        let data = Data::new("Empty");
        let json_str = render(&data).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(v["items"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn json_feed_date_is_rfc3339() {
        let json_str = render(&sample_data()).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        let date = v["items"][0]["date_published"].as_str().unwrap();
        // Should parse as RFC 3339
        chrono::DateTime::parse_from_rfc3339(date).unwrap();
    }

    #[test]
    fn json_feed_skips_none_fields() {
        let data = Data::new("Minimal");
        let json_str = render(&data).unwrap();
        assert!(!json_str.contains("\"description\""));
        assert!(!json_str.contains("\"icon\""));
    }
}
