//! GenApi node system: typed feature access backed by register IO.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use genapi_xml::{AccessMode, NodeDecl, XmlModel};
use thiserror::Error;
use tracing::debug;

/// Error type produced by GenApi operations.
#[derive(Debug, Error)]
pub enum GenApiError {
    /// The requested node does not exist in the nodemap.
    #[error("node not found: {0}")]
    NodeNotFound(String),
    /// The node exists but has a different type.
    #[error("type mismatch for node: {0}")]
    Type(String),
    /// The node access mode forbids the attempted operation.
    #[error("access denied for node: {0}")]
    Access(String),
    /// The provided value violates the limits declared by the node.
    #[error("range error for node: {0}")]
    Range(String),
    /// The node is currently hidden by selector state.
    #[error("node unavailable: {0}")]
    Unavailable(String),
    /// Underlying register IO failed.
    #[error("io error: {0}")]
    Io(String),
    /// Node metadata or conversion failed.
    #[error("parse error: {0}")]
    Parse(String),
}

/// Register access abstraction backed by transports such as GVCP/GenCP.
pub trait RegisterIo {
    /// Read `len` bytes starting at `addr`.
    fn read(&self, addr: u64, len: usize) -> Result<Vec<u8>, GenApiError>;
    /// Write `data` starting at `addr`.
    fn write(&self, addr: u64, data: &[u8]) -> Result<(), GenApiError>;
}

/// Node kinds supported by the Tier-1 subset.
#[derive(Debug)]
pub enum Node {
    /// Signed integer feature stored in a fixed-width register block.
    Integer(IntegerNode),
    /// Floating point feature with optional scale/offset conversion.
    Float(FloatNode),
    /// Enumeration feature mapping integers to symbolic names.
    Enum(EnumNode),
    /// Boolean feature represented as an integer register.
    Boolean(BooleanNode),
    /// Command feature triggering a device-side action when written.
    Command(CommandNode),
    /// Category organising related features.
    Category(CategoryNode),
}

impl Node {
    fn invalidate_cache(&self) {
        match self {
            Node::Integer(node) => {
                node.cache.replace(None);
            }
            Node::Float(node) => {
                node.cache.replace(None);
            }
            Node::Enum(node) => {
                node.cache.replace(None);
            }
            Node::Boolean(node) => {
                node.cache.replace(None);
            }
            Node::Command(_) | Node::Category(_) => {}
        }
    }
}

/// Integer feature metadata extracted from the XML description.
#[derive(Debug)]
pub struct IntegerNode {
    /// Unique feature name.
    pub name: String,
    /// Absolute register address relative to the device control space.
    pub address: u64,
    /// Register length in bytes.
    pub len: u32,
    /// Declared access rights.
    pub access: AccessMode,
    /// Minimum permitted user value.
    pub min: i64,
    /// Maximum permitted user value.
    pub max: i64,
    /// Optional increment step the value must respect.
    pub inc: Option<i64>,
    /// Optional engineering unit such as "us".
    pub unit: Option<String>,
    /// Selector nodes controlling the visibility of this node.
    pub selectors: Vec<String>,
    /// Selector gating rules in the form `(selector, allowed values)`.
    pub selected_if: Vec<(String, Vec<String>)>,
    cache: RefCell<Option<i64>>,
}

/// Floating point feature metadata.
#[derive(Debug)]
pub struct FloatNode {
    pub name: String,
    pub address: u64,
    pub len: u32,
    pub access: AccessMode,
    pub min: f64,
    pub max: f64,
    pub unit: Option<String>,
    /// Optional rational scale `(numerator, denominator)` applied to the raw value.
    pub scale: Option<(i64, i64)>,
    /// Optional offset added after scaling.
    pub offset: Option<f64>,
    pub selectors: Vec<String>,
    pub selected_if: Vec<(String, Vec<String>)>,
    cache: RefCell<Option<f64>>,
}

