//! Load and pre-parse GenICam XML using quick-xml.

use std::future::Future;

use quick_xml::events::{BytesStart, Event};
use quick_xml::name::QName;
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

/// Access privileges for a GenICam node as described in the XML.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessMode {
    /// Read-only node. The underlying register must not be modified by the client.
    RO,
    /// Write-only node. Reading the register is not permitted.
    WO,
    /// Read-write node. The register may be read and written by the client.
    RW,
}

impl AccessMode {
    fn parse(value: &str) -> Result<Self, XmlError> {
        match value.trim().to_ascii_uppercase().as_str() {
            "RO" => Ok(AccessMode::RO),
            "WO" => Ok(AccessMode::WO),
            "RW" => Ok(AccessMode::RW),
            other => Err(XmlError::Invalid(format!("unknown access mode: {other}"))),
        }
    }
}

/// Declaration of a node extracted from the GenICam XML description.
#[derive(Debug, Clone)]
pub enum NodeDecl {
    /// Integer feature backed by a fixed register block.
    Integer {
        /// Feature name.
        name: String,
        /// Absolute register address.
        address: u64,
        /// Register length in bytes.
        len: u32,
        /// Access privileges.
        access: AccessMode,
        /// Minimum allowed user value.
        min: i64,
        /// Maximum allowed user value.
        max: i64,
        /// Optional increment step enforced by the device.
        inc: Option<i64>,
        /// Engineering unit (if provided).
        unit: Option<String>,
        /// Selector nodes referencing this feature.
        selectors: Vec<String>,
        /// Selector gating rules in the form (selector name, allowed values).
        selected_if: Vec<(String, Vec<String>)>,
    },
    /// Floating point feature backed by an integer register with scaling.
    Float {
        name: String,
        address: u64,
        len: u32,
        access: AccessMode,
        min: f64,
        max: f64,
        unit: Option<String>,
        /// Optional rational scale applied to the raw register value.
        scale: Option<(i64, i64)>,
        /// Optional additive offset applied after scaling.
        offset: Option<f64>,
        selectors: Vec<String>,
        selected_if: Vec<(String, Vec<String>)>,
    },
    /// Enumeration feature exposing a list of named integer values.
    Enum {
        name: String,
        address: u64,
        len: u32,
        access: AccessMode,
        entries: Vec<(String, i64)>,
        default: Option<String>,
        selectors: Vec<String>,
        selected_if: Vec<(String, Vec<String>)>,
    },
    /// Boolean feature backed by a single bit/byte register.
    Boolean {
        name: String,
        address: u64,
        len: u32,
        access: AccessMode,
        selectors: Vec<String>,
        selected_if: Vec<(String, Vec<String>)>,
    },
    /// Command feature that triggers an action when written.
    Command {
        name: String,
        address: u64,
        len: u32,
    },
    /// Category used to organise features.
    Category { name: String, children: Vec<String> },
}

/// Full XML model describing the GenICam schema version and all declared nodes.
#[derive(Debug, Clone)]
pub struct XmlModel {
    /// Combined schema version extracted from the RegisterDescription attributes.
    pub version: String,
    /// Flat list of node declarations present in the document.
    pub nodes: Vec<NodeDecl>,
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

/// Parse a GenICam XML document into an [`XmlModel`].
///
/// The parser only understands a practical subset of the schema. Unknown tags
/// are skipped which keeps the implementation forward compatible with richer
/// documents.
pub fn parse(xml: &str) -> Result<XmlModel, XmlError> {
    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);
    let mut buf = Vec::new();
    let mut version = String::from("0.0.0");
    let mut nodes = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => match e.name().as_ref() {
                b"RegisterDescription" => {
                    version = schema_version_from(e)?;
                }
                b"Integer" => {
                    let node = parse_integer(&mut reader, e.clone())?;
                    nodes.push(node);
                }
                b"Float" => {
                    let node = parse_float(&mut reader, e.clone())?;
                    nodes.push(node);
                }
                b"Enumeration" => {
                    let node = parse_enum(&mut reader, e.clone())?;
                    nodes.push(node);
                }
                b"Boolean" => {
                    let node = parse_boolean(&mut reader, e.clone())?;
                    nodes.push(node);
                }
                b"Command" => {
                    let node = parse_command(&mut reader, e.clone())?;
                    nodes.push(node);
                }
                b"Category" => {
                    let node = parse_category(&mut reader, e.clone())?;
                    nodes.push(node);
                }
                _ => {
                    skip_element(&mut reader, e.name().as_ref())?;
                }
            },
            Ok(Event::Empty(ref e)) => match e.name().as_ref() {
                b"RegisterDescription" => {
                    version = schema_version_from(e)?;
                }
                b"Command" => {
                    let node = parse_command_empty(e)?;
                    nodes.push(node);
                }
                b"Category" => {
                    let node = parse_category_empty(e)?;
                    nodes.push(node);
                }
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(err) => return Err(XmlError::Xml(err.to_string())),
            _ => {}
        }
        buf.clear();
    }

    Ok(XmlModel { version, nodes })
}

