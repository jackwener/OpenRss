use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::Writer;
use std::io::Cursor;

use crate::data::Data;
use crate::error::AppError;

/// Render `Data` as an RSS 2.0 XML document.
pub fn render(data: &Data) -> Result<String, AppError> {
    let mut writer = Writer::new_with_indent(Cursor::new(Vec::new()), b' ', 2);

    // XML declaration
    writer
        .write_event(Event::Decl(BytesDecl::new("1.0", Some("utf-8"), None)))
        .map_err(|e| AppError::Render(e.to_string()))?;

    // <rss version="2.0">
    let mut rss = BytesStart::new("rss");
    rss.push_attribute(("version", "2.0"));
    rss.push_attribute(("xmlns:atom", "http://www.w3.org/2005/Atom"));
    writer
        .write_event(Event::Start(rss))
        .map_err(|e| AppError::Render(e.to_string()))?;

    // <channel>
    writer
        .write_event(Event::Start(BytesStart::new("channel")))
        .map_err(|e| AppError::Render(e.to_string()))?;

    write_text_element(&mut writer, "title", &data.title)?;

    if let Some(ref link) = data.link {
        write_text_element(&mut writer, "link", link)?;
        // atom:link self reference
        let mut atom_link = BytesStart::new("atom:link");
        atom_link.push_attribute(("href", link.as_str()));
        atom_link.push_attribute(("rel", "self"));
        atom_link.push_attribute(("type", "application/rss+xml"));
        writer
            .write_event(Event::Empty(atom_link))
            .map_err(|e| AppError::Render(e.to_string()))?;
    }

    if let Some(ref desc) = data.description {
        write_text_element(&mut writer, "description", desc)?;
    } else {
        write_text_element(&mut writer, "description", &data.title)?;
    }

    if let Some(ref lang) = data.language {
        write_text_element(&mut writer, "language", lang)?;
    }

    if let Some(ref image) = data.image {
        writer
            .write_event(Event::Start(BytesStart::new("image")))
            .map_err(|e| AppError::Render(e.to_string()))?;
        write_text_element(&mut writer, "url", image)?;
        write_text_element(&mut writer, "title", &data.title)?;
        if let Some(ref link) = data.link {
            write_text_element(&mut writer, "link", link)?;
        }
        writer
            .write_event(Event::End(BytesEnd::new("image")))
            .map_err(|e| AppError::Render(e.to_string()))?;
    }

    if let Some(ttl) = data.ttl {
        write_text_element(&mut writer, "ttl", &ttl.to_string())?;
    }

    if let Some(ref updated) = data.updated {
        write_text_element(&mut writer, "lastBuildDate", &updated.to_rfc2822())?;
    }

    // Items
    for item in &data.items {
        writer
            .write_event(Event::Start(BytesStart::new("item")))
            .map_err(|e| AppError::Render(e.to_string()))?;

        write_text_element(&mut writer, "title", &item.title)?;

        if let Some(ref desc) = item.description {
            write_cdata_element(&mut writer, "description", desc)?;
        }

        if let Some(ref link) = item.link {
            write_text_element(&mut writer, "link", link)?;
        }

        if let Some(ref date) = item.pub_date {
            write_text_element(&mut writer, "pubDate", &date.to_rfc2822())?;
        }

        if let Some(ref author) = item.author {
            write_text_element(&mut writer, "author", author)?;
        }

        for cat in &item.category {
            write_text_element(&mut writer, "category", cat)?;
        }

        if let Some(ref guid) = item.guid {
            let mut guid_el = BytesStart::new("guid");
            guid_el.push_attribute(("isPermaLink", "false"));
            writer
                .write_event(Event::Start(guid_el))
                .map_err(|e| AppError::Render(e.to_string()))?;
            writer
                .write_event(Event::Text(BytesText::new(guid)))
                .map_err(|e| AppError::Render(e.to_string()))?;
            writer
                .write_event(Event::End(BytesEnd::new("guid")))
                .map_err(|e| AppError::Render(e.to_string()))?;
        }

        if let Some(ref url) = item.enclosure_url {
            let mut enc = BytesStart::new("enclosure");
            enc.push_attribute(("url", url.as_str()));
            if let Some(ref t) = item.enclosure_type {
                enc.push_attribute(("type", t.as_str()));
            }
            if let Some(len) = item.enclosure_length {
                enc.push_attribute(("length", len.to_string().as_str()));
            }
            writer
                .write_event(Event::Empty(enc))
                .map_err(|e| AppError::Render(e.to_string()))?;
        }

        writer
            .write_event(Event::End(BytesEnd::new("item")))
            .map_err(|e| AppError::Render(e.to_string()))?;
    }

    // </channel>
    writer
        .write_event(Event::End(BytesEnd::new("channel")))
        .map_err(|e| AppError::Render(e.to_string()))?;

    // </rss>
    writer
        .write_event(Event::End(BytesEnd::new("rss")))
        .map_err(|e| AppError::Render(e.to_string()))?;

    let result = writer.into_inner().into_inner();
    String::from_utf8(result).map_err(|e| AppError::Render(e.to_string()))
}

