pub mod atom;
pub mod json;
pub mod rss;
pub mod xml_utils;

/// Output format for feed rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FeedFormat {
    #[default]
    Rss,
    Atom,
    Json,
}

impl FeedFormat {
    pub fn from_str_loose(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "atom" => FeedFormat::Atom,
            "json" => FeedFormat::Json,
            _ => FeedFormat::Rss,
        }
    }

    pub fn content_type(&self) -> &'static str {
        match self {
            FeedFormat::Rss => "application/rss+xml; charset=utf-8",
            FeedFormat::Atom => "application/atom+xml; charset=utf-8",
            FeedFormat::Json => "application/feed+json; charset=utf-8",
        }
    }
}

/// Render feed data to the requested format.
pub fn render(data: &crate::data::Data, format: FeedFormat) -> Result<String, crate::error::AppError> {
    match format {
        FeedFormat::Rss => rss::render(data),
        FeedFormat::Atom => atom::render(data),
        FeedFormat::Json => json::render(data),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_format_is_rss() {
        assert_eq!(FeedFormat::default(), FeedFormat::Rss);
    }

    #[test]
    fn format_from_str_loose() {
        assert_eq!(FeedFormat::from_str_loose("atom"), FeedFormat::Atom);
        assert_eq!(FeedFormat::from_str_loose("ATOM"), FeedFormat::Atom);
        assert_eq!(FeedFormat::from_str_loose("json"), FeedFormat::Json);
        assert_eq!(FeedFormat::from_str_loose("rss"), FeedFormat::Rss);
        assert_eq!(FeedFormat::from_str_loose("anything"), FeedFormat::Rss);
    }

    #[test]
    fn content_types() {
        assert!(FeedFormat::Rss.content_type().contains("rss+xml"));
        assert!(FeedFormat::Atom.content_type().contains("atom+xml"));
        assert!(FeedFormat::Json.content_type().contains("application/feed+json"));
    }
}
