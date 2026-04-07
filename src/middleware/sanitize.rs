use scraper::Html;

/// Sanitize HTML content: remove scripts, event handlers, fix lazyload, add referrerpolicy.
pub fn sanitize_html(html: &str) -> String {
    if html.is_empty() {
        return String::new();
    }

    let mut result = html.to_string();

    // 1. Remove <script> tags and their content
    result = remove_script_tags(&result);

    // 2. Remove on* event attributes (onclick, onerror, onload, etc.)
    result = remove_event_attributes(&result);

    // 3. Fix lazyload images: data-src / data-original → src
    result = fix_lazyload_images(&result);

    // 4. Add referrerpolicy="no-referrer" to img and iframe
    result = add_referrer_policy(&result);

    result
}

/// Strip all HTML tags, returning plain text.
pub fn strip_html_tags(html: &str) -> String {
    let fragment = Html::parse_fragment(html);
    let mut text = String::new();
    for node in fragment.root_element().text() {
        text.push_str(node);
    }
    text
}

/// Truncate text to N characters, adding "..." if truncated.
pub fn brief(html: &str, max_chars: usize) -> String {
    let plain = strip_html_tags(html);
    if plain.chars().count() <= max_chars {
        return plain;
    }
    let truncated: String = plain.chars().take(max_chars).collect();
    format!("{truncated}...")
}

fn remove_script_tags(html: &str) -> String {
    // Use regex to remove <script>...</script> including content
    let re = regex::RegexBuilder::new(r"<script\b[^>]*>[\s\S]*?</script>")
        .case_insensitive(true)
        .build()
        .unwrap();
    re.replace_all(html, "").to_string()
}

