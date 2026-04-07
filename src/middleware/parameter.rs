use chrono::Utc;
use regex::Regex;

use crate::data::Data;
use super::sanitize;

/// Query parameters that control item filtering, sorting, and limiting.
#[derive(Debug, Default)]
pub struct FilterParams {
    pub filter: Option<String>,
    pub filterout: Option<String>,
    pub filter_title: Option<String>,
    pub filterout_title: Option<String>,
    pub filter_description: Option<String>,
    pub filterout_description: Option<String>,
    pub filter_case_sensitive: Option<bool>,
    pub limit: Option<usize>,
    pub filter_time: Option<i64>,
    pub brief: Option<usize>,
}

impl FilterParams {
    /// Parse from URL query string.
    pub fn from_query(query: Option<&str>) -> Self {
        let query = match query {
            Some(q) => q,
            None => return Self::default(),
        };

        let mut params = Self::default();

        for pair in query.split('&') {
            let mut parts = pair.splitn(2, '=');
            let key = parts.next().unwrap_or("");
            let val = parts.next().unwrap_or("");
            let val_decoded = urlencoding::decode(val).unwrap_or_default().to_string();

            match key {
                "filter" => params.filter = Some(val_decoded),
                "filterout" => params.filterout = Some(val_decoded),
                "filter_title" => params.filter_title = Some(val_decoded),
                "filterout_title" => params.filterout_title = Some(val_decoded),
                "filter_description" => params.filter_description = Some(val_decoded),
                "filterout_description" => params.filterout_description = Some(val_decoded),
                "filter_case_sensitive" => {
                    params.filter_case_sensitive = Some(val_decoded != "false");
                }
                "limit" => params.limit = val_decoded.parse().ok(),
                "filter_time" => params.filter_time = val_decoded.parse().ok(),
                "brief" => params.brief = val_decoded.parse().ok(),
                _ => {}
            }
        }

        params
    }
}

/// Apply filter/sort/limit parameters to Data items.
pub fn apply_filters(data: &mut Data, params: &FilterParams) {
    let case_sensitive = params.filter_case_sensitive.unwrap_or(true);

    // filter_time: only keep items within N seconds
    if let Some(seconds) = params.filter_time {
        let cutoff = Utc::now() - chrono::Duration::seconds(seconds);
        data.items.retain(|item| {
            item.pub_date.map_or(true, |d| d >= cutoff)
        });
    }

    // Unified filter/filterout take priority over individual field filters (RSSHub behavior).
    if params.filter.is_some() || params.filterout.is_some() {
        // filter: include items matching ANY of title/description/author/category
        if let Some(ref pattern) = params.filter {
            if let Some(re) = build_regex(pattern, case_sensitive) {
                data.items.retain(|item| {
                    re.is_match(&item.title)
                        || item.description.as_ref().map_or(false, |d| re.is_match(d))
                        || item.author.as_ref().map_or(false, |a| re.is_match(a))
                        || item.category.iter().any(|c| re.is_match(c))
                });
            }
        }

        // filterout: exclude items matching ANY of title/description/author/category
        if let Some(ref pattern) = params.filterout {
            if let Some(re) = build_regex(pattern, case_sensitive) {
                data.items.retain(|item| {
                    !(re.is_match(&item.title)
                        || item.description.as_ref().map_or(false, |d| re.is_match(d))
                        || item.author.as_ref().map_or(false, |a| re.is_match(a))
                        || item.category.iter().any(|c| re.is_match(c)))
                });
            }
        }
    } else {
        // Individual field filters (only when unified filter is not set)

        // filter_title: include items matching title regex
        if let Some(ref pattern) = params.filter_title {
            if let Some(re) = build_regex(pattern, case_sensitive) {
                data.items.retain(|item| re.is_match(&item.title));
            }
        }

        // filterout_title: exclude items matching title regex
        if let Some(ref pattern) = params.filterout_title {
            if let Some(re) = build_regex(pattern, case_sensitive) {
                data.items.retain(|item| !re.is_match(&item.title));
            }
        }

        // filter_description: include items matching description regex
        if let Some(ref pattern) = params.filter_description {
            if let Some(re) = build_regex(pattern, case_sensitive) {
                data.items
                    .retain(|item| item.description.as_ref().map_or(false, |d| re.is_match(d)));
            }
        }

        // filterout_description: exclude items matching description regex
        if let Some(ref pattern) = params.filterout_description {
            if let Some(re) = build_regex(pattern, case_sensitive) {
                data.items
                    .retain(|item| item.description.as_ref().map_or(true, |d| !re.is_match(d)));
            }
        }
    }

    // HTML sanitization — always applied to descriptions
    for item in &mut data.items {
        if let Some(ref mut desc) = item.description {
            *desc = sanitize::sanitize_html(desc);
        }
    }

    // brief: truncate description to N chars (min 100)
    if let Some(n) = params.brief {
        if n >= 100 {
            for item in &mut data.items {
                if let Some(ref desc) = item.description {
                    item.description = Some(sanitize::brief(desc, n));
                }
            }
        }
    }

    // Sort by pub_date descending (newest first)
    data.items.sort_by(|a, b| {
        let da = a.pub_date.unwrap_or_default();
        let db = b.pub_date.unwrap_or_default();
        db.cmp(&da)
    });

    // limit
    if let Some(limit) = params.limit {
        data.items.truncate(limit);
    }
}

