use axum::{
    body::Body,
    extract::Request,
    http::{header, StatusCode},
    middleware::Next,
    response::Response,
};

use crate::data::Data;
use crate::feed::{self, FeedFormat};

/// Template middleware: renders Data → RSS/Atom/JSON Feed response body.
///
/// Expects the route handler to insert `Data` as a response extension.
/// Reads `?format=` from query params (default: rss).
pub async fn template(request: Request, next: Next) -> Response {
    // Extract format from query params before passing to handler
    let format = extract_format(request.uri().query());

    let response = next.run(request).await;

    // If response already has a body with content (non-feed routes like /healthz), pass through
    if response.extensions().get::<Data>().is_none() {
        return response;
    }

    let data = response.extensions().get::<Data>().unwrap().clone();
    let orig_headers = response.headers().clone();

    match feed::render(&data, format) {
        Ok(body) => {
            let mut resp = Response::new(Body::from(body));
            *resp.status_mut() = StatusCode::OK;
            // Preserve headers from inner middleware (e.g. X-OpenRss-Cache)
            *resp.headers_mut() = orig_headers;
            resp.headers_mut().insert(
                header::CONTENT_TYPE,
                format.content_type().parse().unwrap(),
            );
            // Carry over extensions
            resp.extensions_mut().insert(data);
            resp
        }
        Err(e) => {
            let mut resp = Response::new(Body::from(e.to_string()));
            *resp.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
            resp
        }
    }
}

fn extract_format(query: Option<&str>) -> FeedFormat {
    let query = match query {
        Some(q) => q,
        None => return FeedFormat::default(),
    };

    for pair in query.split('&') {
        if let Some(val) = pair.strip_prefix("format=") {
            return FeedFormat::from_str_loose(val);
        }
    }

    FeedFormat::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_format_is_rss() {
        assert_eq!(extract_format(None), FeedFormat::Rss);
        assert_eq!(extract_format(Some("")), FeedFormat::Rss);
    }

    #[test]
    fn format_atom() {
        assert_eq!(extract_format(Some("format=atom")), FeedFormat::Atom);
    }

    #[test]
    fn format_json() {
        assert_eq!(extract_format(Some("format=json")), FeedFormat::Json);
    }

    #[test]
    fn format_with_other_params() {
        assert_eq!(
            extract_format(Some("limit=5&format=atom&filter=test")),
            FeedFormat::Atom
        );
    }

    #[test]
    fn unknown_format_defaults_to_rss() {
        assert_eq!(extract_format(Some("format=xml")), FeedFormat::Rss);
    }
}
