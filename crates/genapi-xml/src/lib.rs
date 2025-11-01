//! Load and pre-parse GenICam XML using quick-xml.

use std::future::Future;

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;
use thiserror::Error;

const FIRST_URL_ADDRESS: u64 = 0x0000;
const FIRST_URL_MAX_LEN: usize = 512;

#[derive(Debug, Error)]
pub enum XmlError {
    #[error("xml: {0}")]
    Xml(String),
    #[error("invalid descriptor: {0}")]
    Invalid(String),
    #[error("transport: {0}")]
    Transport(String),
    #[error("unsupported URL: {0}")]
    Unsupported(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinimalXmlInfo {
    pub schema_version: Option<String>,
    pub top_level_features: Vec<String>,
}

/// Fetch the GenICam XML document using the provided memory reader closure.
///
/// The closure must return the requested number of bytes starting at the
/// provided address. It can internally perform chunked transfers.
pub async fn fetch_and_load_xml<F, Fut>(mut read_mem: F) -> Result<String, XmlError>
where
    F: FnMut(u64, usize) -> Fut,
    Fut: Future<Output = Result<Vec<u8>, XmlError>>,
{
    let url_bytes = read_mem(FIRST_URL_ADDRESS, FIRST_URL_MAX_LEN).await?;
    let url = first_cstring(&url_bytes)
        .ok_or_else(|| XmlError::Invalid("FirstURL register is empty".into()))?;
    let location = UrlLocation::parse(&url)?;
    match location {
        UrlLocation::Local { address, length } => {
            let xml_bytes = read_mem(address, length).await?;
            String::from_utf8(xml_bytes)
                .map_err(|err| XmlError::Xml(format!("invalid UTF-8: {err}")))
        }
        UrlLocation::LocalNamed(name) => Err(XmlError::Unsupported(format!(
            "named local URL '{name}' is not supported"
        ))),
        UrlLocation::Http(url) => Err(XmlError::Unsupported(format!(
            "HTTP retrieval is not implemented ({url})"
        ))),
        UrlLocation::File(path) => Err(XmlError::Unsupported(format!(
            "file URL '{path}' is not supported"
        ))),
    }
}

/// Parse a GenICam XML snippet and collect minimal metadata.
pub fn parse_into_minimal_nodes(xml: &str) -> Result<MinimalXmlInfo, XmlError> {
    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);
    let mut buf = Vec::new();
    let mut depth = 0usize;
    let mut schema_version: Option<String> = None;
    let mut top_level_features = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                depth += 1;
                handle_start(&e, depth, &mut schema_version, &mut top_level_features)?;
            }
            Ok(Event::Empty(e)) => {
                depth += 1;
                handle_start(&e, depth, &mut schema_version, &mut top_level_features)?;
                if depth > 0 {
                    depth = depth.saturating_sub(1);
                }
            }
            Ok(Event::End(_)) => {
                if depth > 0 {
                    depth = depth.saturating_sub(1);
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(XmlError::Xml(err.to_string())),
            _ => {}
        }
        buf.clear();
    }

    Ok(MinimalXmlInfo {
        schema_version,
        top_level_features,
    })
}

fn handle_start(
    event: &BytesStart<'_>,
    depth: usize,
    schema_version: &mut Option<String>,
    top_level: &mut Vec<String>,
) -> Result<(), XmlError> {
    if depth == 1 && schema_version.is_none() {
        *schema_version = extract_schema_version(event);
    } else if depth == 2 {
        if let Some(name) = attribute_value(event, b"Name")? {
            top_level.push(name);
        } else {
            top_level.push(String::from_utf8_lossy(event.name().as_ref()).to_string());
        }
    }
    Ok(())
}

fn extract_schema_version(event: &BytesStart<'_>) -> Option<String> {
    let major = attribute_value(event, b"SchemaMajorVersion").ok().flatten();
    let minor = attribute_value(event, b"SchemaMinorVersion").ok().flatten();
    let sub = attribute_value(event, b"SchemaSubMinorVersion")
        .ok()
        .flatten();
    if major.is_none() && minor.is_none() && sub.is_none() {
        None
    } else {
        let major = major.unwrap_or_else(|| "0".to_string());
        let minor = minor.unwrap_or_else(|| "0".to_string());
        let sub = sub.unwrap_or_else(|| "0".to_string());
        Some(format!("{major}.{minor}.{sub}"))
    }
}

