use std::sync::Arc;

use bytes::Bytes;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::watch;

use crate::error::HumanizeExt;
use crate::state::device_state::{ConnectionState, StreamerInfo, ZenohState};
use viva_streamer::meta;
use viva_zenoh_api::{
    AcquisitionCommand, AcquisitionControlRequest, AcquisitionStatus, NodeOpResponse,
};

// ── IPC Commands ──────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_acquisition_status(
    zenoh: State<'_, Arc<ZenohState>>,
) -> Result<AcquisitionStatus, String> {
    Ok(zenoh.acquisition.lock().await.status.clone())
}

#[tauri::command]
pub async fn start_acquisition(
    zenoh: State<'_, Arc<ZenohState>>,
    backend: State<'_, crate::backend::BackendState>,
    app: AppHandle,
) -> Result<StreamerInfo, String> {
    if matches!(backend.mode(), crate::backend::BackendMode::Embedded) {
        let info = backend.start_acquisition().await?;
        let mut acq = zenoh.acquisition.lock().await;
        acq.ws_url = Some(info.ws_url.clone());
        acq.width = info.width;
        acq.height = info.height;
        acq.status.active = true;
        let _ = app.emit(
            "acquisition-status",
            AcquisitionStatus {
                active: true,
                fps: None,
                dropped: 0,
            },
        );
        return Ok(info);
    }

    let session = zenoh.get_session().await.humanize()?;
    let device_id = connected_device_id(&zenoh).await.humanize()?;

    // Read image dimensions from cached node values (Width / Height SFNC nodes).
    // The camera service may publish values as JSON strings ("640") or numbers (640),
    // so we handle both forms.
    let (width, height) = {
        let cache = zenoh.node_cache.read().await;
        let w = cache
            .get("Width")
            .and_then(|e| value_as_u32(&e.value))
            .unwrap_or(640);
        let h = cache
            .get("Height")
            .and_then(|e| value_as_u32(&e.value))
            .unwrap_or(480);
        (w, h)
    };

    let image_key = viva_zenoh_api::keys::image(&device_id);
    let meta_key = meta::derive_meta_key(&image_key);

    // Stop any previously running streamer before starting a new one.
    stop_streamer_tasks(&zenoh).await;

    // Create shared state for the embedded streamer
    let (frame_tx, _) = watch::channel(Bytes::new());
    let info_tx = watch::channel(viva_streamer::ws::StreamInfo::from_image_meta(
        &meta::default_image_meta(width, height),
    ))
    .0;
    let shared_meta = Arc::new(tokio::sync::RwLock::new(meta::default_image_meta(
        width, height,
    )));
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    // Bind WS server to :0 (auto-assign port)
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| format!("Failed to bind WS listener: {e}"))?;
    let port = listener
        .local_addr()
        .map_err(|e| format!("Failed to get local address: {e}"))?
        .port();
    let ws_url = format!("ws://127.0.0.1:{port}/ws");

    tracing::info!("Embedded streamer WS on {ws_url}");

    // Spawn Zenoh source task BEFORE sending Start command to the service.
    // This ensures the subscriber is ready to receive frames when the
    // service begins publishing — avoids a race condition where early
    // frames are dropped because no subscriber exists yet.
    let source_config = viva_streamer::zenoh_source::ZenohSourceConfig {
        key_expr: image_key,
        meta_key,
        fps_limit: Some(15),
    };
    let zenoh_handle = tauri::async_runtime::spawn({
        let session = session.clone();
        let shared_meta = shared_meta.clone();
        let frame_tx = frame_tx.clone();
        let info_tx = info_tx.clone();
        let shutdown_rx = shutdown_rx.clone();
        async move {
            if let Err(e) = viva_streamer::zenoh_source::run_with_session(
                session,
                source_config,
                shared_meta,
                frame_tx,
                info_tx,
                shutdown_rx,
            )
            .await
            {
                tracing::error!("Zenoh source task error: {e}");
            }
        }
    });

    // Spawn WS server task
    let ws_handle = tauri::async_runtime::spawn({
        let state = viva_streamer::ws::AppState { frame_tx, info_tx };
        let shutdown_rx = shutdown_rx.clone();
        async move {
            if let Err(e) = viva_streamer::ws::run_server_with_listener(
                listener,
                "/ws".to_string(),
                state,
                shutdown_rx,
            )
            .await
            {
                tracing::error!("WS server task error: {e}");
            }
        }
    });

    // Store handles for cleanup
    {
        let mut acq = zenoh.acquisition.lock().await;
        acq.shutdown_tx = Some(shutdown_tx);
        acq.task_handles = vec![zenoh_handle, ws_handle];
        acq.ws_url = Some(ws_url.clone());
        acq.width = width;
        acq.height = height;
        acq.status.active = true;
    }

    // Allow Zenoh subscription interest to propagate to the service peer
    // before telling it to start streaming. Without this, the service may
    // publish frames that no subscriber is ready to receive.
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // NOW send start command — subscriber is ready to receive frames.
    if let Err(e) = send_acquisition_command(&session, &device_id, AcquisitionCommand::Start).await
    {
        // Roll back: stop the streamer tasks we just spawned.
        stop_streamer_tasks(&zenoh).await;
        return Err(e);
    }

    let _ = app.emit(
        "acquisition-status",
        AcquisitionStatus {
            active: true,
            fps: None,
            dropped: 0,
        },
    );

    Ok(StreamerInfo {
        ws_url,
        width,
        height,
    })
}