/// Enumeration feature metadata and mapping tables.
#[derive(Debug)]
pub struct EnumNode {
    pub name: String,
    pub address: u64,
    pub len: u32,
    pub access: AccessMode,
    pub entries: Vec<(String, i64)>,
    pub default: Option<String>,
    pub selectors: Vec<String>,
    pub selected_if: Vec<(String, Vec<String>)>,
    map_by_name: HashMap<String, i64>,
    map_by_value: HashMap<i64, String>,
    cache: RefCell<Option<String>>,
}

/// Boolean feature metadata.
#[derive(Debug)]
pub struct BooleanNode {
    pub name: String,
    pub address: u64,
    pub len: u32,
    pub access: AccessMode,
    pub selectors: Vec<String>,
    pub selected_if: Vec<(String, Vec<String>)>,
    cache: RefCell<Option<bool>>,
}

/// Command feature metadata.
#[derive(Debug)]
pub struct CommandNode {
    pub name: String,
    pub address: u64,
    pub len: u32,
}

/// Category node describing child feature names.
#[derive(Debug)]
pub struct CategoryNode {
    pub name: String,
    pub children: Vec<String>,
}

/// Runtime nodemap built from an [`XmlModel`] capable of reading and writing
/// feature values via a [`RegisterIo`] transport.
#[derive(Debug)]
pub struct NodeMap {
    version: String,
    nodes: HashMap<String, Node>,
    dependents: HashMap<String, Vec<String>>,
}

impl NodeMap {
    /// Return the schema version string associated with the XML description.
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Fetch a node by name for inspection.
    pub fn node(&self, name: &str) -> Option<&Node> {
        self.nodes.get(name)
    }

    /// Read an integer feature value using the provided transport.
    pub fn get_integer(&self, name: &str, io: &dyn RegisterIo) -> Result<i64, GenApiError> {
        let node = self.get_integer_node(name)?;
        ensure_readable(&node.access, name)?;
        self.ensure_selectors(name, &node.selected_if, io)?;
        if let Some(value) = *node.cache.borrow() {
            return Ok(value);
        }
        let raw = io
            .read(node.address, node.len as usize)
            .map_err(|err| match err {
                GenApiError::Io(_) => err,
                other => other,
            })?;
        let value = bytes_to_i64(name, &raw)?;
        debug!(node = %name, raw = value, "read integer feature");
        node.cache.replace(Some(value));
        Ok(value)
    }

    /// Write an integer feature and update dependent caches.
    pub fn set_integer(
        &mut self,
        name: &str,
        value: i64,
        io: &dyn RegisterIo,
    ) -> Result<(), GenApiError> {
        let node = self.get_integer_node(name)?;
        ensure_writable(&node.access, name)?;
        self.ensure_selectors(name, &node.selected_if, io)?;
        if value < node.min || value > node.max {
            return Err(GenApiError::Range(name.to_string()));
        }
        if let Some(inc) = node.inc {
            if inc != 0 && (value - node.min) % inc != 0 {
                return Err(GenApiError::Range(name.to_string()));
            }
        }
        let bytes = i64_to_bytes(name, value, node.len)?;
        debug!(node = %name, raw = value, "write integer feature");
        io.write(node.address, &bytes).map_err(|err| match err {
            GenApiError::Io(_) => err,
            other => other,
        })?;
        node.cache.replace(Some(value));
        self.invalidate_dependents(name);
        Ok(())
    }

    /// Read a floating point feature.
    pub fn get_float(&self, name: &str, io: &dyn RegisterIo) -> Result<f64, GenApiError> {
        let node = self.get_float_node(name)?;
        ensure_readable(&node.access, name)?;
        self.ensure_selectors(name, &node.selected_if, io)?;
        if let Some(value) = *node.cache.borrow() {
            return Ok(value);
        }
        let raw = io
            .read(node.address, node.len as usize)
            .map_err(|err| match err {
                GenApiError::Io(_) => err,
                other => other,
            })?;
        let raw_value = bytes_to_i64(name, &raw)?;
        let value = apply_scale(node, raw_value as f64);
        debug!(node = %name, raw = raw_value, value, "read float feature");
        node.cache.replace(Some(value));
        Ok(value)
    }

