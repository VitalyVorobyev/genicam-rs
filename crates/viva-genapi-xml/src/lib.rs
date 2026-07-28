#![cfg_attr(docsrs, feature(doc_cfg))]
//! Load and pre-parse GenICam XML using quick-xml.
//!
//! This crate provides types and functions for parsing GenICam XML descriptions
//! into a structured representation that can be used by the core evaluation engine.

mod builders;
#[cfg(feature = "fetch")]
mod fetch;
mod parsers;
mod util;

#[cfg(feature = "fetch")]
pub use fetch::fetch_and_load_xml;

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::QName;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use parsers::{
    parse_boolean, parse_category, parse_category_empty, parse_command, parse_command_empty,
    parse_converter, parse_enum, parse_float, parse_int_converter, parse_integer, parse_string,
    parse_struct_reg, parse_swissknife,
};
use util::{attribute_value, skip_element};

/// Source of the numeric value backing an enumeration entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EnumValueSrc {
    /// Numeric literal declared directly in the XML.
    Literal(i64),
    /// Value obtained from another node referenced via `<pValue>`.
    FromNode(String),
}

/// References to predicate provider nodes used for runtime gating.
///
/// GenICam's `pIsImplemented`, `pIsAvailable` and `pIsLocked` each point at
/// another node (typically an Integer, Boolean or IntSwissKnife) whose current
/// value gates whether a feature is implemented, accessible, or writable.
/// These three references are shared by most node variants, so we collect them
/// into one struct to keep the variant fields small.
///
/// All three fields are optional; a node with no predicates has
/// [`PredicateRefs::default()`]. Serde fields use the GenICam XML spelling so
/// round-trip JSON matches the XML attribute names.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PredicateRefs {
    /// Name of a node evaluating to non-zero iff the feature is implemented.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "pIsImplemented"
    )]
    pub p_is_implemented: Option<String>,
    /// Name of a node evaluating to non-zero iff the feature is accessible now.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "pIsAvailable"
    )]
    pub p_is_available: Option<String>,
    /// Name of a node evaluating to non-zero iff the feature is locked (RW→RO).
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "pIsLocked")]
    pub p_is_locked: Option<String>,
}

impl PredicateRefs {
    /// Iterate over all referenced node names (for dependency-graph walks).
    pub fn references(&self) -> impl Iterator<Item = &str> {
        [
            self.p_is_implemented.as_deref(),
            self.p_is_available.as_deref(),
            self.p_is_locked.as_deref(),
        ]
        .into_iter()
        .flatten()
    }

    /// `true` when every field is `None`.
    pub fn is_empty(&self) -> bool {
        self.p_is_implemented.is_none()
            && self.p_is_available.is_none()
            && self.p_is_locked.is_none()
    }
}

/// Declaration for a single enumeration entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnumEntryDecl {
    /// Symbolic entry name exposed to clients.
    pub name: String,
    /// Source describing how to resolve the numeric value for this entry.
    pub value: EnumValueSrc,
    /// Optional user facing label.
    pub display_name: Option<String>,
    /// Predicate refs controlling whether this entry is implemented / available.
    #[serde(default, skip_serializing_if = "PredicateRefs::is_empty")]
    pub predicates: PredicateRefs,
}

#[derive(Debug, Error)]
#[non_exhaustive]
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

/// Visibility level controlling which users see a feature.
///
/// GenICam defines four levels; features at a given level are visible to
/// users at that level and above.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[non_exhaustive]
pub enum Visibility {
    /// Shown to all users (default).
    #[default]
    Beginner,
    /// Shown to experienced users.
    Expert,
    /// Shown only to advanced integrators.
    Guru,
    /// Hidden from all UI presentations.
    Invisible,
}

impl Visibility {
    pub(crate) fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "Beginner" => Some(Self::Beginner),
            "Expert" => Some(Self::Expert),
            "Guru" => Some(Self::Guru),
            "Invisible" => Some(Self::Invisible),
            _ => None,
        }
    }
}

/// Recommended UI representation for a numeric feature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Representation {
    Linear,
    Logarithmic,
    Boolean,
    PureNumber,
    HexNumber,
    /// Display as dotted-quad IPv4 address.
    IPV4Address,
    /// Display as colon-separated MAC address.
    MACAddress,
}

impl Representation {
    pub(crate) fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "Linear" => Some(Self::Linear),
            "Logarithmic" => Some(Self::Logarithmic),
            "Boolean" => Some(Self::Boolean),
            "PureNumber" => Some(Self::PureNumber),
            "HexNumber" => Some(Self::HexNumber),
            "IPV4Address" => Some(Self::IPV4Address),
            "MACAddress" => Some(Self::MACAddress),
            _ => None,
        }
    }
}

/// Shared metadata present on every GenICam node.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeMeta {
    /// Visibility level (Beginner, Expert, Guru, Invisible).
    pub visibility: Visibility,
    /// Long-form description of the feature.
    pub description: Option<String>,
    /// Short tooltip text for UI hover hints.
    pub tooltip: Option<String>,
    /// Human-readable label (may differ from the node name).
    pub display_name: Option<String>,
    /// Recommended UI representation for numeric features.
    pub representation: Option<Representation>,
}

/// Access privileges for a GenICam node as described in the XML.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccessMode {
    /// Read-only node. The underlying register must not be modified by the client.
    RO,
    /// Write-only node. Reading the register is not permitted.
    WO,
    /// Read-write node. The register may be read and written by the client.
    RW,
}

impl AccessMode {
    /// Parse an `<AccessMode>` value.
    ///
    /// Beyond the three spellings the standard defines, the single-letter forms
    /// `R` / `W` are accepted: they appear in third-party GenICam documents
    /// (including the standard's own conformance fixtures). An unrecognized
    /// value falls back to `RW` — the same default as an absent `<AccessMode>` —
    /// so one odd register cannot make a whole camera unusable. The runtime
    /// still surfaces the device's own error if the access is genuinely denied.
    pub(crate) fn parse(value: &str) -> Result<Self, XmlError> {
        let trimmed = value.trim();
        match trimmed.to_ascii_uppercase().as_str() {
            "RO" | "R" => Ok(AccessMode::RO),
            "WO" | "W" => Ok(AccessMode::WO),
            "RW" | "WR" => Ok(AccessMode::RW),
            other => {
                tracing::warn!(
                    access_mode = other,
                    "unknown <AccessMode> value, assuming RW"
                );
                Ok(AccessMode::RW)
            }
        }
    }
}

