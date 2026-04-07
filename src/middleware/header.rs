use axum::{
    body::Body,
    extract::{Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::Response,
};
use std::sync::Arc;

use crate::config::Config;

/// Header middleware: sets Cache-Control, CORS, ETag, and handles 304.
pub async fn header_middleware(
    State(config): State<Arc<Config>>,
    request: Request,
    next: Next,
) -> Response {
    let if_none_match = request
        .headers()
        .get(header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let mut response = next.run(request).await;

    // CORS + Cache-Control (scoped borrow)
    {
        let max_age = config.cache_expire;
        let headers = response.headers_mut();
        headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*".parse().unwrap());
        headers.insert(header::ACCESS_CONTROL_ALLOW_METHODS, "GET".parse().unwrap());
        headers.insert(header::X_CONTENT_TYPE_OPTIONS, "nosniff".parse().unwrap());
        headers.insert(
            header::CACHE_CONTROL,
            format!("public, max-age={max_age}").parse().unwrap(),
        );
    }

    // ETag: compute from Data in extensions
    if let Some(data) = response.extensions().get::<crate::data::Data>() {
        let etag = compute_etag(data);
        let etag_header = format!("\"{etag}\"");

        // Check If-None-Match → 304
        if let Some(ref inm) = if_none_match {
            if inm.trim_matches('"') == etag {
                let mut not_modified = Response::new(Body::empty());
                *not_modified.status_mut() = StatusCode::NOT_MODIFIED;
                *not_modified.headers_mut() = response.headers().clone();
                not_modified
                    .headers_mut()
                    .insert(header::ETAG, etag_header.parse().unwrap());
                return not_modified;
            }
        }

        response
            .headers_mut()
            .insert(header::ETAG, etag_header.parse().unwrap());
    }

    response
}

/// Compute ETag from Data by hashing a stable JSON representation.
/// Excludes `updated` field since it changes every request for feeds without explicit dates.
fn compute_etag(data: &crate::data::Data) -> String {
    use xxhash_rust::xxh64::xxh64;

    // Hash title + items (excluding updated/lastBuildDate which change)
    let mut input = data.title.clone();
    for item in &data.items {
        input.push_str(&item.title);
        if let Some(ref link) = item.link {
            input.push_str(link);
        }
        if let Some(ref desc) = item.description {
            input.push_str(desc);
        }
        if let Some(ref guid) = item.guid {
            input.push_str(guid);
        }
    }

    format!("{:016x}", xxh64(input.as_bytes(), 0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{Data, DataItem};

    #[test]
    fn computes_etag() {
        let mut data = Data::new("Test");
        data.items.push(DataItem::new("Item 1"));
        let etag = compute_etag(&data);
        assert_eq!(etag.len(), 16); // xxh64 hex = 16 chars
    }

    #[test]
    fn etag_is_deterministic() {
        let mut data = Data::new("Test");
        data.items.push(DataItem::new("Item 1"));
        assert_eq!(compute_etag(&data), compute_etag(&data));
    }

    #[test]
    fn etag_changes_with_content() {
        let mut d1 = Data::new("Feed A");
        d1.items.push(DataItem::new("Item"));
        let mut d2 = Data::new("Feed B");
        d2.items.push(DataItem::new("Item"));
        assert_ne!(compute_etag(&d1), compute_etag(&d2));
    }

    #[test]
    fn etag_ignores_updated_field() {
        let mut d1 = Data::new("Feed");
        d1.items.push(DataItem::new("Item"));
        d1.updated = Some(chrono::Utc::now());

        let mut d2 = Data::new("Feed");
        d2.items.push(DataItem::new("Item"));
        // d2.updated is None

        assert_eq!(compute_etag(&d1), compute_etag(&d2));
    }
}