fn schema_version_from(event: &BytesStart<'_>) -> Result<String, XmlError> {
    let major = attribute_value(event, b"SchemaMajorVersion")?;
    let minor = attribute_value(event, b"SchemaMinorVersion")?;
    let sub = attribute_value(event, b"SchemaSubMinorVersion")?;
    let major = major.unwrap_or_else(|| "0".to_string());
    let minor = minor.unwrap_or_else(|| "0".to_string());
    let sub = sub.unwrap_or_else(|| "0".to_string());
    Ok(format!("{major}.{minor}.{sub}"))
}

fn parse_integer(reader: &mut Reader<&[u8]>, start: BytesStart<'_>) -> Result<NodeDecl, XmlError> {
    let name = attribute_value_required(&start, b"Name")?;
    let mut address = None;
    let mut length = None;
    let mut access = AccessMode::RW;
    let mut min = None;
    let mut max = None;
    let mut inc = None;
    let mut unit = None;
    let mut selectors = Vec::new();
    let mut selected_if: Vec<(String, Vec<String>)> = Vec::new();
    let mut last_selector: Option<usize> = None;
    let node_name = start.name().as_ref().to_vec();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => match e.name().as_ref() {
                b"Address" => {
                    let text = read_text_start(reader, e)?;
                    address = Some(parse_u64(&text)?);
                }
                b"Length" => {
                    let text = read_text_start(reader, e)?;
                    let value = parse_u64(&text)?;
                    length = Some(u32::try_from(value).map_err(|_| {
                        XmlError::Invalid(format!("length out of range for node {name}"))
                    })?);
                }
                b"AccessMode" => {
                    let text = read_text_start(reader, e)?;
                    access = AccessMode::parse(&text)?;
                }
                b"Min" => {
                    let text = read_text_start(reader, e)?;
                    min = Some(parse_i64(&text)?);
                }
                b"Max" => {
                    let text = read_text_start(reader, e)?;
                    max = Some(parse_i64(&text)?);
                }
                b"Inc" => {
                    let text = read_text_start(reader, e)?;
                    inc = Some(parse_i64(&text)?);
                }
                b"Unit" => {
                    let text = read_text_start(reader, e)?;
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        unit = Some(trimmed.to_string());
                    }
                }
                b"pSelected" => {
                    let text = read_text_start(reader, e)?;
                    let selector = text.trim().to_string();
                    if !selector.is_empty() {
                        selectors.push(selector.clone());
                        selected_if.push((selector.clone(), Vec::new()));
                        last_selector = Some(selected_if.len() - 1);
                    }
                }
                b"Selected" => {
                    let text = read_text_start(reader, e)?;
                    if let Some(idx) = last_selector {
                        let value = text.trim();
                        if !value.is_empty() {
                            selected_if[idx].1.push(value.to_string());
                        }
                    }
                }
                _ => skip_element(reader, e.name().as_ref())?,
            },
            Ok(Event::Empty(ref e)) => match e.name().as_ref() {
                b"pSelected" => {
                    if let Some(value) = attribute_value(e, b"Name")? {
                        selectors.push(value.clone());
                        selected_if.push((value, Vec::new()));
                        last_selector = Some(selected_if.len() - 1);
                    }
                }
                b"Selected" => {
                    if let Some(idx) = last_selector {
                        if let Some(value) = attribute_value(e, b"Value")? {
                            selected_if[idx].1.push(value);
                        }
                    }
                }
                _ => {}
            },
            Ok(Event::End(ref e)) if e.name().as_ref() == node_name.as_slice() => break,
            Ok(Event::Eof) => {
                return Err(XmlError::Invalid(format!(
                    "unterminated Integer node {name}"
                )))
            }
            Err(err) => return Err(XmlError::Xml(err.to_string())),
            _ => {}
        }
        buf.clear();
    }

    let address = address
        .ok_or_else(|| XmlError::Invalid(format!("Integer node {name} is missing <Address>")))?;
    let length = length
        .ok_or_else(|| XmlError::Invalid(format!("Integer node {name} is missing <Length>")))?;
    let min =
        min.ok_or_else(|| XmlError::Invalid(format!("Integer node {name} is missing <Min>")))?;
    let max =
        max.ok_or_else(|| XmlError::Invalid(format!("Integer node {name} is missing <Max>")))?;

    Ok(NodeDecl::Integer {
        name,
        address,
        len: length,
        access,
        min,
        max,
        inc,
        unit,
        selectors,
        selected_if,
    })
}

