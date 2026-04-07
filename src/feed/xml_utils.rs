use quick_xml::events::{BytesCData, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::Writer;
use std::io::Cursor;

use crate::error::AppError;

pub type XmlWriter = Writer<Cursor<Vec<u8>>>;

pub fn write_text_element(writer: &mut XmlWriter, tag: &str, text: &str) -> Result<(), AppError> {
    writer.write_event(Event::Start(BytesStart::new(tag)))?;
    writer.write_event(Event::Text(BytesText::new(text)))?;
    writer.write_event(Event::End(BytesEnd::new(tag)))?;
    Ok(())
}

pub fn write_cdata_element(
    writer: &mut XmlWriter,
    tag: &str,
    content: &str,
) -> Result<(), AppError> {
    writer.write_event(Event::Start(BytesStart::new(tag)))?;
    writer.write_event(Event::CData(BytesCData::new(content)))?;
    writer.write_event(Event::End(BytesEnd::new(tag)))?;
    Ok(())
}