/// Register addressing metadata for a node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Addressing {
    /// Node uses a fixed register block regardless of selector state.
    Fixed { address: u64, len: u32 },
    /// Node switches between register blocks based on a selector value.
    BySelector {
        /// Name of the selector node controlling the address.
        selector: String,
        /// Mapping of selector value to `(address, length)` pair.
        map: Vec<(String, (u64, u32))>,
    },
    /// Node resolves its register block through another node providing the address.
    Indirect {
        /// Node providing the register address at runtime.
        p_address_node: String,
        /// Length of the target register block in bytes.
        len: u32,
    },
}

/// Byte order used to interpret a multi-byte register payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ByteOrder {
    /// The first byte contains the least significant bits.
    Little,
    /// The first byte contains the most significant bits.
    Big,
}

impl ByteOrder {
    pub(crate) fn parse(tag: &str) -> Option<Self> {
        match tag.trim().to_ascii_lowercase().as_str() {
            "littleendian" => Some(ByteOrder::Little),
            "bigendian" => Some(ByteOrder::Big),
            _ => None,
        }
    }
}

fn default_big_endian() -> ByteOrder {
    ByteOrder::Big
}

/// Bitfield metadata describing a sub-range of a register payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BitField {
    /// Starting bit offset within the interpreted register value.
    pub bit_offset: u16,
    /// Number of bits covered by the field.
    pub bit_length: u16,
    /// Byte order used when interpreting the enclosing register.
    pub byte_order: ByteOrder,
}

/// Byte-level encoding of the payload behind a `<Float>` / `<FloatReg>` node.
///
/// GenICam's XSD lets a float feature be backed by either:
///
/// - a native IEEE 754 register — a `<FloatReg>` element, or a `<Float>` whose
///   addressing reads exactly 4 or 8 bytes and carries no `<Scale>`/`<Offset>`;
/// - a scaled integer register — a `<Float>` that declares `<Scale>` and/or
///   `<Offset>` and reads the register bytes as a signed integer.
///
/// Prior to this field, `get_float`/`set_float` always used the scaled-integer
/// codec. That returned the bit pattern of the IEEE 754 value decoded as i64
/// for fields such as `AcquisitionFrameRate` (e.g. `1106247680` for 30.0) —
/// see `doc/2026-04-12-genapi-numeric-type-dispatch.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FloatEncoding {
    /// Register bytes are an IEEE 754 value (4 bytes → f32, 8 bytes → f64).
    Ieee754,
    /// Register bytes are a signed integer; `Scale` and `Offset` map to the
    /// user-facing value.
    #[default]
    ScaledInteger,
}

/// Output type of a SwissKnife expression node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SkOutput {
    /// Integer output. The runtime rounds the computed value to the nearest
    /// integer with ties going towards zero.
    Integer,
    /// Floating point output. The runtime exposes the value as a `f64` without
    /// any additional processing.
    #[default]
    Float,
}

impl SkOutput {
    pub(crate) fn parse(tag: &str) -> Option<Self> {
        match tag.trim().to_ascii_lowercase().as_str() {
            "integer" => Some(SkOutput::Integer),
            "float" => Some(SkOutput::Float),
            _ => None,
        }
    }
}

/// Declaration of a SwissKnife node consisting of an arithmetic expression.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwissKnifeDecl {
    /// Feature name exposed to clients.
    pub name: String,
    /// Shared metadata.
    pub meta: NodeMeta,
    /// Raw expression string to be parsed by the runtime.
    pub expr: String,
    /// Mapping of variables used in the expression to provider node names.
    pub variables: Vec<(String, String)>,
    /// Desired output type (integer or float).
    pub output: SkOutput,
    /// Predicate refs gating implementation / availability.
    #[serde(default, skip_serializing_if = "PredicateRefs::is_empty")]
    pub predicates: PredicateRefs,
}

/// Declaration of a Converter node for bidirectional value transformation.
///
/// Converters expose a floating-point value computed from an underlying
/// register or node via a formula.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConverterDecl {
    /// Feature name exposed to clients.
    pub name: String,
    /// Shared metadata.
    pub meta: NodeMeta,
    /// Name of the node providing the raw register value.
    pub p_value: String,
    /// Expression converting raw register value to user-facing value (FROM direction).
    pub formula_to: String,
    /// Expression converting user-facing value back to raw register value (TO direction).
    pub formula_from: String,
    /// Mapping of expression variables to provider node names for `formula_to`.
    pub variables_to: Vec<(String, String)>,
    /// Mapping of expression variables to provider node names for `formula_from`.
    pub variables_from: Vec<(String, String)>,
    /// Engineering unit (if provided).
    pub unit: Option<String>,
    /// Desired output type.
    pub output: SkOutput,
    /// Predicate refs gating implementation / availability / lock state.
    #[serde(default, skip_serializing_if = "PredicateRefs::is_empty")]
    pub predicates: PredicateRefs,
}

/// Declaration of an IntConverter node for integer-specific bidirectional conversion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntConverterDecl {
    /// Feature name exposed to clients.
    pub name: String,
    /// Shared metadata.
    pub meta: NodeMeta,
    /// Name of the node providing the raw register value.
    pub p_value: String,
    /// Expression converting raw register value to user-facing value (FROM direction).
    pub formula_to: String,
    /// Expression converting user-facing value back to raw register value (TO direction).
    pub formula_from: String,
    /// Mapping of expression variables to provider node names for `formula_to`.
    pub variables_to: Vec<(String, String)>,
    /// Mapping of expression variables to provider node names for `formula_from`.
    pub variables_from: Vec<(String, String)>,
    /// Engineering unit (if provided).
    pub unit: Option<String>,
    /// Predicate refs gating implementation / availability / lock state.
    #[serde(default, skip_serializing_if = "PredicateRefs::is_empty")]
    pub predicates: PredicateRefs,
}