    /// Write a floating point feature using the scale/offset conversion.
    pub fn set_float(
        &mut self,
        name: &str,
        value: f64,
        io: &dyn RegisterIo,
    ) -> Result<(), GenApiError> {
        let node = self.get_float_node(name)?;
        ensure_writable(&node.access, name)?;
        self.ensure_selectors(name, &node.selected_if, io)?;
        if value < node.min || value > node.max {
            return Err(GenApiError::Range(name.to_string()));
        }
        let raw = encode_float(node, value)?;
        let bytes = i64_to_bytes(name, raw, node.len)?;
        debug!(node = %name, raw, value, "write float feature");
        io.write(node.address, &bytes).map_err(|err| match err {
            GenApiError::Io(_) => err,
            other => other,
        })?;
        node.cache.replace(Some(value));
        self.invalidate_dependents(name);
        Ok(())
    }

    /// Read an enumeration feature returning the symbolic entry name.
    pub fn get_enum(&self, name: &str, io: &dyn RegisterIo) -> Result<String, GenApiError> {
        let node = self.get_enum_node(name)?;
        ensure_readable(&node.access, name)?;
        self.ensure_selectors(name, &node.selected_if, io)?;
        if let Some(value) = node.cache.borrow().clone() {
            return Ok(value);
        }
        let raw = io
            .read(node.address, node.len as usize)
            .map_err(|err| match err {
                GenApiError::Io(_) => err,
                other => other,
            })?;
        let raw_value = bytes_to_i64(name, &raw)?;
        let entry = node.map_by_value.get(&raw_value).cloned().ok_or_else(|| {
            GenApiError::Parse(format!("unknown enum value {raw_value} for {name}"))
        })?;
        debug!(node = %name, raw = raw_value, entry = %entry, "read enum feature");
        node.cache.replace(Some(entry.clone()));
        Ok(entry)
    }

    /// Write an enumeration entry.
    pub fn set_enum(
        &mut self,
        name: &str,
        entry: &str,
        io: &dyn RegisterIo,
    ) -> Result<(), GenApiError> {
        let node = self.get_enum_node(name)?;
        ensure_writable(&node.access, name)?;
        self.ensure_selectors(name, &node.selected_if, io)?;
        let raw = *node
            .map_by_name
            .get(entry)
            .ok_or_else(|| GenApiError::Range(name.to_string()))?;
        let bytes = i64_to_bytes(name, raw, node.len)?;
        debug!(node = %name, raw, entry, "write enum feature");
        io.write(node.address, &bytes).map_err(|err| match err {
            GenApiError::Io(_) => err,
            other => other,
        })?;
        node.cache.replace(Some(entry.to_string()));
        self.invalidate_dependents(name);
        Ok(())
    }

    /// Read a boolean feature.
    pub fn get_bool(&self, name: &str, io: &dyn RegisterIo) -> Result<bool, GenApiError> {
        let node = self.get_bool_node(name)?;
        ensure_readable(&node.access, name)?;
        self.ensure_selectors(name, &node.selected_if, io)?;
        if let Some(value) = *node.cache.borrow() {
            return Ok(value);
        }
        let raw = io
            .read(node.address, node.len as usize)
            .map_err(|err| match err {
                GenApiError::Io(_) => err,
                other => other,
            })?;
        let raw_value = bytes_to_i64(name, &raw)?;
        let value = raw_value != 0;
        debug!(node = %name, raw = raw_value, value, "read boolean feature");
        node.cache.replace(Some(value));
        Ok(value)
    }

    /// Write a boolean feature.
    pub fn set_bool(
        &mut self,
        name: &str,
        value: bool,
        io: &dyn RegisterIo,
    ) -> Result<(), GenApiError> {
        let node = self.get_bool_node(name)?;
        ensure_writable(&node.access, name)?;
        self.ensure_selectors(name, &node.selected_if, io)?;
        let raw = if value { 1 } else { 0 };
        let bytes = i64_to_bytes(name, raw, node.len)?;
        debug!(node = %name, raw, value, "write boolean feature");
        io.write(node.address, &bytes).map_err(|err| match err {
            GenApiError::Io(_) => err,
            other => other,
        })?;
        node.cache.replace(Some(value));
        self.invalidate_dependents(name);
        Ok(())
    }