/// Build a regex, returning None if the pattern is invalid (don't crash on bad user input).
fn build_regex(pattern: &str, case_sensitive: bool) -> Option<Regex> {
    let builder = regex::RegexBuilder::new(pattern)
        .case_insensitive(!case_sensitive)
        .size_limit(1 << 20) // 1MB compiled size limit (safety)
        .build();
    builder.ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{Data, DataItem};
    use chrono::{TimeZone, Utc};

    fn sample_data() -> Data {
        let mut data = Data::new("Test");

        let mut i1 = DataItem::new("Rust Programming");
        i1.description = Some("Learn Rust basics".into());
        i1.author = Some("Alice".into());
        i1.category = vec!["tech".into(), "rust".into()];
        i1.pub_date = Some(Utc.with_ymd_and_hms(2025, 1, 15, 12, 0, 0).unwrap());

        let mut i2 = DataItem::new("Python Tutorial");
        i2.description = Some("Python for beginners".into());
        i2.author = Some("Bob".into());
        i2.category = vec!["tech".into(), "python".into()];
        i2.pub_date = Some(Utc.with_ymd_and_hms(2025, 1, 16, 12, 0, 0).unwrap());

        let mut i3 = DataItem::new("Cooking Tips");
        i3.description = Some("How to make pasta".into());
        i3.author = Some("Charlie".into());
        i3.category = vec!["food".into()];
        i3.pub_date = Some(Utc.with_ymd_and_hms(2025, 1, 14, 12, 0, 0).unwrap());

        data.items = vec![i3, i1, i2]; // Intentionally unsorted
        data
    }

    #[test]
    fn filter_by_title_regex() {
        let mut data = sample_data();
        let params = FilterParams {
            filter_title: Some("Rust".into()),
            ..Default::default()
        };
        apply_filters(&mut data, &params);
        assert_eq!(data.items.len(), 1);
        assert_eq!(data.items[0].title, "Rust Programming");
    }

    #[test]
    fn filter_out_by_title() {
        let mut data = sample_data();
        let params = FilterParams {
            filterout_title: Some("Cooking".into()),
            ..Default::default()
        };
        apply_filters(&mut data, &params);
        assert_eq!(data.items.len(), 2);
        assert!(data.items.iter().all(|i| i.title != "Cooking Tips"));
    }

    #[test]
    fn filter_by_description() {
        let mut data = sample_data();
        let params = FilterParams {
            filter_description: Some("pasta".into()),
            ..Default::default()
        };
        apply_filters(&mut data, &params);
        assert_eq!(data.items.len(), 1);
        assert_eq!(data.items[0].title, "Cooking Tips");
    }

    #[test]
    fn filter_out_by_description() {
        let mut data = sample_data();
        let params = FilterParams {
            filterout_description: Some("beginners".into()),
            ..Default::default()
        };
        apply_filters(&mut data, &params);
        assert_eq!(data.items.len(), 2);
    }

    #[test]
    fn filter_unified_matches_any_field() {
        let mut data = sample_data();
        // "Alice" is an author, should match
        let params = FilterParams {
            filter: Some("Alice".into()),
            ..Default::default()
        };
        apply_filters(&mut data, &params);
        assert_eq!(data.items.len(), 1);
        assert_eq!(data.items[0].title, "Rust Programming");
    }

    #[test]
    fn filter_unified_matches_category() {
        let mut data = sample_data();
        let params = FilterParams {
            filter: Some("food".into()),
            ..Default::default()
        };
        apply_filters(&mut data, &params);
        assert_eq!(data.items.len(), 1);
        assert_eq!(data.items[0].title, "Cooking Tips");
    }

    #[test]
    fn filterout_unified() {
        let mut data = sample_data();
        let params = FilterParams {
            filterout: Some("tech".into()),
            ..Default::default()
        };
        apply_filters(&mut data, &params);
        assert_eq!(data.items.len(), 1);
        assert_eq!(data.items[0].title, "Cooking Tips");
    }

    #[test]
    fn limit_items() {
        let mut data = sample_data();
        let params = FilterParams {
            limit: Some(2),
            ..Default::default()
        };
        apply_filters(&mut data, &params);
        assert_eq!(data.items.len(), 2);
    }

    #[test]
    fn sort_by_pubdate_desc() {
        let mut data = sample_data();
        let params = FilterParams::default();
        apply_filters(&mut data, &params);
        // Should be sorted newest first
        assert_eq!(data.items[0].title, "Python Tutorial"); // Jan 16
        assert_eq!(data.items[1].title, "Rust Programming"); // Jan 15
        assert_eq!(data.items[2].title, "Cooking Tips"); // Jan 14
    }

    #[test]
    fn filter_case_insensitive() {
        let mut data = sample_data();
        let params = FilterParams {
            filter_title: Some("rust".into()),
            filter_case_sensitive: Some(false),
            ..Default::default()
        };
        apply_filters(&mut data, &params);
        assert_eq!(data.items.len(), 1);
        assert_eq!(data.items[0].title, "Rust Programming");
    }

    #[test]
    fn filter_case_sensitive() {
        let mut data = sample_data();
        let params = FilterParams {
            filter_title: Some("rust".into()),
            filter_case_sensitive: Some(true),
            ..Default::default()
        };
        apply_filters(&mut data, &params);
        assert_eq!(data.items.len(), 0); // "Rust" != "rust"
    }

    #[test]
    fn regex_filter_is_safe() {
        // Malicious regex should not crash — build_regex returns None
        let mut data = sample_data();
        let params = FilterParams {
            filter_title: Some("((((((".into()),
            ..Default::default()
        };
        apply_filters(&mut data, &params);
        // Invalid regex is silently ignored — all items pass
        assert_eq!(data.items.len(), 3);
    }

    #[test]
    fn filter_time() {
        let mut data = Data::new("Test");

        let mut recent = DataItem::new("Recent");
        recent.pub_date = Some(Utc::now() - chrono::Duration::seconds(10));
        data.items.push(recent);

        let mut old = DataItem::new("Old");
        old.pub_date = Some(Utc::now() - chrono::Duration::seconds(3600));
        data.items.push(old);

        let params = FilterParams {
            filter_time: Some(60), // Keep only items from last 60 seconds
            ..Default::default()
        };
        apply_filters(&mut data, &params);
        assert_eq!(data.items.len(), 1);
        assert_eq!(data.items[0].title, "Recent");
    }

    #[test]
    fn from_query_parses_all_params() {
        let params = FilterParams::from_query(Some(
            "filter=test&filterout=spam&filter_title=rust&limit=10&filter_time=3600&filter_case_sensitive=false&brief=200",
        ));
        assert_eq!(params.filter.as_deref(), Some("test"));
        assert_eq!(params.filterout.as_deref(), Some("spam"));
        assert_eq!(params.filter_title.as_deref(), Some("rust"));
        assert_eq!(params.limit, Some(10));
        assert_eq!(params.filter_time, Some(3600));
        assert_eq!(params.filter_case_sensitive, Some(false));
        assert_eq!(params.brief, Some(200));
    }

    #[test]
    fn from_query_handles_none() {
        let params = FilterParams::from_query(None);
        assert!(params.filter.is_none());
        assert!(params.limit.is_none());
    }

    #[test]
    fn brief_truncates_description() {
        let mut data = Data::new("Test");
        let mut item = DataItem::new("Item");
        item.description = Some("<p>This is a very long description that contains lots of text and should be truncated by the brief parameter</p>".into());
        data.items.push(item);

        let params = FilterParams {
            brief: Some(100),
            ..Default::default()
        };
        apply_filters(&mut data, &params);
        let desc = data.items[0].description.as_ref().unwrap();
        assert!(desc.ends_with("..."));
        // Plain text (tags stripped) + "..." should be around 103 chars
        assert!(desc.len() <= 110);
    }

    #[test]
    fn brief_ignored_when_under_100() {
        let mut data = Data::new("Test");
        let mut item = DataItem::new("Item");
        item.description = Some("<p>Some description here</p>".into());
        data.items.push(item);

        let params = FilterParams {
            brief: Some(50), // Under 100, should be ignored
            ..Default::default()
        };
        apply_filters(&mut data, &params);
        let desc = data.items[0].description.as_ref().unwrap();
        // HTML sanitization still applies but brief is skipped
        assert!(desc.contains("Some description here"));
    }

    #[test]
    fn sanitizes_script_in_description() {
        let mut data = Data::new("Test");
        let mut item = DataItem::new("Item");
        item.description = Some(r#"<p>Safe</p><script>alert("xss")</script>"#.into());
        data.items.push(item);

        let params = FilterParams::default();
        apply_filters(&mut data, &params);
        let desc = data.items[0].description.as_ref().unwrap();
        assert!(!desc.contains("script"));
        assert!(!desc.contains("alert"));
        assert!(desc.contains("<p>Safe</p>"));
    }

    #[test]
    fn sanitizes_event_handlers() {
        let mut data = Data::new("Test");
        let mut item = DataItem::new("Item");
        item.description = Some(r#"<img src="a.jpg" onerror="alert(1)">"#.into());
        data.items.push(item);

        let params = FilterParams::default();
        apply_filters(&mut data, &params);
        let desc = data.items[0].description.as_ref().unwrap();
        assert!(!desc.contains("onerror"));
        assert!(desc.contains("src=\"a.jpg\""));
    }

    #[test]
    fn unified_filter_overrides_individual() {
        let mut data = sample_data();
        // filter=food should match "Cooking Tips" (category "food")
        // filter_title=Rust should be IGNORED because unified filter is set
        let params = FilterParams {
            filter: Some("food".into()),
            filter_title: Some("Rust".into()),
            ..Default::default()
        };
        apply_filters(&mut data, &params);
        // Only "Cooking Tips" matches unified filter "food"
        assert_eq!(data.items.len(), 1);
        assert_eq!(data.items[0].title, "Cooking Tips");
    }
}