#[tauri::command]
pub async fn stop_acquisition(
    zenoh: State<'_, Arc<ZenohState>>,
    backend: State<'_, crate::backend::BackendState>,
    app: AppHandle,
) -> Result<(), String> {
    if matches!(backend.mode(), crate::backend::BackendMode::Embedded) {
        backend.stop_acquisition().await?;
        let mut acq = zenoh.acquisition.lock().await;
        acq.status.active = false;
        acq.ws_url = None;
        let _ = app.emit(
            "acquisition-status",
            AcquisitionStatus {
                active: false,
                fps: None,
                dropped: 0,
            },
        );
        return Ok(());
    }

    let session = zenoh.get_session().await.humanize()?;
    let device_id = connected_device_id(&zenoh).await.humanize()?;

    // Signal device service to stop — best-effort, don't block on error
    let _ = send_acquisition_command(&session, &device_id, AcquisitionCommand::Stop).await;

    stop_streamer_tasks(&zenoh).await;

    let _ = app.emit(
        "acquisition-status",
        AcquisitionStatus {
            active: false,
            fps: None,
            dropped: 0,
        },
    );

    Ok(())
}

// ── Streamer lifecycle ───────────────────────────────────────────────────────

/// Signal embedded streamer tasks to shut down and wait for cleanup.
pub async fn stop_streamer_tasks(zenoh: &ZenohState) {
    let mut acq = zenoh.acquisition.lock().await;
    if let Some(tx) = acq.shutdown_tx.take() {
        let _ = tx.send(true);
    }
    for handle in acq.task_handles.drain(..) {
        handle.abort();
    }
    acq.ws_url = None;
    acq.status.active = false;
}

// ── Helpers ───────────────────────────────────────────────────────────────────

async fn connected_device_id(zenoh: &ZenohState) -> Result<String, String> {
    match zenoh.connection.lock().await.clone() {
        ConnectionState::Connected { device_id, .. } => Ok(device_id),
        _ => Err("Not connected to a device".to_string()),
    }
}

/// Extract a u32 from a JSON value that may be a number or a numeric string.
fn value_as_u32(v: &serde_json::Value) -> Option<u32> {
    v.as_u64()
        .map(|n| n as u32)
        .or_else(|| v.as_f64().map(|f| f as u32))
        .or_else(|| v.as_str().and_then(|s| s.parse::<u32>().ok()))
}

async fn send_acquisition_command(
    session: &zenoh::Session,
    device_id: &str,
    command: AcquisitionCommand,
) -> Result<(), String> {
    let key = viva_zenoh_api::keys::acquisition_control(device_id);
    let payload =
        serde_json::to_vec(&AcquisitionControlRequest { command }).map_err(|e| e.to_string())?;

    let replies = session
        .get(&key)
        .payload(payload)
        .timeout(std::time::Duration::from_secs(5))
        .await
        .map_err(|e| format!("Zenoh error: {e}"))?;

    match replies.recv_async().await {
        Ok(reply) => match reply.result() {
            Ok(sample) => {
                let bytes = sample.payload().to_bytes();
                let resp: NodeOpResponse =
                    serde_json::from_slice(&bytes).map_err(|e| format!("Parse error: {e}"))?;
                if resp.ok {
                    Ok(())
                } else {
                    Err(resp
                        .error
                        .unwrap_or_else(|| "Acquisition command failed".to_string()))
                }
            }
            Err(e) => Err(format!("Reply error: {e}")),
        },
        Err(_) => Err("No reply for acquisition command (timeout)".to_string()),
    }
}