fn remove_event_attributes(html: &str) -> String {
    // Remove on* attributes like onclick="...", onerror='...', onload=...
    let re = regex::RegexBuilder::new(r#"\s+on\w+\s*=\s*(?:"[^"]*"|'[^']*'|\S+)"#)
        .case_insensitive(true)
        .build()
        .unwrap();
    re.replace_all(html, "").to_string()
}

fn fix_lazyload_images(html: &str) -> String {
    // Replace data-src="..." with src="..." on img tags that have no src or empty src
    let re = regex::RegexBuilder::new(
        r#"(<img\b[^>]*?)(?:\s+src\s*=\s*(?:"[^"]*"|'[^']*'))?\s+(data-(?:src|original)\s*=\s*(?:"[^"]*"|'[^']*'))"#,
    )
    .case_insensitive(true)
    .build()
    .unwrap();

    re.replace_all(html, |caps: &regex::Captures| {
        let prefix = &caps[1];
        let data_attr = &caps[2];
        // Extract the URL from data-src="url" or data-original="url"
        let url_re = regex::Regex::new(r#"data-(?:src|original)\s*=\s*"([^"]*)""#).unwrap();
        if let Some(url_caps) = url_re.captures(data_attr) {
            let url = &url_caps[1];
            format!("{prefix} src=\"{url}\"")
        } else {
            let url_re = regex::Regex::new(r#"data-(?:src|original)\s*=\s*'([^']*)'"#).unwrap();
            if let Some(url_caps) = url_re.captures(data_attr) {
                let url = &url_caps[1];
                format!("{prefix} src='{url}'")
            } else {
                caps[0].to_string()
            }
        }
    })
    .to_string()
}

fn add_referrer_policy(html: &str) -> String {
    // Add referrerpolicy="no-referrer" to <img> and <iframe> that don't already have it
    let re = regex::RegexBuilder::new(r"<(img|iframe)\b([^>]*?)(/?)>")
        .case_insensitive(true)
        .build()
        .unwrap();

    re.replace_all(html, |caps: &regex::Captures| {
        let tag = &caps[1];
        let attrs = &caps[2];
        let close = &caps[3];
        if attrs.contains("referrerpolicy") {
            caps[0].to_string()
        } else {
            format!("<{tag}{attrs} referrerpolicy=\"no-referrer\"{close}>")
        }
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_script_tags() {
        let html = r#"<p>Hello</p><script>alert("xss")</script><p>World</p>"#;
        let result = sanitize_html(html);
        assert!(!result.contains("script"));
        assert!(!result.contains("alert"));
        assert!(result.contains("<p>Hello</p>"));
        assert!(result.contains("<p>World</p>"));
    }

    #[test]
    fn removes_script_with_attributes() {
        let html = r#"<script type="text/javascript" src="evil.js"></script><p>Safe</p>"#;
        let result = sanitize_html(html);
        assert!(!result.contains("script"));
        assert!(result.contains("<p>Safe</p>"));
    }

    #[test]
    fn removes_event_handlers() {
        let html = r##"<img src="a.jpg" onerror="alert(1)"><a href="#" onclick="steal()">click</a>"##;
        let result = sanitize_html(html);
        assert!(!result.contains("onerror"));
        assert!(!result.contains("onclick"));
        assert!(!result.contains("alert"));
        assert!(!result.contains("steal"));
        assert!(result.contains("src=\"a.jpg\""));
        assert!(result.contains("href=\"#\""));
    }

    #[test]
    fn fixes_lazyload_data_src() {
        let html = r#"<img data-src="https://example.com/img.jpg" src="">"#;
        let result = sanitize_html(html);
        assert!(result.contains("src=\"https://example.com/img.jpg\""));
    }

    #[test]
    fn fixes_lazyload_data_original() {
        let html = r#"<img data-original="https://example.com/photo.png">"#;
        let result = sanitize_html(html);
        assert!(result.contains("src=\"https://example.com/photo.png\""));
    }

    #[test]
    fn adds_referrer_policy_to_img() {
        let html = r#"<img src="photo.jpg">"#;
        let result = sanitize_html(html);
        assert!(result.contains("referrerpolicy=\"no-referrer\""));
    }

    #[test]
    fn adds_referrer_policy_to_iframe() {
        let html = r#"<iframe src="https://example.com"></iframe>"#;
        let result = sanitize_html(html);
        assert!(result.contains("referrerpolicy=\"no-referrer\""));
    }

    #[test]
    fn does_not_duplicate_referrer_policy() {
        let html = r#"<img src="a.jpg" referrerpolicy="origin">"#;
        let result = sanitize_html(html);
        // Should keep existing policy, not add another
        assert!(result.contains("referrerpolicy=\"origin\""));
        assert_eq!(result.matches("referrerpolicy").count(), 1);
    }

    #[test]
    fn self_closing_img() {
        let html = r#"<img src="a.jpg" />"#;
        let result = sanitize_html(html);
        assert!(result.contains("referrerpolicy=\"no-referrer\""));
    }

    #[test]
    fn strip_tags() {
        let html = "<p>Hello <b>world</b></p><br/><a href='#'>link</a>";
        assert_eq!(strip_html_tags(html), "Hello worldlink");
    }

    #[test]
    fn brief_truncates() {
        let html = "<p>This is a long description that should be truncated</p>";
        let result = brief(html, 10);
        assert_eq!(result, "This is a ...");
    }

    #[test]
    fn brief_no_truncate_when_short() {
        let html = "<p>Short</p>";
        let result = brief(html, 100);
        assert_eq!(result, "Short");
    }

    #[test]
    fn brief_handles_unicode() {
        let html = "<p>你好世界这是一个测试</p>";
        let result = brief(html, 4);
        assert_eq!(result, "你好世界...");
    }

    #[test]
    fn sanitize_empty() {
        assert_eq!(sanitize_html(""), "");
    }

    #[test]
    fn sanitize_plain_text() {
        assert_eq!(sanitize_html("just text"), "just text");
    }

    #[test]
    fn combined_sanitization() {
        let html = r#"<p>Content</p><script>evil()</script><img onerror="x" data-src="real.jpg" src="placeholder.gif"><iframe src="video.mp4"></iframe>"#;
        let result = sanitize_html(html);
        assert!(!result.contains("script"));
        assert!(!result.contains("evil"));
        assert!(!result.contains("onerror"));
        assert!(result.contains("src=\"real.jpg\""));
        assert!(result.contains("referrerpolicy=\"no-referrer\""));
    }
}