    /// Execute a command feature by writing a one-valued payload.
    pub fn exec_command(&mut self, name: &str, io: &dyn RegisterIo) -> Result<(), GenApiError> {
        let node = self.get_command_node(name)?;
        if node.len == 0 {
            return Err(GenApiError::Parse(format!(
                "command node {name} has zero length"
            )));
        }
        let mut data = vec![0u8; node.len as usize];
        if let Some(last) = data.last_mut() {
            *last = 1;
        }
        debug!(node = %name, "execute command");
        io.write(node.address, &data).map_err(|err| match err {
            GenApiError::Io(_) => err,
            other => other,
        })?;
        self.invalidate_dependents(name);
        Ok(())
    }

    fn get_integer_node(&self, name: &str) -> Result<&IntegerNode, GenApiError> {
        match self.nodes.get(name) {
            Some(Node::Integer(node)) => Ok(node),
            Some(_) => Err(GenApiError::Type(name.to_string())),
            None => Err(GenApiError::NodeNotFound(name.to_string())),
        }
    }

    fn get_float_node(&self, name: &str) -> Result<&FloatNode, GenApiError> {
        match self.nodes.get(name) {
            Some(Node::Float(node)) => Ok(node),
            Some(_) => Err(GenApiError::Type(name.to_string())),
            None => Err(GenApiError::NodeNotFound(name.to_string())),
        }
    }

    fn get_enum_node(&self, name: &str) -> Result<&EnumNode, GenApiError> {
        match self.nodes.get(name) {
            Some(Node::Enum(node)) => Ok(node),
            Some(_) => Err(GenApiError::Type(name.to_string())),
            None => Err(GenApiError::NodeNotFound(name.to_string())),
        }
    }

    fn get_bool_node(&self, name: &str) -> Result<&BooleanNode, GenApiError> {
        match self.nodes.get(name) {
            Some(Node::Boolean(node)) => Ok(node),
            Some(_) => Err(GenApiError::Type(name.to_string())),
            None => Err(GenApiError::NodeNotFound(name.to_string())),
        }
    }

    fn get_command_node(&self, name: &str) -> Result<&CommandNode, GenApiError> {
        match self.nodes.get(name) {
            Some(Node::Command(node)) => Ok(node),
            Some(_) => Err(GenApiError::Type(name.to_string())),
            None => Err(GenApiError::NodeNotFound(name.to_string())),
        }
    }

    fn ensure_selectors(
        &self,
        node_name: &str,
        rules: &[(String, Vec<String>)],
        io: &dyn RegisterIo,
    ) -> Result<(), GenApiError> {
        for (selector, allowed) in rules {
            if allowed.is_empty() {
                continue;
            }
            let current = self.get_selector_value(selector, io)?;
            if !allowed.iter().any(|value| value == &current) {
                return Err(GenApiError::Unavailable(node_name.to_string()));
            }
        }
        Ok(())
    }

    fn get_selector_value(
        &self,
        selector: &str,
        io: &dyn RegisterIo,
    ) -> Result<String, GenApiError> {
        match self.nodes.get(selector) {
            Some(Node::Enum(_)) => self.get_enum(selector, io),
            Some(Node::Boolean(_)) => Ok(self.get_bool(selector, io)?.to_string()),
            Some(Node::Integer(_)) => Ok(self.get_integer(selector, io)?.to_string()),
            Some(_) => Err(GenApiError::Parse(format!(
                "selector {selector} has unsupported type"
            ))),
            None => Err(GenApiError::NodeNotFound(selector.to_string())),
        }
    }

    fn invalidate_dependents(&self, name: &str) {
        if let Some(children) = self.dependents.get(name) {
            let mut visited = HashSet::new();
            for child in children {
                self.invalidate_recursive(child, &mut visited);
            }
        }
    }

    fn invalidate_recursive(&self, name: &str, visited: &mut HashSet<String>) {
        if !visited.insert(name.to_string()) {
            return;
        }
        if let Some(node) = self.nodes.get(name) {
            node.invalidate_cache();
        }
        if let Some(children) = self.dependents.get(name) {
            for child in children {
                self.invalidate_recursive(child, visited);
            }
        }
    }
}