/// Declaration of a StringReg node for string-typed register access.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StringDecl {
    /// Feature name exposed to clients.
    pub name: String,
    /// Shared metadata.
    pub meta: NodeMeta,
    /// Addressing metadata for the register block.
    pub addressing: Addressing,
    /// Access privileges.
    pub access: AccessMode,
    /// Predicate refs gating implementation / availability / lock state.
    #[serde(default, skip_serializing_if = "PredicateRefs::is_empty")]
    pub predicates: PredicateRefs,
}

/// Declaration of a node extracted from the GenICam XML description.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeDecl {
    /// Integer feature backed by a register block or delegated via pValue.
    Integer {
        /// Feature name.
        name: String,
        /// Shared metadata (visibility, description, tooltip, etc.).
        meta: NodeMeta,
        /// Addressing metadata (absent when delegated via `pvalue`).
        addressing: Option<Addressing>,
        /// Length in bytes of the register payload.
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
        /// Optional bitfield metadata describing the active bit range.
        bitfield: Option<BitField>,
        /// Selector nodes referencing this feature.
        selectors: Vec<String>,
        /// Selector gating rules in the form (selector name, allowed values).
        selected_if: Vec<(String, Vec<String>)>,
        /// Node providing the value (delegates read/write to another node).
        pvalue: Option<String>,
        /// Node providing the dynamic maximum.
        p_max: Option<String>,
        /// Node providing the dynamic minimum.
        p_min: Option<String>,
        /// Static value (for constant integer nodes with `<Value>`).
        value: Option<i64>,
        /// Predicate refs gating implementation / availability / lock state.
        #[serde(default, skip_serializing_if = "PredicateRefs::is_empty")]
        predicates: PredicateRefs,
    },
    /// Floating point feature backed by an integer register with scaling,
    /// a native IEEE 754 register, or delegated via pValue.
    Float {
        name: String,
        meta: NodeMeta,
        /// Addressing metadata (absent when delegated via `pvalue`).
        addressing: Option<Addressing>,
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
        /// Node providing the value (delegates read/write to another node).
        pvalue: Option<String>,
        /// How the register payload should be interpreted — native IEEE 754
        /// or scaled integer. Defaults to [`FloatEncoding::ScaledInteger`] to
        /// preserve existing behaviour for XML that relied on it.
        #[serde(default)]
        encoding: FloatEncoding,
        /// Byte order of the register payload. Defaults to [`ByteOrder::Big`]
        /// (the GenICam default).
        #[serde(default = "default_big_endian")]
        byte_order: ByteOrder,
        /// Predicate refs gating implementation / availability / lock state.
        #[serde(default, skip_serializing_if = "PredicateRefs::is_empty")]
        predicates: PredicateRefs,
    },
    /// Enumeration feature exposing a list of named integer values.
    Enum {
        name: String,
        meta: NodeMeta,
        /// Addressing metadata (absent when delegated via `pvalue`).
        addressing: Option<Addressing>,
        access: AccessMode,
        entries: Vec<EnumEntryDecl>,
        default: Option<String>,
        selectors: Vec<String>,
        selected_if: Vec<(String, Vec<String>)>,
        /// Node providing the integer value (delegates register read/write).
        pvalue: Option<String>,
        /// Predicate refs gating implementation / availability / lock state.
        #[serde(default, skip_serializing_if = "PredicateRefs::is_empty")]
        predicates: PredicateRefs,
    },
    /// Boolean feature backed by a single bit/byte register or delegated via pValue.
    Boolean {
        name: String,
        meta: NodeMeta,
        /// Addressing metadata (absent when delegated via `pvalue`).
        addressing: Option<Addressing>,
        len: u32,
        access: AccessMode,
        bitfield: Option<BitField>,
        selectors: Vec<String>,
        selected_if: Vec<(String, Vec<String>)>,
        /// Node providing the value (delegates read/write to another node).
        pvalue: Option<String>,
        /// On value for pValue-backed booleans.
        on_value: Option<i64>,
        /// Off value for pValue-backed booleans.
        off_value: Option<i64>,
        /// Predicate refs gating implementation / availability / lock state.
        #[serde(default, skip_serializing_if = "PredicateRefs::is_empty")]
        predicates: PredicateRefs,
    },
    /// Command feature that triggers an action when written.
    Command {
        name: String,
        meta: NodeMeta,
        /// Fixed register address (absent when delegated via `pvalue`).
        address: Option<u64>,
        len: u32,
        /// Node providing the command register (delegates write).
        pvalue: Option<String>,
        /// Value to write when executing the command.
        command_value: Option<i64>,
        /// Predicate refs gating implementation / availability / lock state.
        #[serde(default, skip_serializing_if = "PredicateRefs::is_empty")]
        predicates: PredicateRefs,
    },
    /// Category used to organise features.
    Category {
        name: String,
        meta: NodeMeta,
        children: Vec<String>,
        /// Predicate refs gating implementation / availability.
        #[serde(default, skip_serializing_if = "PredicateRefs::is_empty")]
        predicates: PredicateRefs,
    },
    /// Computed value backed by an arithmetic expression referencing other nodes.
    SwissKnife(SwissKnifeDecl),
    /// Converter transforming raw values to/from user-facing floating-point values.
    Converter(ConverterDecl),
    /// IntConverter transforming raw values to/from user-facing integer values.
    IntConverter(IntConverterDecl),
    /// StringReg for string-typed register access.
    String(StringDecl),
}