fn parse_float(reader: &mut Reader<&[u8]>, start: BytesStart<'_>) -> Result<NodeDecl, XmlError> {
    let name = attribute_value_required(&start, b"Name")?;
    let mut address = None;
    let mut length = None;
    let mut access = AccessMode::RW;
    let mut min = None;
    let mut max = None;
    let mut unit = None;
    let mut scale_num: Option<i64> = None;
    let mut scale_den: Option<i64> = None;
    let mut offset = None;
    let mut selectors = Vec::new();
    let mut selected_if = Vec::new();
    let mut last_selector = None;
    let node_name = start.name().as_ref().to_vec();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => match e.name().as_ref() {
                b"Address" => {
                    let text = read_text_start(reader, e)?;
                    address = Some(parse_u64(&text)?);
                }
                b"Length" => {
                    let text = read_text_start(reader, e)?;
                    let value = parse_u64(&text)?;
                    length = Some(u32::try_from(value).map_err(|_| {
                        XmlError::Invalid(format!("length out of range for node {name}"))
                    })?);
                }
                b"AccessMode" => {
                    let text = read_text_start(reader, e)?;
                    access = AccessMode::parse(&text)?;
                }
                b"Min" => {
                    let text = read_text_start(reader, e)?;
                    min = Some(parse_f64(&text)?);
                }
                b"Max" => {
                    let text = read_text_start(reader, e)?;
                    max = Some(parse_f64(&text)?);
                }
                b"Unit" => {
                    let text = read_text_start(reader, e)?;
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        unit = Some(trimmed.to_string());
                    }
                }
                b"Scale" => {
                    let text = read_text_start(reader, e)?;
                    let (num, den) = parse_scale(&text)?;
                    scale_num = Some(num);
                    scale_den = Some(den);
                }
                b"ScaleNumerator" => {
                    let text = read_text_start(reader, e)?;
                    scale_num = Some(parse_i64(&text)?);
                }
                b"ScaleDenominator" => {
                    let text = read_text_start(reader, e)?;
                    scale_den = Some(parse_i64(&text)?);
                }
                b"Offset" => {
                    let text = read_text_start(reader, e)?;
                    offset = Some(parse_f64(&text)?);
                }
                b"pSelected" => {
                    let text = read_text_start(reader, e)?;
                    let selector = text.trim().to_string();
                    if !selector.is_empty() {
                        selectors.push(selector.clone());
                        selected_if.push((selector.clone(), Vec::new()));
                        last_selector = Some(selected_if.len() - 1);
                    }
                }
                b"Selected" => {
                    let text = read_text_start(reader, e)?;
                    if let Some(idx) = last_selector {
                        let value = text.trim();
                        if !value.is_empty() {
                            selected_if[idx].1.push(value.to_string());
                        }
                    }
                }
                _ => skip_element(reader, e.name().as_ref())?,
            },
            Ok(Event::Empty(ref e)) => match e.name().as_ref() {
                b"pSelected" => {
                    if let Some(value) = attribute_value(e, b"Name")? {
                        selectors.push(value.clone());
                        selected_if.push((value, Vec::new()));
                        last_selector = Some(selected_if.len() - 1);
                    }
                }
                b"Selected" => {
                    if let Some(idx) = last_selector {
                        if let Some(value) = attribute_value(e, b"Value")? {
                            selected_if[idx].1.push(value);
                        }
                    }
                }
                _ => {}
            },
            Ok(Event::End(ref e)) if e.name().as_ref() == node_name.as_slice() => break,
            Ok(Event::Eof) => {
                return Err(XmlError::Invalid(format!("unterminated Float node {name}")))
            }
            Err(err) => return Err(XmlError::Xml(err.to_string())),
            _ => {}
        }
        buf.clear();
    }

    let address = address
        .ok_or_else(|| XmlError::Invalid(format!("Float node {name} is missing <Address>")))?;
    let length = length
        .ok_or_else(|| XmlError::Invalid(format!("Float node {name} is missing <Length>")))?;
    let min =
        min.ok_or_else(|| XmlError::Invalid(format!("Float node {name} is missing <Min>")))?;
    let max =
        max.ok_or_else(|| XmlError::Invalid(format!("Float node {name} is missing <Max>")))?;
    let scale = match (scale_num, scale_den) {
        (Some(num), Some(den)) if den != 0 => Some((num, den)),
        (None, None) => None,
        (Some(num), None) => Some((num, 1)),
        _ => None,
    };

    Ok(NodeDecl::Float {
        name,
        address,
        len: length,
        access,
        min,
        max,
        unit,
        scale,
        offset,
        selectors,
        selected_if,
    })
}