impl From<XmlModel> for NodeMap {
    fn from(model: XmlModel) -> Self {
        let mut nodes = HashMap::new();
        let mut dependents: HashMap<String, Vec<String>> = HashMap::new();
        for decl in model.nodes {
            match decl {
                NodeDecl::Integer {
                    name,
                    address,
                    len,
                    access,
                    min,
                    max,
                    inc,
                    unit,
                    selectors,
                    selected_if,
                } => {
                    for (selector, _) in &selected_if {
                        dependents
                            .entry(selector.clone())
                            .or_default()
                            .push(name.clone());
                    }
                    let node = IntegerNode {
                        name: name.clone(),
                        address,
                        len,
                        access,
                        min,
                        max,
                        inc,
                        unit,
                        selectors,
                        selected_if,
                        cache: RefCell::new(None),
                    };
                    nodes.insert(name, Node::Integer(node));
                }
                NodeDecl::Float {
                    name,
                    address,
                    len,
                    access,
                    min,
                    max,
                    unit,
                    scale,
                    offset,
                    selectors,
                    selected_if,
                } => {
                    for (selector, _) in &selected_if {
                        dependents
                            .entry(selector.clone())
                            .or_default()
                            .push(name.clone());
                    }
                    let node = FloatNode {
                        name: name.clone(),
                        address,
                        len,
                        access,
                        min,
                        max,
                        unit,
                        scale,
                        offset,
                        selectors,
                        selected_if,
                        cache: RefCell::new(None),
                    };
                    nodes.insert(name, Node::Float(node));
                }
                NodeDecl::Enum {
                    name,
                    address,
                    len,
                    access,
                    entries,
                    default,
                    selectors,
                    selected_if,
                } => {
                    for (selector, _) in &selected_if {
                        dependents
                            .entry(selector.clone())
                            .or_default()
                            .push(name.clone());
                    }
                    let mut map_by_name = HashMap::new();
                    let mut map_by_value = HashMap::new();
                    for (entry, value) in &entries {
                        map_by_name.insert(entry.clone(), *value);
                        map_by_value.entry(*value).or_insert_with(|| entry.clone());
                    }
                    let node = EnumNode {
                        name: name.clone(),
                        address,
                        len,
                        access,
                        entries,
                        default,
                        selectors,
                        selected_if,
                        map_by_name,
                        map_by_value,
                        cache: RefCell::new(None),
                    };
                    nodes.insert(name, Node::Enum(node));
                }
                NodeDecl::Boolean {
                    name,
                    address,
                    len,
                    access,
                    selectors,
                    selected_if,
                } => {
                    for (selector, _) in &selected_if {
                        dependents
                            .entry(selector.clone())
                            .or_default()
                            .push(name.clone());
                    }
                    let node = BooleanNode {
                        name: name.clone(),
                        address,
                        len,
                        access,
                        selectors,
                        selected_if,
                        cache: RefCell::new(None),
                    };
                    nodes.insert(name, Node::Boolean(node));
                }
                NodeDecl::Command { name, address, len } => {
                    let node = CommandNode {
                        name: name.clone(),
                        address,
                        len,
                    };
                    nodes.insert(name, Node::Command(node));
                }
                NodeDecl::Category { name, children } => {
                    let node = CategoryNode {
                        name: name.clone(),
                        children,
                    };
                    nodes.insert(name, Node::Category(node));
                }
            }
        }

        NodeMap {
            version: model.version,
            nodes,
            dependents,
        }
    }
}

fn ensure_readable(access: &AccessMode, name: &str) -> Result<(), GenApiError> {
    if matches!(access, AccessMode::WO) {
        return Err(GenApiError::Access(name.to_string()));
    }
    Ok(())
}

fn ensure_writable(access: &AccessMode, name: &str) -> Result<(), GenApiError> {
    if matches!(access, AccessMode::RO) {
        return Err(GenApiError::Access(name.to_string()));
    }
    Ok(())
}