fn attribute_value(event: &BytesStart<'_>, name: &[u8]) -> Result<Option<String>, XmlError> {
    for attr in event.attributes() {
        let attr = attr.map_err(|err| XmlError::Xml(err.to_string()))?;
        if attr.key.as_ref() == name {
            let value = attr
                .unescape_value()
                .map_err(|err| XmlError::Xml(err.to_string()))?;
            let trimmed = value.trim().to_string();
            if trimmed.is_empty() {
                return Ok(None);
            }
            return Ok(Some(trimmed));
        }
    }
    Ok(None)
}

#[derive(Debug)]
enum UrlLocation {
    Local { address: u64, length: usize },
    LocalNamed(String),
    Http(String),
    File(String),
}

impl UrlLocation {
    fn parse(url: &str) -> Result<Self, XmlError> {
        if let Some(rest) = url.strip_prefix("local:") {
            parse_local_url(rest)
        } else if url.starts_with("http://") || url.starts_with("https://") {
            Ok(UrlLocation::Http(url.to_string()))
        } else if url.starts_with("file://") {
            Ok(UrlLocation::File(url.to_string()))
        } else {
            Err(XmlError::Unsupported(format!("unknown URL scheme: {url}")))
        }
    }
}

fn parse_local_url(rest: &str) -> Result<UrlLocation, XmlError> {
    let trimmed = rest.trim();
    if trimmed.is_empty() {
        return Err(XmlError::Invalid("empty local URL".into()));
    }
    let mut address = None;
    let mut length = None;
    for part in trimmed.split(|c| c == ';' || c == ',') {
        let token = part.trim();
        if token.is_empty() {
            continue;
        }
        if let Some((key, value)) = token.split_once('=') {
            let key = key.trim().to_ascii_lowercase();
            let value = value.trim();
            match key.as_str() {
                "address" | "addr" | "offset" => {
                    address = Some(parse_int(value)?);
                }
                "length" | "size" => {
                    let len = parse_int(value)?;
                    length = Some(
                        len.try_into()
                            .map_err(|_| XmlError::Invalid("length does not fit usize".into()))?,
                    );
                }
                _ => {}
            }
        } else if token.starts_with("0x") {
            address = Some(parse_int(token)?);
        } else {
            return Ok(UrlLocation::LocalNamed(token.to_string()));
        }
    }
    match (address, length) {
        (Some(address), Some(length)) => Ok(UrlLocation::Local { address, length }),
        _ => Err(XmlError::Invalid(format!("unsupported local URL: {rest}"))),
    }
}

fn parse_int(value: &str) -> Result<u64, XmlError> {
    let trimmed = value.trim();
    if let Some(hex) = trimmed.strip_prefix("0x") {
        u64::from_str_radix(hex, 16)
            .map_err(|err| XmlError::Invalid(format!("invalid hex value: {err}")))
    } else {
        trimmed
            .parse()
            .map_err(|err| XmlError::Invalid(format!("invalid integer: {err}")))
    }
}

fn first_cstring(bytes: &[u8]) -> Option<String> {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    let slice = &bytes[..end];
    let value = String::from_utf8_lossy(slice).trim().to_string();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn parse_minimal_xml() {
        let xml = r#"
            <RegisterDescription SchemaMajorVersion="1" SchemaMinorVersion="2" SchemaSubMinorVersion="3">
                <Category Name="Root" />
                <Integer Name="Width" />
            </RegisterDescription>
        "#;
        let info = parse_into_minimal_nodes(xml).expect("parse xml");
        assert_eq!(info.schema_version.as_deref(), Some("1.2.3"));
        assert_eq!(info.top_level_features.len(), 2);
        assert_eq!(info.top_level_features[0], "Root");
        assert_eq!(info.top_level_features[1], "Width");

        let data = b"local:address=0x10;length=0x3\0".to_vec();
        let xml_payload = b"<a/>".to_vec();
        let loaded = fetch_and_load_xml(|addr, len| {
            let data = data.clone();
            let xml_payload = xml_payload.clone();
            async move {
                if addr == FIRST_URL_ADDRESS {
                    Ok(data)
                } else if addr == 0x10 && len == 0x3 {
                    Ok(xml_payload)
                } else {
                    Err(XmlError::Transport("unexpected read".into()))
                }
            }
        })
        .await
        .expect("load xml");
        assert_eq!(loaded, "<a/>");
    }
}
