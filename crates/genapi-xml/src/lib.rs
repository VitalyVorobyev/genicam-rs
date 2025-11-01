//! Load + pre-parse GenICam XML (no evaluation). Use quick-xml.

use quick_xml::Reader;
use quick_xml::events::Event;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum XmlError {
    #[error("xml: {0}")]
    Xml(String),
}

pub fn parse_into_minimal_nodes(xml: &str) -> Result<Vec<String>, XmlError> {
    // Example: collect node names only; flesh out later
    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);
    let mut names = Vec::new();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if tag.ends_with("Node") { names.push(tag); }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(XmlError::Xml(e.to_string())),
            _ => {}
        }
        buf.clear();
    }
    Ok(names)
}
