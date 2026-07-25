//! Library interface for `viva-mock-service`.
//!
//! Exposes all service modules so they can be used from integration tests
//! and other crates without going through the binary entry point.

pub mod acquisition;
pub mod config;
pub mod discovery;
pub mod interdependencies;
pub mod nodes;
pub mod state;
pub mod status;
pub mod xml_queryable;