fn write_text_element(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    tag: &str,
    text: &str,
) -> Result<(), AppError> {
    writer
        .write_event(Event::Start(BytesStart::new(tag)))
        .map_err(|e| AppError::Render(e.to_string()))?;
    writer
        .write_event(Event::Text(BytesText::new(text)))
        .map_err(|e| AppError::Render(e.to_string()))?;
    writer
        .write_event(Event::End(BytesEnd::new(tag)))
        .map_err(|e| AppError::Render(e.to_string()))?;
    Ok(())
}

fn write_cdata_element(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    tag: &str,
    content: &str,
) -> Result<(), AppError> {
    writer
        .write_event(Event::Start(BytesStart::new(tag)))
        .map_err(|e| AppError::Render(e.to_string()))?;
    writer
        .write_event(Event::CData(quick_xml::events::BytesCData::new(content)))
        .map_err(|e| AppError::Render(e.to_string()))?;
    writer
        .write_event(Event::End(BytesEnd::new(tag)))
        .map_err(|e| AppError::Render(e.to_string()))?;
    Ok(())
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
        item.description = Some("<p>Hello <b>world</b></p>".into());
        item.pub_date = Some(Utc.with_ymd_and_hms(2025, 1, 15, 12, 0, 0).unwrap());
        item.author = Some("Alice".into());
        item.category = vec!["tech".into(), "rust".into()];
        item.guid = Some("guid-001".into());
        data.items.push(item);

        data
    }

    #[test]
    fn rss_renders_valid_xml() {
        let xml = render(&sample_data()).unwrap();
        assert!(xml.starts_with("<?xml version=\"1.0\" encoding=\"utf-8\"?>"));
        assert!(xml.contains("<rss version=\"2.0\""));
        assert!(xml.contains("</rss>"));
        // Verify it parses back
        quick_xml::Reader::from_str(&xml);
    }

    #[test]
    fn rss_includes_channel_metadata() {
        let xml = render(&sample_data()).unwrap();
        assert!(xml.contains("<title>Test Feed</title>"));
        assert!(xml.contains("<link>https://example.com</link>"));
        assert!(xml.contains("<description>A test feed</description>"));
        assert!(xml.contains("<language>en</language>"));
    }

    #[test]
    fn rss_includes_item_fields() {
        let xml = render(&sample_data()).unwrap();
        assert!(xml.contains("<title>Item 1</title>"));
        assert!(xml.contains("<link>https://example.com/1</link>"));
        assert!(xml.contains("<author>Alice</author>"));
        assert!(xml.contains("<category>tech</category>"));
        assert!(xml.contains("<category>rust</category>"));
    }

    #[test]
    fn rss_includes_pubdate() {
        let xml = render(&sample_data()).unwrap();
        // RFC 2822 date format
        assert!(xml.contains("<pubDate>"));
        assert!(xml.contains("2025"));
    }

    #[test]
    fn rss_includes_guid() {
        let xml = render(&sample_data()).unwrap();
        assert!(xml.contains("<guid isPermaLink=\"false\">guid-001</guid>"));
    }

    #[test]
    fn rss_description_uses_cdata() {
        let xml = render(&sample_data()).unwrap();
        assert!(xml.contains("<![CDATA[<p>Hello <b>world</b></p>]]>"));
    }

    #[test]
    fn rss_handles_empty_items() {
        let data = Data::new("Empty Feed");
        let xml = render(&data).unwrap();
        assert!(xml.contains("<title>Empty Feed</title>"));
        assert!(!xml.contains("<item>"));
    }

    #[test]
    fn rss_includes_enclosure() {
        let mut data = Data::new("Podcast");
        let mut item = DataItem::new("Episode 1");
        item.enclosure_url = Some("https://example.com/ep1.mp3".into());
        item.enclosure_type = Some("audio/mpeg".into());
        item.enclosure_length = Some(12345678);
        data.items.push(item);

        let xml = render(&data).unwrap();
        assert!(xml.contains("enclosure"));
        assert!(xml.contains("url=\"https://example.com/ep1.mp3\""));
        assert!(xml.contains("type=\"audio/mpeg\""));
        assert!(xml.contains("length=\"12345678\""));
    }

    #[test]
    fn rss_escapes_html_in_title() {
        let mut data = Data::new("Feed with <special> & \"chars\"");
        data.items.push(DataItem::new("Item <1> & \"2\""));

        let xml = render(&data).unwrap();
        // Title text should be XML-escaped
        assert!(xml.contains("&lt;special&gt;"));
        assert!(xml.contains("&amp;"));
    }

    #[test]
    fn rss_includes_image() {
        let mut data = Data::new("Feed");
        data.link = Some("https://example.com".into());
        data.image = Some("https://example.com/logo.png".into());

        let xml = render(&data).unwrap();
        assert!(xml.contains("<image>"));
        assert!(xml.contains("<url>https://example.com/logo.png</url>"));
    }

    #[test]
    fn rss_includes_ttl() {
        let mut data = Data::new("Feed");
        data.ttl = Some(30);

        let xml = render(&data).unwrap();
        assert!(xml.contains("<ttl>30</ttl>"));
    }

    #[test]
    fn rss_description_defaults_to_title() {
        let data = Data::new("My Feed");
        let xml = render(&data).unwrap();
        assert!(xml.contains("<description>My Feed</description>"));
    }
}