/// A node element that could not be parsed and was left out of the model.
///
/// Parsing isolates each node, so a declaration we cannot make sense of costs
/// that one feature instead of the whole document. These records exist so the
/// loss is visible rather than silent — surface them in a UI, log them, or
/// assert on them in tests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkippedNode {
    /// XML tag of the node element, e.g. `Integer`.
    pub tag: String,
    /// `Name` attribute, when the element had one.
    pub name: Option<String>,
    /// Rendered parse error explaining why the node was dropped.
    pub error: String,
}

/// Full XML model describing the GenICam schema version and all declared nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XmlModel {
    /// Combined schema version extracted from the RegisterDescription attributes.
    pub version: String,
    /// Flat list of node declarations present in the document.
    pub nodes: Vec<NodeDecl>,
    /// Node elements that failed to parse and were skipped. Empty for a clean
    /// document.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skipped: Vec<SkippedNode>,
}

/// Minimal metadata extracted from a quick XML scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinimalXmlInfo {
    pub schema_version: Option<String>,
    pub top_level_features: Vec<String>,
}

/// Parse a GenICam XML snippet and collect minimal metadata.
pub fn parse_into_minimal_nodes(xml: &str) -> Result<MinimalXmlInfo, XmlError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    // Vendor XML is not ours to fix: a lone `&` in a tooltip must not stop us
    // from reading the document.
    reader.config_mut().allow_dangling_amp = true;
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
                depth = depth.saturating_sub(1);
            }
            Ok(Event::End(_)) => {
                depth = depth.saturating_sub(1);
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

/// `true` for tags this parser turns into one or more [`NodeDecl`]s.
fn is_node_tag(tag: &[u8]) -> bool {
    matches!(
        tag,
        b"Integer"
            | b"IntReg"
            | b"MaskedIntReg"
            | b"IntSwissKnife"
            | b"SwissKnife"
            | b"Float"
            | b"FloatReg"
            | b"Enumeration"
            | b"Boolean"
            | b"Command"
            | b"Category"
            | b"Converter"
            | b"IntConverter"
            | b"StringReg"
            | b"String"
            | b"StructReg"
    )
}

/// Dispatch a node element to its parser. `start` must be the element's start
/// event and `reader` must be positioned just after it.
fn parse_node(
    reader: &mut Reader<&[u8]>,
    start: BytesStart<'_>,
) -> Result<Vec<NodeDecl>, XmlError> {
    let tag = start.name().as_ref().to_vec();
    let node = match tag.as_slice() {
        b"Integer" | b"IntReg" | b"MaskedIntReg" => parse_integer(reader, start)?,
        b"IntSwissKnife" | b"SwissKnife" => parse_swissknife(reader, start)?,
        b"Float" | b"FloatReg" => parse_float(reader, start)?,
        b"Enumeration" => parse_enum(reader, start)?,
        b"Boolean" => parse_boolean(reader, start)?,
        b"Command" => parse_command(reader, start)?,
        b"Category" => parse_category(reader, start)?,
        b"Converter" => parse_converter(reader, start)?,
        b"IntConverter" => parse_int_converter(reader, start)?,
        b"StringReg" | b"String" => parse_string(reader, start)?,
        b"StructReg" => return parse_struct_reg(reader, start),
        other => {
            return Err(XmlError::Invalid(format!(
                "not a node element: {}",
                String::from_utf8_lossy(other)
            )));
        }
    };
    Ok(vec![node])
}

/// Parse one node element in isolation, from a slice spanning the whole element.
///
/// A fresh reader over just this element means a parse failure cannot leave the
/// document-level reader at an unknown position — the caller has already
/// consumed exactly this element and is correctly positioned either way.
fn parse_isolated_node(element: &str) -> Result<Vec<NodeDecl>, XmlError> {
    let mut reader = Reader::from_str(element);
    reader.config_mut().trim_text(true);
    reader.config_mut().allow_dangling_amp = true;
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let start = e.into_owned();
                return parse_node(&mut reader, start);
            }
            Ok(Event::Eof) => {
                return Err(XmlError::Invalid("node element is empty".into()));
            }
            Err(err) => return Err(XmlError::Xml(err.to_string())),
            // Leading whitespace or a comment before the start tag.
            _ => {}
        }
    }
}