fn parse_enum(reader: &mut Reader<&[u8]>, start: BytesStart<'_>) -> Result<NodeDecl, XmlError> {
    let name = attribute_value_required(&start, b"Name")?;
    let mut address = None;
    let mut length = None;
    let mut access = AccessMode::RW;
    let mut entries = Vec::new();
    let mut default = None;
    let mut selectors = Vec::new();
    let mut selected_if = Vec::new();
    let mut last_selector = None;
    let node_name = start.name().as_ref().to_vec();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => match e.name().as_ref() {
                b"Address" => {
                    let text = read_text_start(reader, e)?;
                    address = Some(parse_u64(&text)?);
                }
                b"Length" => {
                    let text = read_text_start(reader, e)?;
                    let value = parse_u64(&text)?;
                    length = Some(u32::try_from(value).map_err(|_| {
                        XmlError::Invalid(format!("length out of range for node {name}"))
                    })?);
                }
                b"AccessMode" => {
                    let text = read_text_start(reader, e)?;
                    access = AccessMode::parse(&text)?;
                }
                b"EnumEntry" => {
                    let entry = parse_enum_entry(reader, e.clone())?;
                    entries.push(entry);
                }
                b"pSelected" => {
                    let text = read_text_start(reader, e)?;
                    let selector = text.trim().to_string();
                    if !selector.is_empty() {
                        selectors.push(selector.clone());
                        selected_if.push((selector.clone(), Vec::new()));
                        last_selector = Some(selected_if.len() - 1);
                    }
                }
                b"Selected" => {
                    let text = read_text_start(reader, e)?;
                    if let Some(idx) = last_selector {
                        let value = text.trim();
                        if !value.is_empty() {
                            selected_if[idx].1.push(value.to_string());
                        }
                    }
                }
                b"pValueDefault" => {
                    let text = read_text_start(reader, e)?;
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        default = Some(trimmed.to_string());
                    }
                }
                _ => skip_element(reader, e.name().as_ref())?,
            },
            Ok(Event::Empty(ref e)) => match e.name().as_ref() {
                b"EnumEntry" => {
                    let entry = parse_enum_entry_empty(e)?;
                    entries.push(entry);
                }
                b"pSelected" => {
                    if let Some(value) = attribute_value(e, b"Name")? {
                        selectors.push(value.clone());
                        selected_if.push((value, Vec::new()));
                        last_selector = Some(selected_if.len() - 1);
                    }
                }
                b"Selected" => {
                    if let Some(idx) = last_selector {
                        if let Some(value) = attribute_value(e, b"Value")? {
                            selected_if[idx].1.push(value);
                        }
                    }
                }
                _ => {}
            },
            Ok(Event::End(ref e)) if e.name().as_ref() == node_name.as_slice() => break,
            Ok(Event::Eof) => {
                return Err(XmlError::Invalid(format!(
                    "unterminated Enumeration node {name}"
                )))
            }
            Err(err) => return Err(XmlError::Xml(err.to_string())),
            _ => {}
        }
        buf.clear();
    }

    let address = address.ok_or_else(|| {
        XmlError::Invalid(format!("Enumeration node {name} is missing <Address>"))
    })?;
    let length = length
        .ok_or_else(|| XmlError::Invalid(format!("Enumeration node {name} is missing <Length>")))?;

    Ok(NodeDecl::Enum {
        name,
        address,
        len: length,
        access,
        entries,
        default,
        selectors,
        selected_if,
    })
}

