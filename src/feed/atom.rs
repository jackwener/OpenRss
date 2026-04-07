use quick_xml::events::{BytesCData, BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::Writer;
use std::io::Cursor;

use crate::data::Data;
use crate::error::AppError;

/// Render `Data` as an Atom 1.0 XML document.
pub fn render(data: &Data) -> Result<String, AppError> {
    let mut writer = Writer::new_with_indent(Cursor::new(Vec::new()), b' ', 2);

    // XML declaration
    writer
        .write_event(Event::Decl(BytesDecl::new("1.0", Some("utf-8"), None)))
        .map_err(|e| AppError::Render(e.to_string()))?;

    // <feed xmlns="http://www.w3.org/2005/Atom">
    let mut feed = BytesStart::new("feed");
    feed.push_attribute(("xmlns", "http://www.w3.org/2005/Atom"));
    writer
        .write_event(Event::Start(feed))
        .map_err(|e| AppError::Render(e.to_string()))?;

    write_text_element(&mut writer, "title", &data.title)?;

    if let Some(ref link) = data.link {
        let mut link_el = BytesStart::new("link");
        link_el.push_attribute(("href", link.as_str()));
        link_el.push_attribute(("rel", "alternate"));
        writer
            .write_event(Event::Empty(link_el))
            .map_err(|e| AppError::Render(e.to_string()))?;

        // Self link
        let mut self_link = BytesStart::new("link");
        self_link.push_attribute(("href", link.as_str()));
        self_link.push_attribute(("rel", "self"));
        self_link.push_attribute(("type", "application/atom+xml"));
        writer
            .write_event(Event::Empty(self_link))
            .map_err(|e| AppError::Render(e.to_string()))?;

        // Feed ID = link
        write_text_element(&mut writer, "id", link)?;
    }

    if let Some(ref desc) = data.description {
        write_text_element(&mut writer, "subtitle", desc)?;
    }

    // updated: use data.updated, or the most recent item date, or now
    let updated = data
        .updated
        .or_else(|| data.items.iter().filter_map(|i| i.pub_date).max())
        .unwrap_or_else(chrono::Utc::now);
    write_text_element(&mut writer, "updated", &updated.to_rfc3339())?;

    if let Some(ref image) = data.image {
        write_text_element(&mut writer, "icon", image)?;
    }

    if let Some(ref lang) = data.language {
        // Atom uses xml:lang on the feed element, but we can also emit it here
        write_text_element(&mut writer, "dc:language", lang)?;
    }

    // Entries
    for item in &data.items {
        writer
            .write_event(Event::Start(BytesStart::new("entry")))
            .map_err(|e| AppError::Render(e.to_string()))?;

        write_text_element(&mut writer, "title", &item.title)?;

        if let Some(ref link) = item.link {
            let mut link_el = BytesStart::new("link");
            link_el.push_attribute(("href", link.as_str()));
            writer
                .write_event(Event::Empty(link_el))
                .map_err(|e| AppError::Render(e.to_string()))?;

            // Use link as id if no guid
            let id = item.guid.as_deref().unwrap_or(link.as_str());
            write_text_element(&mut writer, "id", id)?;
        } else if let Some(ref guid) = item.guid {
            write_text_element(&mut writer, "id", guid)?;
        }

        if let Some(ref desc) = item.description {
            // content type="html"
            let mut content = BytesStart::new("content");
            content.push_attribute(("type", "html"));
            writer
                .write_event(Event::Start(content))
                .map_err(|e| AppError::Render(e.to_string()))?;
            writer
                .write_event(Event::CData(BytesCData::new(desc)))
                .map_err(|e| AppError::Render(e.to_string()))?;
            writer
                .write_event(Event::End(BytesEnd::new("content")))
                .map_err(|e| AppError::Render(e.to_string()))?;
        }

        if let Some(ref date) = item.pub_date {
            write_text_element(&mut writer, "published", &date.to_rfc3339())?;
            write_text_element(&mut writer, "updated", &date.to_rfc3339())?;
        }

        if let Some(ref author) = item.author {
            writer
                .write_event(Event::Start(BytesStart::new("author")))
                .map_err(|e| AppError::Render(e.to_string()))?;
            write_text_element(&mut writer, "name", author)?;
            writer
                .write_event(Event::End(BytesEnd::new("author")))
                .map_err(|e| AppError::Render(e.to_string()))?;
        }

        for cat in &item.category {
            let mut cat_el = BytesStart::new("category");
            cat_el.push_attribute(("term", cat.as_str()));
            writer
                .write_event(Event::Empty(cat_el))
                .map_err(|e| AppError::Render(e.to_string()))?;
        }

        writer
            .write_event(Event::End(BytesEnd::new("entry")))
            .map_err(|e| AppError::Render(e.to_string()))?;
    }

    // </feed>
    writer
        .write_event(Event::End(BytesEnd::new("feed")))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{Data, DataItem};
    use chrono::{TimeZone, Utc};

    fn sample_data() -> Data {
        let mut data = Data::new("Test Feed");
        data.link = Some("https://example.com".into());
        data.description = Some("A test feed".into());

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
    fn atom_renders_valid_xml() {
        let xml = render(&sample_data()).unwrap();
        assert!(xml.starts_with("<?xml version=\"1.0\" encoding=\"utf-8\"?>"));
        assert!(xml.contains("<feed xmlns=\"http://www.w3.org/2005/Atom\">"));
        assert!(xml.contains("</feed>"));
    }

    #[test]
    fn atom_includes_feed_metadata() {
        let xml = render(&sample_data()).unwrap();
        assert!(xml.contains("<title>Test Feed</title>"));
        assert!(xml.contains("<subtitle>A test feed</subtitle>"));
        assert!(xml.contains("href=\"https://example.com\""));
    }

    #[test]
    fn atom_includes_updated() {
        let xml = render(&sample_data()).unwrap();
        // ISO 8601 date
        assert!(xml.contains("<updated>"));
        assert!(xml.contains("2025"));
    }

    #[test]
    fn atom_includes_entry_fields() {
        let xml = render(&sample_data()).unwrap();
        assert!(xml.contains("<entry>"));
        assert!(xml.contains("<title>Item 1</title>"));
        assert!(xml.contains("href=\"https://example.com/1\""));
        assert!(xml.contains("<id>guid-001</id>"));
    }

    #[test]
    fn atom_includes_author() {
        let xml = render(&sample_data()).unwrap();
        assert!(xml.contains("<author>"));
        assert!(xml.contains("<name>Alice</name>"));
    }

    #[test]
    fn atom_includes_category() {
        let xml = render(&sample_data()).unwrap();
        assert!(xml.contains("term=\"tech\""));
    }

    #[test]
    fn atom_content_is_html_cdata() {
        let xml = render(&sample_data()).unwrap();
        assert!(xml.contains("type=\"html\""));
        assert!(xml.contains("<![CDATA[<p>Hello</p>]]>"));
    }

    #[test]
    fn atom_handles_empty_items() {
        let data = Data::new("Empty");
        let xml = render(&data).unwrap();
        assert!(xml.contains("<title>Empty</title>"));
        assert!(!xml.contains("<entry>"));
    }

    #[test]
    fn atom_uses_link_as_id_when_no_guid() {
        let mut data = Data::new("Feed");
        let mut item = DataItem::new("No GUID");
        item.link = Some("https://example.com/x".into());
        // guid is None
        data.items.push(item);

        let xml = render(&data).unwrap();
        assert!(xml.contains("<id>https://example.com/x</id>"));
    }
}
