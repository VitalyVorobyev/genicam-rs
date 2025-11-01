//! GenApi node system: minimal types, to be expanded.

use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GenApiError {
    #[error("node not found: {0}")]
    NodeNotFound(String),
    #[error("type mismatch for node: {0}")]
    TypeMismatch(String),
    #[error("range error for node: {0}")]
    Range(String),
}

#[derive(Debug, Clone)]
pub enum Node {
    Integer(IntegerNode),
    Float(FloatNode),
    Enum(EnumNode),
    Boolean(BooleanNode),
    Command(CommandNode),
    Category(CategoryNode),
}

#[derive(Debug, Clone)]
pub struct IntegerNode { pub name: String, pub min: i64, pub max: i64, pub value: i64 }
#[derive(Debug, Clone)]
pub struct FloatNode   { pub name: String, pub min: f64, pub max: f64, pub value: f64, pub unit: Option<String> }
#[derive(Debug, Clone)]
pub struct EnumNode    { pub name: String, pub entries: Vec<String>, pub value: String }
#[derive(Debug, Clone)]
pub struct BooleanNode { pub name: String, pub value: bool }
#[derive(Debug, Clone)]
pub struct CommandNode { pub name: String }
#[derive(Debug, Clone)]
pub struct CategoryNode{ pub name: String, pub children: Vec<String> }

#[derive(Debug, Default, Clone)]
pub struct NodeMap {
    nodes: HashMap<String, Node>,
}

impl NodeMap {
    pub fn get(&self, name: &str) -> Option<&Node> { self.nodes.get(name) }
    pub fn get_mut(&mut self, name: &str) -> Option<&mut Node> { self.nodes.get_mut(name) }
    pub fn insert(&mut self, name: String, node: Node) { self.nodes.insert(name, node); }
}