fn parse_boolean(reader: &mut Reader<&[u8]>, start: BytesStart<'_>) -> Result<NodeDecl, XmlError> {
    let name = attribute_value_required(&start, b"Name")?;
    let mut address = None;
    let mut length = None;
    let mut access = AccessMode::RW;
    let mut selectors = Vec::new();
    let mut selected_if = Vec::new();
    let mut last_selector = None;
    let node_name = start.name().as_ref().to_vec();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => match e.name().as_ref() {
                b"Address" => {
                    let text = read_text_start(reader, e)?;
                    address = Some(parse_u64(&text)?);
                }
                b"Length" => {
                    let text = read_text_start(reader, e)?;
                    let value = parse_u64(&text)?;
                    length = Some(u32::try_from(value).map_err(|_| {
                        XmlError::Invalid(format!("length out of range for node {name}"))
                    })?);
                }
                b"AccessMode" => {
                    let text = read_text_start(reader, e)?;
                    access = AccessMode::parse(&text)?;
                }
                b"pSelected" => {
                    let text = read_text_start(reader, e)?;
                    let selector = text.trim().to_string();
                    if !selector.is_empty() {
                        selectors.push(selector.clone());
                        selected_if.push((selector.clone(), Vec::new()));
                        last_selector = Some(selected_if.len() - 1);
                    }
                }
                b"Selected" => {
                    let text = read_text_start(reader, e)?;
                    if let Some(idx) = last_selector {
                        let value = text.trim();
                        if !value.is_empty() {
                            selected_if[idx].1.push(value.to_string());
                        }
                    }
                }
                _ => skip_element(reader, e.name().as_ref())?,
            },
            Ok(Event::Empty(ref e)) => match e.name().as_ref() {
                b"pSelected" => {
                    if let Some(value) = attribute_value(e, b"Name")? {
                        selectors.push(value.clone());
                        selected_if.push((value, Vec::new()));
                        last_selector = Some(selected_if.len() - 1);
                    }
                }
                b"Selected" => {
                    if let Some(idx) = last_selector {
                        if let Some(value) = attribute_value(e, b"Value")? {
                            selected_if[idx].1.push(value);
                        }
                    }
                }
                _ => {}
            },
            Ok(Event::End(ref e)) if e.name().as_ref() == node_name.as_slice() => break,
            Ok(Event::Eof) => {
                return Err(XmlError::Invalid(format!(
                    "unterminated Boolean node {name}"
                )))
            }
            Err(err) => return Err(XmlError::Xml(err.to_string())),
            _ => {}
        }
        buf.clear();
    }

    let address = address
        .ok_or_else(|| XmlError::Invalid(format!("Boolean node {name} is missing <Address>")))?;
    let length = length.unwrap_or(1);

    Ok(NodeDecl::Boolean {
        name,
        address,
        len: length,
        access,
        selectors,
        selected_if,
    })
}