fn bytes_to_i64(name: &str, bytes: &[u8]) -> Result<i64, GenApiError> {
    if bytes.is_empty() {
        return Err(GenApiError::Parse(format!(
            "node {name} returned empty payload"
        )));
    }
    if bytes.len() > 8 {
        return Err(GenApiError::Parse(format!(
            "node {name} uses unsupported width {}",
            bytes.len()
        )));
    }
    let mut buf = [0u8; 8];
    let offset = 8 - bytes.len();
    buf[offset..].copy_from_slice(bytes);
    if !bytes.is_empty() && (bytes[0] & 0x80) != 0 {
        for byte in &mut buf[..offset] {
            *byte = 0xFF;
        }
    }
    Ok(i64::from_be_bytes(buf))
}

fn i64_to_bytes(name: &str, value: i64, width: u32) -> Result<Vec<u8>, GenApiError> {
    if width == 0 || width > 8 {
        return Err(GenApiError::Parse(format!(
            "node {name} has unsupported width {width}"
        )));
    }
    let width = width as usize;
    let bytes = value.to_be_bytes();
    let mut data = bytes[8 - width..].to_vec();
    let roundtrip = bytes_to_i64(name, &data)?;
    if roundtrip != value {
        return Err(GenApiError::Range(format!(
            "value {value} does not fit {width} bytes for {name}"
        )));
    }
    Ok(data)
}

fn apply_scale(node: &FloatNode, raw: f64) -> f64 {
    let mut value = raw;
    if let Some((num, den)) = node.scale {
        value *= num as f64 / den as f64;
    }
    if let Some(offset) = node.offset {
        value += offset;
    }
    value
}

