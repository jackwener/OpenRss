use quick_xml::events::{BytesCData, BytesDecl, BytesEnd, BytesStart, Event};
use quick_xml::Writer;
use std::io::Cursor;

use super::xml_utils::write_text_element;
use crate::data::Data;
use crate::error::AppError;

/// Render `Data` as an Atom 1.0 XML document.
pub fn render(data: &Data) -> Result<String, AppError> {
    let mut writer = Writer::new_with_indent(Cursor::new(Vec::new()), b' ', 2);

    // XML declaration
    writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("utf-8"), None)))?;

    // <feed xmlns="http://www.w3.org/2005/Atom">
    let mut feed = BytesStart::new("feed");
    feed.push_attribute(("xmlns", "http://www.w3.org/2005/Atom"));
    if let Some(ref lang) = data.language {
        feed.push_attribute(("xml:lang", lang.as_str()));
    }
    writer.write_event(Event::Start(feed))?;

    write_text_element(&mut writer, "title", &data.title)?;
    write_text_element(&mut writer, "generator", "OpenRss")?;

    if let Some(ref link) = data.link {
        let mut link_el = BytesStart::new("link");
        link_el.push_attribute(("href", link.as_str()));
        link_el.push_attribute(("rel", "alternate"));
        writer.write_event(Event::Empty(link_el))?;

        // Self link
        let mut self_link = BytesStart::new("link");
        self_link.push_attribute(("href", link.as_str()));
        self_link.push_attribute(("rel", "self"));
        self_link.push_attribute(("type", "application/atom+xml"));
        writer.write_event(Event::Empty(self_link))?;

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

    // Entries
    for item in &data.items {
        writer.write_event(Event::Start(BytesStart::new("entry")))?;

        write_text_element(&mut writer, "title", &item.title)?;

        if let Some(ref link) = item.link {
            let mut link_el = BytesStart::new("link");
            link_el.push_attribute(("href", link.as_str()));
            writer.write_event(Event::Empty(link_el))?;
        }

        // <id> is REQUIRED in Atom. Fallback chain: guid -> link -> UUID
        let id = item
            .guid
            .as_deref()
            .or(item.link.as_deref())
            .map(|s| s.to_string())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        write_text_element(&mut writer, "id", &id)?;

        if let Some(ref desc) = item.description {
            // content type="html"
            let mut content = BytesStart::new("content");
            content.push_attribute(("type", "html"));
            writer.write_event(Event::Start(content))?;
            writer.write_event(Event::CData(BytesCData::new(desc)))?;
            writer.write_event(Event::End(BytesEnd::new("content")))?;
        }

        if let Some(ref date) = item.pub_date {
            write_text_element(&mut writer, "published", &date.to_rfc3339())?;
            write_text_element(&mut writer, "updated", &date.to_rfc3339())?;
        }

        if let Some(ref author) = item.author {
            writer.write_event(Event::Start(BytesStart::new("author")))?;
            write_text_element(&mut writer, "name", author)?;
            writer.write_event(Event::End(BytesEnd::new("author")))?;
        }

        for cat in &item.category {
            let mut cat_el = BytesStart::new("category");
            cat_el.push_attribute(("term", cat.as_str()));
            writer.write_event(Event::Empty(cat_el))?;
        }

        writer.write_event(Event::End(BytesEnd::new("entry")))?;
    }

    // </feed>
    writer.write_event(Event::End(BytesEnd::new("feed")))?;

    let result = writer.into_inner().into_inner();
    String::from_utf8(result).map_err(|e| AppError::Render(e.to_string()))
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
    fn atom_renders_valid_xml() {
        let xml = render(&sample_data()).unwrap();
        assert!(xml.starts_with("<?xml version=\"1.0\" encoding=\"utf-8\"?>"));
        assert!(xml.contains("<feed xmlns=\"http://www.w3.org/2005/Atom\""));
        assert!(xml.contains("</feed>"));
    }

    #[test]
    fn atom_includes_xml_lang() {
        let xml = render(&sample_data()).unwrap();
        assert!(xml.contains("xml:lang=\"en\""));
        // Should NOT contain dc:language
        assert!(!xml.contains("dc:language"));
    }

    #[test]
    fn atom_includes_generator() {
        let xml = render(&sample_data()).unwrap();
        assert!(xml.contains("<generator>OpenRss</generator>"));
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
        data.items.push(item);

        let xml = render(&data).unwrap();
        assert!(xml.contains("<id>https://example.com/x</id>"));
    }

    #[test]
    fn atom_generates_uuid_when_no_guid_or_link() {
        let mut data = Data::new("Feed");
        let item = DataItem::new("No GUID or Link");
        data.items.push(item);

        let xml = render(&data).unwrap();
        // Should contain an <id> with a UUID (36 chars with hyphens)
        assert!(xml.contains("<id>"));
        // Extract id content
        let id_start = xml.find("<id>").unwrap() + 4;
        let id_end = xml[id_start..].find("</id>").unwrap() + id_start;
        let id = &xml[id_start..id_end];
        assert_eq!(id.len(), 36); // UUID v4 format
    }

    #[test]
    fn atom_no_xml_lang_when_no_language() {
        let data = Data::new("No Lang");
        let xml = render(&data).unwrap();
        assert!(!xml.contains("xml:lang"));
    }
}