fn parse_command(reader: &mut Reader<&[u8]>, start: BytesStart<'_>) -> Result<NodeDecl, XmlError> {
    let name = attribute_value_required(&start, b"Name")?;
    let mut address = None;
    let mut length = None;
    let node_name = start.name().as_ref().to_vec();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => match e.name().as_ref() {
                b"Address" => {
                    let text = read_text_start(reader, e)?;
                    address = Some(parse_u64(&text)?);
                }
                b"Length" => {
                    let text = read_text_start(reader, e)?;
                    let value = parse_u64(&text)?;
                    length = Some(u32::try_from(value).map_err(|_| {
                        XmlError::Invalid(format!("length out of range for node {name}"))
                    })?);
                }
                _ => skip_element(reader, e.name().as_ref())?,
            },
            Ok(Event::End(ref e)) if e.name().as_ref() == node_name.as_slice() => break,
            Ok(Event::Eof) => {
                return Err(XmlError::Invalid(format!(
                    "unterminated Command node {name}"
                )))
            }
            Err(err) => return Err(XmlError::Xml(err.to_string())),
            _ => {}
        }
        buf.clear();
    }

    let address = address
        .ok_or_else(|| XmlError::Invalid(format!("Command node {name} is missing <Address>")))?;
    let length = length.unwrap_or(1);

    Ok(NodeDecl::Command {
        name,
        address,
        len: length,
    })
}

fn parse_command_empty(start: &BytesStart<'_>) -> Result<NodeDecl, XmlError> {
    let name = attribute_value_required(start, b"Name")?;
    let address = attribute_value_required(start, b"Address")?;
    let address = parse_u64(&address)?;
    let length = attribute_value(start, b"Length")?;
    let length = match length {
        Some(value) => {
            let raw = parse_u64(&value)?;
            u32::try_from(raw)
                .map_err(|_| XmlError::Invalid("command length out of range".into()))?
        }
        None => 1,
    };
    Ok(NodeDecl::Command {
        name,
        address,
        len: length,
    })
}

fn parse_category(reader: &mut Reader<&[u8]>, start: BytesStart<'_>) -> Result<NodeDecl, XmlError> {
    let name = attribute_value_required(&start, b"Name")?;
    let node_name = start.name().as_ref().to_vec();
    let mut children = Vec::new();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => match e.name().as_ref() {
                b"pFeature" => {
                    let text = read_text_start(reader, e)?;
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        children.push(trimmed.to_string());
                    }
                }
                _ => skip_element(reader, e.name().as_ref())?,
            },
            Ok(Event::Empty(ref e)) if e.name().as_ref() == b"pFeature" => {
                if let Some(value) = attribute_value(e, b"Name")? {
                    if !value.is_empty() {
                        children.push(value);
                    }
                }
            }
            Ok(Event::End(ref e)) if e.name().as_ref() == node_name.as_slice() => break,
            Ok(Event::Eof) => {
                return Err(XmlError::Invalid(format!(
                    "unterminated Category node {name}"
                )))
            }
            Err(err) => return Err(XmlError::Xml(err.to_string())),
            _ => {}
        }
        buf.clear();
    }

    Ok(NodeDecl::Category { name, children })
}

fn parse_category_empty(start: &BytesStart<'_>) -> Result<NodeDecl, XmlError> {
    let name = attribute_value_required(start, b"Name")?;
    Ok(NodeDecl::Category {
        name,
        children: Vec::new(),
    })
}

fn parse_enum_entry(
    reader: &mut Reader<&[u8]>,
    start: BytesStart<'_>,
) -> Result<(String, i64), XmlError> {
    let mut name = attribute_value_required(&start, b"Name")?;
    let mut value = attribute_value(&start, b"Value")?;
    let node_name = start.name().as_ref().to_vec();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => match e.name().as_ref() {
                b"Value" => {
                    let text = read_text_start(reader, e)?;
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        value = Some(trimmed.to_string());
                    }
                }
                b"Name" => {
                    let text = read_text_start(reader, e)?;
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        name = trimmed.to_string();
                    }
                }
                _ => skip_element(reader, e.name().as_ref())?,
            },
            Ok(Event::End(ref e)) if e.name().as_ref() == node_name.as_slice() => break,
            Ok(Event::Eof) => {
                return Err(XmlError::Invalid("unterminated EnumEntry element".into()))
            }
            Err(err) => return Err(XmlError::Xml(err.to_string())),
            _ => {}
        }
        buf.clear();
    }

    let value = value.ok_or_else(|| XmlError::Invalid("EnumEntry missing Value".into()))?;
    let value = parse_i64(&value)?;
    Ok((name, value))
}

