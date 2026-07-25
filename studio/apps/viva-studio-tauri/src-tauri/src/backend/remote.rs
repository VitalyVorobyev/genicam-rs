//! Stub remote backend for Zenoh-based camera access.
//!
//! The existing Zenoh commands in `commands/device.rs`, `commands/nodes.rs`, and
//! `commands/acquisition.rs` remain the working implementation for remote mode.
//! This stub exists so that the `DeviceBackend` trait has a concrete remote type
//! that can be instantiated when the app detects a Zenoh configuration.
//!
//! Full migration of the Zenoh commands to this trait is planned for a follow-up.

use std::collections::HashMap;

use async_trait::async_trait;

use crate::state::device_state::{DeviceInfo, NodeValueEntry, StreamerInfo};

use super::{BackendMode, ConnectResult, DeviceBackend};

/// Placeholder remote backend.
///
/// In remote mode the existing `ZenohState`-based commands handle all device
/// communication. This struct satisfies the `DeviceBackend` trait so the app
/// can query the backend mode from the frontend.
pub struct RemoteBackend;

#[async_trait]
impl DeviceBackend for RemoteBackend {
    fn mode(&self) -> BackendMode {
        BackendMode::Remote
    }

    async fn discover(&self) -> Vec<DeviceInfo> {
        // Discovery is handled by the existing Zenoh subscriber in device.rs.
        Vec::new()
    }

    async fn connect(&self, _device_id: &str) -> Result<ConnectResult, String> {
        Err("Remote mode: device operations use Zenoh commands. \
             Pass --zenoh-config or set ZENOH_CONFIG to enable."
            .to_string())
    }

    async fn disconnect(&self, _device_id: &str) -> Result<(), String> {
        Err("Remote mode: use existing Zenoh disconnect command".to_string())
    }

    async fn get_feature(&self, _name: &str) -> Result<NodeValueEntry, String> {
        Err("Remote mode: use existing Zenoh node commands".to_string())
    }

    async fn set_feature(&self, _name: &str, _value: &serde_json::Value) -> Result<(), String> {
        Err("Remote mode: use existing Zenoh node commands".to_string())
    }

    async fn exec_command(&self, _name: &str) -> Result<(), String> {
        Err("Remote mode: use existing Zenoh node commands".to_string())
    }

    async fn bulk_read(
        &self,
        _names: &[String],
    ) -> Result<HashMap<String, NodeValueEntry>, String> {
        Err("Remote mode: use existing Zenoh node commands".to_string())
    }

    async fn start_acquisition(&self) -> Result<StreamerInfo, String> {
        Err("Remote mode: use existing Zenoh acquisition commands".to_string())
    }

    async fn stop_acquisition(&self) -> Result<(), String> {
        Err("Remote mode: use existing Zenoh acquisition commands".to_string())
    }

    async fn get_xml(&self) -> Result<String, String> {
        Err("Remote mode: use existing Zenoh XML fetch".to_string())
    }
}
