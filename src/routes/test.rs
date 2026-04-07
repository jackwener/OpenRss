use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use chrono::{TimeZone, Utc};

use crate::data::{Data, DataItem};
use crate::error::AppError;
use crate::registry::{AppState, RouteDefinition};

/// Returns a test route with fixed data, for verifying the middleware chain.
pub fn routes() -> Vec<RouteDefinition> {
    vec![RouteDefinition {
        path: "/test/example",
        name: "test/example",
        example: "/test/example",
        handler: test_handler,
    }]
}

fn test_handler(
    _state: Arc<AppState>,
    _path: HashMap<String, String>,
    _query: HashMap<String, String>,
) -> Pin<Box<dyn Future<Output = Result<Data, AppError>> + Send>> {
    Box::pin(async {
        let mut data = Data::new("Test Feed");
        data.link = Some("https://example.com".into());
        data.description = Some("A test feed for OpenRss".into());
        data.language = Some("en".into());

        for i in 1..=5 {
            let mut item = DataItem::new(format!("Test Item {i}"));
            item.link = Some(format!("https://example.com/{i}"));
            item.description = Some(format!("<p>Content for item {i}</p>"));
            item.pub_date = Some(
                Utc.with_ymd_and_hms(2025, 1, 10 + i, 12, 0, 0).unwrap(),
            );
            item.author = Some("OpenRss".into());
            item.guid = Some(format!("test-{i}"));
            data.items.push(item);
        }

        Ok(data)
    })
}