/// Parse a GenICam XML document into an [`XmlModel`].
///
/// The parser only understands a practical subset of the schema. Unknown tags
/// are skipped which keeps the implementation forward compatible with richer
/// documents.
///
/// Each node element is parsed in isolation: one declaration we cannot make
/// sense of costs that single feature and is recorded in [`XmlModel::skipped`],
/// rather than failing the load and leaving the camera unopenable. Only errors
/// that make the document as a whole unreadable are returned.
pub fn parse(xml: &str) -> Result<XmlModel, XmlError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    // Vendor XML is not ours to fix: a lone `&` in a tooltip must not stop us
    // from opening the camera.
    reader.config_mut().allow_dangling_amp = true;
    let mut buf = Vec::new();
    let mut version = String::from("0.0.0");
    let mut nodes = Vec::new();
    let mut skipped = Vec::new();

    loop {
        // Captured before the read so a node element's slice can be recovered
        // verbatim, start tag included.
        let element_start = reader.buffer_position() as usize;
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => match e.name().as_ref() {
                b"RegisterDescription" => {
                    version = schema_version_from(e)?;
                }
                b"Group" => {
                    // Group is a transparent container wrapping feature nodes;
                    // let child events surface in the next loop iterations.
                }
                b"Port" => {
                    // Port nodes are transport-level abstractions; skip them.
                    skip_element(&mut reader, e.name().as_ref())?;
                }
                tag if is_node_tag(tag) => {
                    let tag = tag.to_vec();
                    let name = attribute_value(e, b"Name")?;
                    // Consume the element up front: whatever the node parser
                    // makes of it, the document reader stays in step.
                    reader
                        .read_to_end(QName(&tag))
                        .map_err(|err| XmlError::Xml(err.to_string()))?;
                    let element = &xml[element_start..reader.buffer_position() as usize];
                    match parse_isolated_node(element) {
                        Ok(parsed) => nodes.extend(parsed),
                        Err(err) => {
                            let tag = String::from_utf8_lossy(&tag).into_owned();
                            tracing::warn!(
                                tag = %tag,
                                node = name.as_deref().unwrap_or("<unnamed>"),
                                error = %err,
                                "skipping unparsable node"
                            );
                            skipped.push(SkippedNode {
                                tag,
                                name,
                                error: err.to_string(),
                            });
                        }
                    }
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

    if !skipped.is_empty() {
        tracing::warn!(
            count = skipped.len(),
            total = nodes.len() + skipped.len(),
            "some GenApi nodes could not be parsed and were skipped"
        );
    }

    Ok(XmlModel {
        version,
        nodes,
        skipped,
    })
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

    #[test]
    fn parse_minimal_xml() {
        let info = parse_into_minimal_nodes(FIXTURE).expect("parse xml");
        assert_eq!(info.schema_version.as_deref(), Some("1.2.3"));
        assert_eq!(info.top_level_features.len(), 7);
        assert_eq!(info.top_level_features[0], "Root");
    }

    #[test]
    fn parse_fixture_model() {
        let model = parse(FIXTURE).expect("parse fixture");
        assert_eq!(model.version, "1.2.3");
        assert_eq!(model.nodes.len(), 7);
        match &model.nodes[0] {
            NodeDecl::Category { name, children, .. } => {
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
                assert!(matches!(entries[0].value, EnumValueSrc::Literal(0)));
                assert!(matches!(entries[1].value, EnumValueSrc::Literal(1)));
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

    #[test]
    fn parse_swissknife_node() {
        const XML: &str = r#"
            <RegisterDescription SchemaMajorVersion="1" SchemaMinorVersion="0" SchemaSubMinorVersion="0">
                <Integer Name="GainRaw">
                    <Address>0x3000</Address>
                    <Length>4</Length>
                    <AccessMode>RW</AccessMode>
                    <Min>0</Min>
                    <Max>1000</Max>
                </Integer>
                <Float Name="Offset">
                    <Address>0x3008</Address>
                    <Length>4</Length>
                    <AccessMode>RW</AccessMode>
                    <Min>-100.0</Min>
                    <Max>100.0</Max>
                </Float>
                <SwissKnife Name="ComputedGain">
                    <Expression>(GainRaw * 0.5) + Offset</Expression>
                    <pVariable Name="GainRaw">GainRaw</pVariable>
                    <pVariable Name="Offset">Offset</pVariable>
                    <Output>Float</Output>
                </SwissKnife>
            </RegisterDescription>
        "#;

        let model = parse(XML).expect("parse swissknife xml");
        assert_eq!(model.nodes.len(), 3);
        let swiss = model
            .nodes
            .iter()
            .find_map(|decl| match decl {
                NodeDecl::SwissKnife(node) => Some(node),
                _ => None,
            })
            .expect("swissknife present");
        assert_eq!(swiss.name, "ComputedGain");
        assert_eq!(swiss.expr, "(GainRaw * 0.5) + Offset");
        assert_eq!(swiss.output, SkOutput::Float);
        assert_eq!(swiss.variables.len(), 2);
        assert_eq!(
            swiss.variables[0],
            ("GainRaw".to_string(), "GainRaw".to_string())
        );
        assert_eq!(
            swiss.variables[1],
            ("Offset".to_string(), "Offset".to_string())
        );
    }

    #[test]
    fn parse_int_swissknife_with_hex_and_ampersand() {
        // Test that &amp; is decoded to & and hex literals are supported.
        const XML: &str = r#"
            <RegisterDescription SchemaMajorVersion="1" SchemaMinorVersion="0" SchemaSubMinorVersion="0">
                <IntSwissKnife Name="PayloadSize">
                    <pVariable Name="W">Width</pVariable>
                    <pVariable Name="H">Height</pVariable>
                    <pVariable Name="PF">PixelFormat</pVariable>
                    <Formula>W * H * ((PF>>16)&amp;0xFF) / 8</Formula>
                </IntSwissKnife>
            </RegisterDescription>
        "#;

        let model = parse(XML).expect("parse intswissknife");
        assert_eq!(model.nodes.len(), 1);
        let swiss = model
            .nodes
            .iter()
            .find_map(|decl| match decl {
                NodeDecl::SwissKnife(node) => Some(node),
                _ => None,
            })
            .expect("swissknife present");
        assert_eq!(swiss.name, "PayloadSize");
        // &amp; should be decoded to &
        assert!(
            swiss.expr.contains('&'),
            "expression should contain decoded '&': {}",
            swiss.expr
        );
        assert!(
            swiss.expr.contains("0xFF"),
            "expression should contain hex literal: {}",
            swiss.expr
        );
    }

    #[test]
    fn parse_enum_entry_with_pvalue() {
        const XML: &str = r#"
            <RegisterDescription SchemaMajorVersion="1" SchemaMinorVersion="0" SchemaSubMinorVersion="0">
                <Enumeration Name="Mode">
                    <Address>0x0000_4000</Address>
                    <Length>4</Length>
                    <AccessMode>RW</AccessMode>
                    <EnumEntry Name="Fixed10">
                        <Value>10</Value>
                    </EnumEntry>
                    <EnumEntry Name="DynFromReg">
                        <pValue>RegModeVal</pValue>
                    </EnumEntry>
                </Enumeration>
                <Integer Name="RegModeVal">
                    <Address>0x0000_4100</Address>
                    <Length>4</Length>
                    <AccessMode>RW</AccessMode>
                    <Min>0</Min>
                    <Max>65535</Max>
                </Integer>
            </RegisterDescription>
        "#;

        let model = parse(XML).expect("parse enum pvalue");
        assert_eq!(model.nodes.len(), 2);
        match &model.nodes[0] {
            NodeDecl::Enum { entries, .. } => {
                assert_eq!(entries.len(), 2);
                assert!(matches!(entries[0].value, EnumValueSrc::Literal(10)));
                match &entries[1].value {
                    EnumValueSrc::FromNode(node) => assert_eq!(node, "RegModeVal"),
                    other => panic!("unexpected entry value: {other:?}"),
                }
            }
            other => panic!("unexpected node: {other:?}"),
        }
    }

    #[test]
    fn parse_indirect_addressing() {
        const XML: &str = r#"
            <RegisterDescription SchemaMajorVersion="1" SchemaMinorVersion="0" SchemaSubMinorVersion="0">
                <Integer Name="RegAddr">
                    <Address>0x2000</Address>
                    <Length>4</Length>
                    <AccessMode>RW</AccessMode>
                    <Min>0</Min>
                    <Max>65535</Max>
                </Integer>
                <Integer Name="Gain" Address="0xFFFF">
                    <pAddress>RegAddr</pAddress>
                    <Length>4</Length>
                    <AccessMode>RW</AccessMode>
                    <Min>0</Min>
                    <Max>255</Max>
                </Integer>
            </RegisterDescription>
        "#;

        let model = parse(XML).expect("parse indirect xml");
        assert_eq!(model.nodes.len(), 2);
        match &model.nodes[0] {
            NodeDecl::Integer {
                name, addressing, ..
            } => {
                assert_eq!(name, "RegAddr");
                assert!(
                    matches!(addressing, Some(Addressing::Fixed { address, len }) if *address == 0x2000 && *len == 4)
                );
            }
            other => panic!("unexpected node: {other:?}"),
        }
        match &model.nodes[1] {
            NodeDecl::Integer {
                name, addressing, ..
            } => {
                assert_eq!(name, "Gain");
                match addressing {
                    Some(Addressing::Indirect {
                        p_address_node,
                        len,
                    }) => {
                        assert_eq!(p_address_node, "RegAddr");
                        assert_eq!(*len, 4);
                    }
                    other => panic!("expected indirect addressing, got {other:?}"),
                }
            }
            other => panic!("unexpected node: {other:?}"),
        }
    }

    #[test]
    fn parse_indirect_float_is_scaled_integer() {
        // Regression test: indirect <Float> with Length=4 and no
        // <Scale>/<Offset> must NOT be reclassified as IEEE 754. Pointer-
        // backed float features on real cameras are almost always scaled
        // integer registers; silently decoding their bytes as IEEE 754
        // corrupts the value.
        const XML: &str = r#"
            <RegisterDescription SchemaMajorVersion="1" SchemaMinorVersion="0" SchemaSubMinorVersion="0">
                <Integer Name="RegAddr">
                    <Address>0x2000</Address>
                    <Length>4</Length>
                    <AccessMode>RW</AccessMode>
                </Integer>
                <Float Name="Exposure">
                    <pAddress>RegAddr</pAddress>
                    <Length>4</Length>
                    <AccessMode>RW</AccessMode>
                </Float>
            </RegisterDescription>
        "#;

        let model = parse(XML).expect("parse indirect float");
        let float = model
            .nodes
            .iter()
            .find(|n| matches!(n, NodeDecl::Float { name, .. } if name == "Exposure"))
            .expect("Exposure node");
        match float {
            NodeDecl::Float {
                encoding,
                addressing,
                ..
            } => {
                assert!(
                    matches!(addressing, Some(Addressing::Indirect { .. })),
                    "expected indirect addressing"
                );
                assert_eq!(*encoding, FloatEncoding::ScaledInteger);
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn parse_integer_bitfield_big_endian() {
        const XML: &str = r#"
            <RegisterDescription SchemaMajorVersion="1" SchemaMinorVersion="0" SchemaSubMinorVersion="0">
                <Integer Name="Packed">
                    <Address>0x1000</Address>
                    <Length>4</Length>
                    <AccessMode>RW</AccessMode>
                    <Min>0</Min>
                    <Max>65535</Max>
                    <Lsb>8</Lsb>
                    <Msb>15</Msb>
                    <Endianness>BigEndian</Endianness>
                </Integer>
            </RegisterDescription>
        "#;

        let model = parse(XML).expect("parse big-endian bitfield");
        assert_eq!(model.nodes.len(), 1);
        match &model.nodes[0] {
            NodeDecl::Integer { len, bitfield, .. } => {
                assert_eq!(*len, 4);
                let field = bitfield.as_ref().expect("bitfield present");
                assert_eq!(field.byte_order, ByteOrder::Big);
                assert_eq!(field.bit_length, 8);
                assert_eq!(field.bit_offset, 16);
            }
            other => panic!("unexpected node: {other:?}"),
        }
    }

    #[test]
    fn parse_boolean_bitfield_default_length() {
        const XML: &str = r#"
            <RegisterDescription SchemaMajorVersion="1" SchemaMinorVersion="0" SchemaSubMinorVersion="0">
                <Boolean Name="Flag">
                    <Address>0x2000</Address>
                    <Length>1</Length>
                    <AccessMode>RW</AccessMode>
                    <Bit>3</Bit>
                </Boolean>
            </RegisterDescription>
        "#;

        let model = parse(XML).expect("parse boolean bitfield");
        assert_eq!(model.nodes.len(), 1);
        match &model.nodes[0] {
            NodeDecl::Boolean { len, bitfield, .. } => {
                assert_eq!(*len, 1);
                let bf = bitfield.as_ref().expect("bitfield present");
                assert_eq!(bf.byte_order, ByteOrder::Little);
                assert_eq!(bf.bit_length, 1);
                assert_eq!(bf.bit_offset, 3);
            }
            other => panic!("unexpected node: {other:?}"),
        }
    }

    #[test]
    fn parse_integer_bitfield_mask() {
        const XML: &str = r#"
            <RegisterDescription SchemaMajorVersion="1" SchemaMinorVersion="0" SchemaSubMinorVersion="0">
                <Integer Name="Masked">
                    <Address>0x3000</Address>
                    <Length>4</Length>
                    <AccessMode>RW</AccessMode>
                    <Min>0</Min>
                    <Max>65535</Max>
                    <Mask>0x0000FF00</Mask>
                </Integer>
            </RegisterDescription>
        "#;

        let model = parse(XML).expect("parse mask bitfield");
        assert_eq!(model.nodes.len(), 1);
        match &model.nodes[0] {
            NodeDecl::Integer { bitfield, .. } => {
                let field = bitfield.as_ref().expect("bitfield present");
                assert_eq!(field.byte_order, ByteOrder::Little);
                assert_eq!(field.bit_length, 8);
                assert_eq!(field.bit_offset, 8);
            }
            other => panic!("unexpected node: {other:?}"),
        }
    }

    #[test]
    fn parse_node_metadata() {
        const XML: &str = r#"
            <RegisterDescription SchemaMajorVersion="1" SchemaMinorVersion="0" SchemaSubMinorVersion="0">
                <Integer Name="Width">
                    <Address>0x100</Address>
                    <Length>4</Length>
                    <AccessMode>RW</AccessMode>
                    <Min>16</Min>
                    <Max>4096</Max>
                    <Visibility>Expert</Visibility>
                    <Description>Image width in pixels.</Description>
                    <ToolTip>Width of the acquired image</ToolTip>
                    <DisplayName>Image Width</DisplayName>
                    <Representation>Linear</Representation>
                </Integer>
                <Float Name="Gain">
                    <Address>0x200</Address>
                    <Length>4</Length>
                    <AccessMode>RW</AccessMode>
                    <Min>0.0</Min>
                    <Max>48.0</Max>
                    <Unit>dB</Unit>
                    <Visibility>Beginner</Visibility>
                    <Representation>Logarithmic</Representation>
                </Float>
                <Category Name="Root">
                    <Visibility>Guru</Visibility>
                    <Description>Top-level category</Description>
                    <pFeature>Width</pFeature>
                    <pFeature>Gain</pFeature>
                </Category>
                <Enumeration Name="PixelFormat">
                    <Address>0x300</Address>
                    <Length>4</Length>
                    <AccessMode>RW</AccessMode>
                    <Visibility>Beginner</Visibility>
                    <ToolTip>Pixel format selector</ToolTip>
                    <EnumEntry Name="Mono8" Value="0" />
                </Enumeration>
            </RegisterDescription>
        "#;

        let model = parse(XML).expect("parse metadata xml");
        assert_eq!(model.nodes.len(), 4);

        // Integer with full metadata
        match &model.nodes[0] {
            NodeDecl::Integer { name, meta, .. } => {
                assert_eq!(name, "Width");
                assert_eq!(meta.visibility, Visibility::Expert);
                assert_eq!(meta.description.as_deref(), Some("Image width in pixels."));
                assert_eq!(meta.tooltip.as_deref(), Some("Width of the acquired image"));
                assert_eq!(meta.display_name.as_deref(), Some("Image Width"));
                assert_eq!(meta.representation, Some(Representation::Linear));
            }
            other => panic!("unexpected node: {other:?}"),
        }

        // Float with visibility + representation
        match &model.nodes[1] {
            NodeDecl::Float { name, meta, .. } => {
                assert_eq!(name, "Gain");
                assert_eq!(meta.visibility, Visibility::Beginner);
                assert_eq!(meta.representation, Some(Representation::Logarithmic));
                assert!(meta.description.is_none());
            }
            other => panic!("unexpected node: {other:?}"),
        }

        // Category with visibility + description
        match &model.nodes[2] {
            NodeDecl::Category { name, meta, .. } => {
                assert_eq!(name, "Root");
                assert_eq!(meta.visibility, Visibility::Guru);
                assert_eq!(meta.description.as_deref(), Some("Top-level category"));
            }
            other => panic!("unexpected node: {other:?}"),
        }

        // Enum with visibility + tooltip
        match &model.nodes[3] {
            NodeDecl::Enum { name, meta, .. } => {
                assert_eq!(name, "PixelFormat");
                assert_eq!(meta.visibility, Visibility::Beginner);
                assert_eq!(meta.tooltip.as_deref(), Some("Pixel format selector"));
            }
            other => panic!("unexpected node: {other:?}"),
        }
    }

    /// Wrap a node fragment in a minimal document and parse it.
    fn parse_fragment(node: &str) -> XmlModel {
        let xml = format!(
            r#"<RegisterDescription SchemaMajorVersion="1" SchemaMinorVersion="1" SchemaSubMinorVersion="0">{node}</RegisterDescription>"#
        );
        parse(&xml).expect("parse fragment")
    }

    /// Extract the metadata of the single node in a parsed fragment.
    fn only_meta(model: &XmlModel) -> &NodeMeta {
        match model.nodes.first().expect("one node") {
            NodeDecl::Integer { meta, .. } => meta,
            other => panic!("unexpected node: {other:?}"),
        }
    }

    /// Regression for issue #45: a FLIR BFS-PGE camera failed to open because a
    /// CDATA section in a text element was run through XML unescaping, where the
    /// literal `&` it legally contains has no `;` to terminate it.
    #[test]
    fn cdata_text_is_taken_literally() {
        let model = parse_fragment(
            r#"<Integer Name="Gain">
                 <Address>0x100</Address>
                 <ToolTip><![CDATA[Gain in dB & raw units, 0 < x < 10]]></ToolTip>
               </Integer>"#,
        );
        assert_eq!(
            only_meta(&model).tooltip.as_deref(),
            Some("Gain in dB & raw units, 0 < x < 10")
        );
    }

    /// Non-conformant vendor XML with a lone `&` must not stop the document from
    /// loading — a cosmetic tooltip is never worth failing a camera connect over.
    #[test]
    fn dangling_ampersand_is_kept_verbatim() {
        let model = parse_fragment(
            r#"<Integer Name="Gain">
                 <Address>0x100</Address>
                 <ToolTip>Exposure & gain</ToolTip>
               </Integer>"#,
        );
        assert_eq!(
            only_meta(&model).tooltip.as_deref(),
            Some("Exposure & gain")
        );
    }

    /// Comments are markup, not text: they are dropped, and a `&` inside one is
    /// not an entity reference.
    #[test]
    fn comments_inside_text_elements_are_dropped() {
        let model = parse_fragment(
            r#"<Integer Name="Gain">
                 <Address>0x100</Address>
                 <Description>Analog <!-- R&D note --> gain</Description>
               </Integer>"#,
        );
        assert_eq!(
            only_meta(&model).description.as_deref(),
            Some("Analog  gain")
        );
    }

    /// Entity references still resolve, and the whitespace around them survives
    /// even though the reader splits character data at each reference.
    #[test]
    fn entity_references_resolve_and_preserve_spacing() {
        let model = parse_fragment(
            r#"<Integer Name="Gain">
                 <Address>0x100</Address>
                 <ToolTip>A &amp; B &lt; C &gt; D &quot;E&quot; &apos;F&apos;</ToolTip>
               </Integer>"#,
        );
        assert_eq!(
            only_meta(&model).tooltip.as_deref(),
            Some(r#"A & B < C > D "E" 'F'"#)
        );
    }

    /// Numeric character references, both hexadecimal and decimal.
    #[test]
    fn character_references_resolve() {
        let model = parse_fragment(
            r#"<Integer Name="Gain">
                 <Address>0x100</Address>
                 <ToolTip>&#x2014; dash &#8212;</ToolTip>
               </Integer>"#,
        );
        assert_eq!(only_meta(&model).tooltip.as_deref(), Some("— dash —"));
    }

    /// GenICam declares no DTD, so an entity we cannot resolve is kept as written
    /// rather than failing the document.
    #[test]
    fn unknown_entity_is_kept_as_written() {
        let model = parse_fragment(
            r#"<Integer Name="Gain">
                 <Address>0x100</Address>
                 <ToolTip>copyright &copy; vendor</ToolTip>
               </Integer>"#,
        );
        assert_eq!(
            only_meta(&model).tooltip.as_deref(),
            Some("copyright &copy; vendor")
        );
    }

    /// SwissKnife formulas escape their bitwise/comparison operators; the parsed
    /// expression must contain the operators, not the entities.
    #[test]
    fn swissknife_formula_entities_resolve_to_operators() {
        let model = parse_fragment(
            r#"<IntSwissKnife Name="GainMask">
                 <pVariable Name="RAW">Gain</pVariable>
                 <Formula>(RAW &amp; 0xFF) &lt; 16</Formula>
               </IntSwissKnife>"#,
        );
        match model.nodes.first().expect("one node") {
            NodeDecl::SwissKnife(node) => assert_eq!(node.expr, "(RAW & 0xFF) < 16"),
            other => panic!("unexpected node: {other:?}"),
        }
    }

    /// A SwissKnife whose formula is constant needs no `<pVariable>`. Rejecting
    /// these blocked a Hikrobot MV-CS050-10GC on `PixelDynamicRangeMin_Value`
    /// (reported in #35) and appears in the standard's own conformance document.
    #[test]
    fn swissknife_without_variables_is_accepted() {
        let model = parse_fragment(
            r#"<IntSwissKnife Name="PixelDynamicRangeMin_Value">
                 <Formula>0x1234</Formula>
               </IntSwissKnife>"#,
        );
        match model.nodes.first().expect("one node") {
            NodeDecl::SwissKnife(node) => {
                assert_eq!(node.expr, "0x1234");
                assert!(node.variables.is_empty());
            }
            other => panic!("unexpected node: {other:?}"),
        }
    }

    /// `<AccessMode>` values beyond the standard's three spellings appear in
    /// third-party documents; an odd one must not cost the whole camera.
    #[test]
    fn access_mode_accepts_aliases_and_defaults_unknown_to_rw() {
        assert_eq!(AccessMode::parse("R").unwrap(), AccessMode::RO);
        assert_eq!(AccessMode::parse("W").unwrap(), AccessMode::WO);
        assert_eq!(AccessMode::parse("WR").unwrap(), AccessMode::RW);
        assert_eq!(AccessMode::parse(" ro ").unwrap(), AccessMode::RO);
        assert_eq!(AccessMode::parse("Bogus").unwrap(), AccessMode::RW);
    }

    /// A node we cannot parse costs that one feature, not the whole document —
    /// and the reader stays in step, so nodes after it still load.
    #[test]
    fn unparsable_node_is_skipped_not_fatal() {
        let model = parse_fragment(
            r#"<Integer Name="Before">
                 <Address>0x100</Address>
                 <Length>4</Length>
               </Integer>
               <Integer Name="Broken">
                 <Address>0xZZZZ</Address>
                 <Length>4</Length>
               </Integer>
               <Integer Name="After">
                 <Address>0x200</Address>
                 <Length>4</Length>
               </Integer>"#,
        );

        let names: Vec<&str> = model
            .nodes
            .iter()
            .map(|node| match node {
                NodeDecl::Integer { name, .. } => name.as_str(),
                other => panic!("unexpected node: {other:?}"),
            })
            .collect();
        assert_eq!(names, ["Before", "After"]);

        assert_eq!(model.skipped.len(), 1);
        let skipped = &model.skipped[0];
        assert_eq!(skipped.tag, "Integer");
        assert_eq!(skipped.name.as_deref(), Some("Broken"));
        assert!(
            skipped.error.contains("invalid hex"),
            "unexpected error: {}",
            skipped.error
        );
    }

    /// Isolation must survive a node whose failure happens before the parser has
    /// consumed any children — the outer reader is positioned by the caller, not
    /// by how far the node parser got.
    #[test]
    fn node_missing_required_name_is_skipped() {
        let model = parse_fragment(
            r#"<Integer>
                 <Address>0x100</Address>
               </Integer>
               <Integer Name="Good">
                 <Address>0x200</Address>
                 <Length>4</Length>
               </Integer>"#,
        );
        assert_eq!(model.nodes.len(), 1);
        assert_eq!(model.skipped.len(), 1);
        assert_eq!(model.skipped[0].name, None);
    }

    /// A clean document reports nothing skipped.
    #[test]
    fn clean_document_skips_nothing() {
        let model = parse(FIXTURE).expect("parse fixture");
        assert!(model.skipped.is_empty());
    }
}
