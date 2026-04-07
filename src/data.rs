use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Core feed data returned by every route handler.
///
/// This is the universal intermediate representation — every adapter produces a `Data`,
/// and every output format (RSS, Atom, JSON Feed) consumes one.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Data {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    pub items: Vec<DataItem>,

    // Feed metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated: Option<DateTime<Utc>>,

    // Podcast extensions (iTunes)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub itunes_author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub itunes_category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub itunes_explicit: Option<bool>,
}

/// A single item/entry in the feed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataItem {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pub_date: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub category: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guid: Option<String>,

    // Enclosure (podcast/media attachments)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enclosure_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enclosure_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enclosure_length: Option<u64>,
}

impl Data {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            link: None,
            description: None,
            language: None,
            image: None,
            items: Vec::new(),
            ttl: None,
            updated: None,
            itunes_author: None,
            itunes_category: None,
            itunes_explicit: None,
        }
    }
}

impl DataItem {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            description: None,
            link: None,
            pub_date: None,
            author: None,
            category: Vec::new(),
            guid: None,
            enclosure_url: None,
            enclosure_type: None,
            enclosure_length: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_new_sets_title() {
        let data = Data::new("Test Feed");
        assert_eq!(data.title, "Test Feed");
        assert!(data.items.is_empty());
    }

    #[test]
    fn data_item_new_sets_title() {
        let item = DataItem::new("Test Item");
        assert_eq!(item.title, "Test Item");
        assert!(item.link.is_none());
        assert!(item.category.is_empty());
    }

    #[test]
    fn data_serializes_to_json() {
        let mut data = Data::new("Test");
        data.link = Some("https://example.com".into());
        data.items.push(DataItem::new("Item 1"));

        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("\"title\":\"Test\""));
        assert!(json.contains("\"link\":\"https://example.com\""));
    }

    #[test]
    fn data_skips_none_fields_in_json() {
        let data = Data::new("Minimal");
        let json = serde_json::to_string(&data).unwrap();
        assert!(!json.contains("\"link\""));
        assert!(!json.contains("\"description\""));
        assert!(!json.contains("\"language\""));
    }
}
