//! Tauri commands for the embedded device backend.
//!
//! These commands delegate to the [`DeviceBackend`] trait and are used when
//! the app runs in embedded mode (direct GigE/USB3 camera access).
//! They can also be called in remote mode, where the backend returns
//! informative error messages directing the user to the Zenoh commands.

use std::collections::HashMap;

use tauri::{AppHandle, Emitter, State};
use tokio::sync::RwLock;

use crate::backend::BackendState;
use crate::commands::xml_model::{ModelSummary, ParseXmlResponse};
use crate::state::ModelState;
use crate::state::device_state::{ConnectionState, DeviceInfo, NodeValueEntry, StreamerInfo};
use viva_xml_model::parse_genicam_xml;

#[tauri::command]
pub async fn embedded_discover(
    backend: State<'_, BackendState>,
) -> Result<Vec<DeviceInfo>, String> {
    Ok(backend.discover().await)
}

#[tauri::command]
pub async fn embedded_connect(
    device_id: String,
    backend: State<'_, BackendState>,
    model: State<'_, RwLock<ModelState>>,
    app: AppHandle,
) -> Result<ParseXmlResponse, String> {
    let result = backend.connect(&device_id).await?;

    // Parse the XML into a UiGraph.
    let graph = parse_genicam_xml(&result.xml).map_err(|e| e.to_string())?;

    let summary = ModelSummary {
        node_count: graph.nodes_by_name.len(),
        category_count: graph.categories.len(),
        root_category: graph.root_category.clone(),
    };

    let response = ParseXmlResponse {
        graph: graph.clone(),
        xml: result.xml.clone(),
        diags: vec![],
        summary,
    };

    // Persist in model state.
    model
        .write()
        .await
        .update(result.xml, graph, vec![], Some(device_id.clone()));

    // Emit connection state change.
    let connected_state = ConnectionState::Connected {
        device_id,
        device_name: result.device_name,
        model: result.model,
    };
    let _ = app.emit("connection-state-changed", connected_state);

    Ok(response)
}

#[tauri::command]
pub async fn embedded_disconnect(
    device_id: String,
    backend: State<'_, BackendState>,
    app: AppHandle,
) -> Result<(), String> {
    backend.disconnect(&device_id).await?;
    let _ = app.emit("connection-state-changed", ConnectionState::Disconnected);
    Ok(())
}

#[tauri::command]
pub async fn embedded_get_feature(
    node_name: String,
    backend: State<'_, BackendState>,
) -> Result<NodeValueEntry, String> {
    backend.get_feature(&node_name).await
}

#[tauri::command]
pub async fn embedded_set_feature(
    node_name: String,
    value: serde_json::Value,
    backend: State<'_, BackendState>,
) -> Result<(), String> {
    backend.set_feature(&node_name, &value).await
}

#[tauri::command]
pub async fn embedded_exec_command(
    node_name: String,
    backend: State<'_, BackendState>,
) -> Result<(), String> {
    backend.exec_command(&node_name).await
}

#[tauri::command]
pub async fn embedded_bulk_read(
    names: Vec<String>,
    backend: State<'_, BackendState>,
) -> Result<HashMap<String, NodeValueEntry>, String> {
    backend.bulk_read(&names).await
}

#[tauri::command]
pub async fn embedded_start_acquisition(
    backend: State<'_, BackendState>,
) -> Result<StreamerInfo, String> {
    backend.start_acquisition().await
}

#[tauri::command]
pub async fn embedded_stop_acquisition(backend: State<'_, BackendState>) -> Result<(), String> {
    backend.stop_acquisition().await
}