fn encode_float(node: &FloatNode, value: f64) -> Result<i64, GenApiError> {
    let mut raw = value;
    if let Some(offset) = node.offset {
        raw -= offset;
    }
    if let Some((num, den)) = node.scale {
        if num == 0 {
            return Err(GenApiError::Parse(format!(
                "node {} has zero scale numerator",
                node.name
            )));
        }
        raw *= den as f64 / num as f64;
    }
    let rounded = raw.round();
    if (raw - rounded).abs() > 1e-6 {
        return Err(GenApiError::Range(node.name.clone()));
    }
    let raw_i64 = rounded as i64;
    Ok(raw_i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"
        <RegisterDescription SchemaMajorVersion="1" SchemaMinorVersion="2" SchemaSubMinorVersion="3">
            <Integer Name="Width">
                <Address>0x100</Address>
                <Length>4</Length>
                <AccessMode>RW</AccessMode>
                <Min>16</Min>
                <Max>4096</Max>
                <Inc>2</Inc>
            </Integer>
            <Float Name="ExposureTime">
                <Address>0x200</Address>
                <Length>4</Length>
                <AccessMode>RW</AccessMode>
                <Min>10.0</Min>
                <Max>100000.0</Max>
                <Scale>1/1000</Scale>
            </Float>
            <Enumeration Name="GainSelector">
                <Address>0x300</Address>
                <Length>2</Length>
                <AccessMode>RW</AccessMode>
                <EnumEntry Name="AnalogAll" Value="0" />
                <EnumEntry Name="DigitalAll" Value="1" />
            </Enumeration>
            <Integer Name="Gain">
                <Address>0x304</Address>
                <Length>2</Length>
                <AccessMode>RW</AccessMode>
                <Min>0</Min>
                <Max>48</Max>
                <pSelected>GainSelector</pSelected>
                <Selected>AnalogAll</Selected>
            </Integer>
            <Boolean Name="GammaEnable">
                <Address>0x400</Address>
                <Length>1</Length>
                <AccessMode>RW</AccessMode>
            </Boolean>
            <Command Name="AcquisitionStart">
                <Address>0x500</Address>
                <Length>4</Length>
            </Command>
        </RegisterDescription>
    "#;

    #[derive(Default)]
    struct MockIo {
        regs: RefCell<HashMap<u64, Vec<u8>>>,
        reads: RefCell<HashMap<u64, usize>>,
    }

    impl MockIo {
        fn with_registers(entries: &[(u64, Vec<u8>)]) -> Self {
            let mut regs = HashMap::new();
            for (addr, data) in entries {
                regs.insert(*addr, data.clone());
            }
            MockIo {
                regs: RefCell::new(regs),
                reads: RefCell::new(HashMap::new()),
            }
        }

        fn read_count(&self, addr: u64) -> usize {
            *self.reads.borrow().get(&addr).unwrap_or(&0)
        }
    }

    impl RegisterIo for MockIo {
        fn read(&self, addr: u64, len: usize) -> Result<Vec<u8>, GenApiError> {
            let mut reads = self.reads.borrow_mut();
            *reads.entry(addr).or_default() += 1;
            let regs = self.regs.borrow();
            let data = regs
                .get(&addr)
                .ok_or_else(|| GenApiError::Io(format!("read miss at 0x{addr:08X}")))?;
            if data.len() != len {
                return Err(GenApiError::Io(format!(
                    "length mismatch at 0x{addr:08X}: expected {len}, have {}",
                    data.len()
                )));
            }
            Ok(data.clone())
        }

        fn write(&self, addr: u64, data: &[u8]) -> Result<(), GenApiError> {
            self.regs.borrow_mut().insert(addr, data.to_vec());
            Ok(())
        }
    }

    fn build_nodemap() -> NodeMap {
        let model = genapi_xml::parse(FIXTURE).expect("parse fixture");
        NodeMap::from(model)
    }

    #[test]
    fn integer_roundtrip_and_cache() {
        let mut nodemap = build_nodemap();
        let io = MockIo::with_registers(&[(0x100, vec![0, 0, 4, 0])]);
        let width = nodemap.get_integer("Width", &io).expect("read width");
        assert_eq!(width, 1024);
        assert_eq!(io.read_count(0x100), 1);
        let width_again = nodemap.get_integer("Width", &io).expect("cached width");
        assert_eq!(width_again, 1024);
        assert_eq!(io.read_count(0x100), 1, "cached value should be reused");
        nodemap
            .set_integer("Width", 1030, &io)
            .expect("write width");
        let width = nodemap
            .get_integer("Width", &io)
            .expect("read updated width");
        assert_eq!(width, 1030);
        assert_eq!(io.read_count(0x100), 1, "write should update cache");
    }

    #[test]
    fn float_conversion_roundtrip() {
        let mut nodemap = build_nodemap();
        let raw = 50_000i64; // 50 ms with 1/1000 scale
        let io = MockIo::with_registers(&[(0x200, i64_to_bytes("ExposureTime", raw, 4).unwrap())]);
        let exposure = nodemap
            .get_float("ExposureTime", &io)
            .expect("read exposure");
        assert!((exposure - 50.0).abs() < 1e-6);
        nodemap
            .set_float("ExposureTime", 75.0, &io)
            .expect("write exposure");
        let raw_back = bytes_to_i64("ExposureTime", &io.read(0x200, 4).unwrap()).unwrap();
        assert_eq!(raw_back, 75_000);
    }

    #[test]
    fn selector_gating() {
        let mut nodemap = build_nodemap();
        let io = MockIo::with_registers(&[
            (0x300, i64_to_bytes("GainSelector", 0, 2).unwrap()),
            (0x304, i64_to_bytes("Gain", 10, 2).unwrap()),
        ]);
        let gain = nodemap.get_integer("Gain", &io).expect("gain available");
        assert_eq!(gain, 10);
        nodemap
            .set_enum("GainSelector", "DigitalAll", &io)
            .expect("set selector");
        let err = nodemap.get_integer("Gain", &io).unwrap_err();
        assert!(matches!(err, GenApiError::Unavailable(_)));
    }

    #[test]
    fn range_enforcement() {
        let mut nodemap = build_nodemap();
        let io = MockIo::with_registers(&[(0x100, vec![0, 0, 0, 16])]);
        let err = nodemap.set_integer("Width", 17, &io).unwrap_err();
        assert!(matches!(err, GenApiError::Range(_)));
    }

    #[test]
    fn command_exec() {
        let mut nodemap = build_nodemap();
        let io = MockIo::with_registers(&[]);
        nodemap
            .exec_command("AcquisitionStart", &io)
            .expect("exec command");
        let payload = io.read(0x500, 4).expect("command write");
        assert_eq!(payload, vec![0, 0, 0, 1]);
    }
}