fn parse_enum_entry_empty(start: &BytesStart<'_>) -> Result<(String, i64), XmlError> {
    let name = attribute_value_required(start, b"Name")?;
    let value = attribute_value_required(start, b"Value")?;
    let value = parse_i64(&value)?;
    Ok((name, value))
}

fn parse_scale(text: &str) -> Result<(i64, i64), XmlError> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(XmlError::Invalid("empty scale value".into()));
    }
    if let Some((num, den)) = trimmed.split_once('/') {
        let num = parse_i64(num)?;
        let den = parse_i64(den)?;
        if den == 0 {
            return Err(XmlError::Invalid("scale denominator is zero".into()));
        }
        Ok((num, den))
    } else {
        let value = parse_f64(trimmed)?;
        if value == 0.0 {
            return Err(XmlError::Invalid("scale value is zero".into()));
        }
        // Approximate decimal scale as a rational using denominator 1_000_000.
        let den = 1_000_000i64;
        let num = (value * den as f64).round() as i64;
        Ok((num, den))
    }
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

fn read_text_start(reader: &mut Reader<&[u8]>, start: &BytesStart<'_>) -> Result<String, XmlError> {
    let end_buf = start.name().as_ref().to_vec();
    reader
        .read_text(QName(&end_buf))
        .map(|cow| cow.into_owned())
        .map_err(|err| XmlError::Xml(err.to_string()))
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

fn attribute_value_required(event: &BytesStart<'_>, name: &[u8]) -> Result<String, XmlError> {
    attribute_value(event, name)?.ok_or_else(|| {
        XmlError::Invalid(format!(
            "missing attribute {}",
            String::from_utf8_lossy(name)
        ))
    })
}

fn parse_u64(value: &str) -> Result<u64, XmlError> {
    let trimmed = value.trim();
    if let Some(hex) = trimmed.strip_prefix("0x") {
        let hex = hex.replace('_', "");
        u64::from_str_radix(&hex, 16)
            .map_err(|err| XmlError::Invalid(format!("invalid hex value: {err}")))
    } else {
        let dec = trimmed.replace('_', "");
        dec.parse()
            .map_err(|err| XmlError::Invalid(format!("invalid integer: {err}")))
    }
}

fn parse_i64(value: &str) -> Result<i64, XmlError> {
    let trimmed = value.trim();
    if let Some(hex) = trimmed.strip_prefix("0x") {
        let hex = hex.replace('_', "");
        i64::from_str_radix(&hex, 16)
            .map_err(|err| XmlError::Invalid(format!("invalid hex value: {err}")))
    } else {
        let dec = trimmed.replace('_', "");
        dec.parse()
            .map_err(|err| XmlError::Invalid(format!("invalid integer: {err}")))
    }
}

fn parse_f64(value: &str) -> Result<f64, XmlError> {
    value
        .trim()
        .parse()
        .map_err(|err| XmlError::Invalid(format!("invalid float: {err}")))
}

fn skip_element(reader: &mut Reader<&[u8]>, name: &[u8]) -> Result<(), XmlError> {
    let mut depth = 1usize;
    let mut buf = Vec::new();
    while depth > 0 {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(_)) => depth += 1,
            Ok(Event::End(ref e)) => {
                if e.name().as_ref() == name {
                    depth -= 1;
                }
            }
            Ok(Event::Eof) => {
                return Err(XmlError::Invalid("unexpected end of file".into()));
            }
            Err(err) => return Err(XmlError::Xml(err.to_string())),
            _ => {}
        }
        buf.clear();
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinimalXmlInfo {
    pub schema_version: Option<String>,
    pub top_level_features: Vec<String>,
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
                    address = Some(parse_u64(value)?);
                }
                "length" | "size" => {
                    let len = parse_u64(value)?;
                    length = Some(
                        len.try_into()
                            .map_err(|_| XmlError::Invalid("length does not fit usize".into()))?,
                    );
                }
                _ => {}
            }
        } else if token.starts_with("0x") {
            address = Some(parse_u64(token)?);
        } else {
            return Ok(UrlLocation::LocalNamed(token.to_string()));
        }
    }
    match (address, length) {
        (Some(address), Some(length)) => Ok(UrlLocation::Local { address, length }),
        _ => Err(XmlError::Invalid(format!("unsupported local URL: {rest}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"
        <RegisterDescription SchemaMajorVersion="1" SchemaMinorVersion="2" SchemaSubMinorVersion="3">
            <Category Name="Root">
                <pFeature>Gain</pFeature>
                <pFeature>GainSelector</pFeature>
            </Category>
            <Integer Name="Width">
                <Address>0x0000_0100</Address>
                <Length>4</Length>
                <AccessMode>RW</AccessMode>
                <Min>16</Min>
                <Max>4096</Max>
                <Inc>2</Inc>
            </Integer>
            <Float Name="ExposureTime">
                <Address>0x0000_0200</Address>
                <Length>4</Length>
                <AccessMode>RW</AccessMode>
                <Min>10.0</Min>
                <Max>200000.0</Max>
                <Scale>1/1000</Scale>
                <Offset>0.0</Offset>
            </Float>
            <Enumeration Name="GainSelector">
                <Address>0x0000_0300</Address>
                <Length>2</Length>
                <AccessMode>RW</AccessMode>
                <EnumEntry Name="AnalogAll" Value="0" />
                <EnumEntry Name="DigitalAll" Value="1" />
            </Enumeration>
            <Integer Name="Gain">
                <Address>0x0000_0304</Address>
                <Length>2</Length>
                <AccessMode>RW</AccessMode>
                <Min>0</Min>
                <Max>48</Max>
                <pSelected>GainSelector</pSelected>
                <Selected>AnalogAll</Selected>
            </Integer>
            <Boolean Name="GammaEnable">
                <Address>0x0000_0400</Address>
                <Length>1</Length>
                <AccessMode>RW</AccessMode>
            </Boolean>
            <Command Name="AcquisitionStart">
                <Address>0x0000_0500</Address>
                <Length>4</Length>
            </Command>
        </RegisterDescription>
    "#;

    #[tokio::test]
    async fn parse_minimal_xml() {
        let info = parse_into_minimal_nodes(FIXTURE).expect("parse xml");
        assert_eq!(info.schema_version.as_deref(), Some("1.2.3"));
        assert_eq!(info.top_level_features.len(), 7);
        assert_eq!(info.top_level_features[0], "Root");

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

    #[test]
    fn parse_fixture_model() {
        let model = parse(FIXTURE).expect("parse fixture");
        assert_eq!(model.version, "1.2.3");
        assert_eq!(model.nodes.len(), 7);
        match &model.nodes[0] {
            NodeDecl::Category { name, children } => {
                assert_eq!(name, "Root");
                assert_eq!(
                    children,
                    &vec!["Gain".to_string(), "GainSelector".to_string()]
                );
            }
            other => panic!("unexpected node: {other:?}"),
        }
        match &model.nodes[1] {
            NodeDecl::Integer {
                name,
                min,
                max,
                inc,
                ..
            } => {
                assert_eq!(name, "Width");
                assert_eq!(*min, 16);
                assert_eq!(*max, 4096);
                assert_eq!(*inc, Some(2));
            }
            other => panic!("unexpected node: {other:?}"),
        }
        match &model.nodes[2] {
            NodeDecl::Float {
                name,
                scale,
                offset,
                ..
            } => {
                assert_eq!(name, "ExposureTime");
                assert_eq!(*scale, Some((1, 1000)));
                assert_eq!(*offset, Some(0.0));
            }
            other => panic!("unexpected node: {other:?}"),
        }
        match &model.nodes[3] {
            NodeDecl::Enum { name, entries, .. } => {
                assert_eq!(name, "GainSelector");
                assert_eq!(entries.len(), 2);
            }
            other => panic!("unexpected node: {other:?}"),
        }
        match &model.nodes[4] {
            NodeDecl::Integer {
                name, selected_if, ..
            } => {
                assert_eq!(name, "Gain");
                assert_eq!(selected_if.len(), 1);
                assert_eq!(selected_if[0].0, "GainSelector");
                assert_eq!(selected_if[0].1, vec!["AnalogAll".to_string()]);
            }
            other => panic!("unexpected node: {other:?}"),
        }
    }
}
