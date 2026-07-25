use std::path::PathBuf;
use std::sync::Arc;

use tauri::State;

use crate::error::HumanizeExt;
use crate::state::device_state::{ConnectionState, ZenohState};
use viva_streamer::recording::{self, GsrHeader, RecordingStatus};

#[tauri::command]
pub async fn start_recording(zenoh: State<'_, Arc<ZenohState>>) -> Result<RecordingStatus, String> {
    let device_id = connected_device_id(&zenoh).await.humanize()?;

    let mut acq = zenoh.acquisition.lock().await;
    if acq.recording.is_some() {
        return Err("Recording already in progress".to_string());
    }
    if !acq.status.active {
        return Err("Start acquisition before recording".to_string());
    }

    // Determine output directory
    let base_dir = recording_base_dir();
    std::fs::create_dir_all(&base_dir)
        .map_err(|e| format!("Failed to create recording directory: {e}"))?;

    let output_path = recording::default_recording_path(&base_dir, &device_id);

    let pixel_format = acq
        .image_meta
        .as_ref()
        .map(|m| format!("{:?}", m.pixel_format))
        .unwrap_or_else(|| "Mono8".to_string());

    let header = GsrHeader {
        version: 1,
        pixel_format,
        width: acq.width,
        height: acq.height,
        device_id,
        started_at: timestamp_iso8601(),
    };

    let handle = recording::start_recording(output_path, header)
        .map_err(|e| format!("Failed to start recording: {e}"))?;

    let status = handle.status.lock().await.clone();
    acq.recording = Some(handle);

    Ok(status)
}

#[tauri::command]
pub async fn stop_recording(zenoh: State<'_, Arc<ZenohState>>) -> Result<RecordingStatus, String> {
    let mut acq = zenoh.acquisition.lock().await;
    let handle = acq
        .recording
        .take()
        .ok_or_else(|| "No recording in progress".to_string())?;

    // Signal shutdown
    let _ = handle.shutdown_tx.send(true);

    // Wait briefly for the writer to finish
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let status = handle.status.lock().await.clone();
    Ok(status)
}

#[tauri::command]
pub async fn get_recording_status(
    zenoh: State<'_, Arc<ZenohState>>,
) -> Result<RecordingStatus, String> {
    let acq = zenoh.acquisition.lock().await;
    match &acq.recording {
        Some(handle) => Ok(handle.status.lock().await.clone()),
        None => Ok(RecordingStatus::default()),
    }
}

async fn connected_device_id(zenoh: &ZenohState) -> Result<String, String> {
    match zenoh.connection.lock().await.clone() {
        ConnectionState::Connected { device_id, .. } => Ok(device_id),
        _ => Err("Not connected to a device".to_string()),
    }
}

fn recording_base_dir() -> PathBuf {
    let base = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join(".viva-studio").join("recordings")
}

fn timestamp_iso8601() -> String {
    use std::time::SystemTime;
    let dur = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}Z", dur.as_secs())
}
