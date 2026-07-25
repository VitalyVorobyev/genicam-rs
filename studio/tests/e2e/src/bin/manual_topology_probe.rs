use std::sync::Arc;
use std::time::Duration;

use e2e_tests::{send_acquisition_command, workspace_path};
use tokio::time::timeout;
use viva_zenoh_api::{AcquisitionCommand, DeviceAnnounce, FrameHeader, HEADER_SIZE};

fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "manual_topology_probe=info,e2e_tests=info,warn".into()),
        )
        .try_init();
}

fn env_or_default(name: &str, default: String) -> String {
    std::env::var(name).unwrap_or(default)
}

async fn discover_device_id(
    session: &Arc<zenoh::Session>,
    timeout_secs: u64,
) -> Result<String, String> {
    let sub = session
        .declare_subscriber(viva_zenoh_api::keys::ANNOUNCE_ALL)
        .await
        .map_err(|e| format!("declare announce subscriber: {e}"))?;

    let sample = timeout(Duration::from_secs(timeout_secs), sub.recv_async())
        .await
        .map_err(|_| format!("timeout waiting for announce after {timeout_secs}s"))?
        .map_err(|e| format!("announce recv: {e}"))?;

    let announce: DeviceAnnounce = serde_json::from_slice(&sample.payload().to_bytes())
        .map_err(|e| format!("parse announce: {e}"))?;

    tracing::info!(
        device_id = %announce.id,
        model = %announce.model,
        name = %announce.name,
        "Discovered device"
    );

    Ok(announce.id)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    init_tracing();

    let timeout_secs = std::env::var("PROBE_TIMEOUT_SECS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(10);
    let client_config = env_or_default(
        "GENICAM_CLIENT_ZENOH_CONFIG",
        workspace_path("config/zenoh-studio.json5")
            .to_string_lossy()
            .into_owned(),
    );

    let session = Arc::new(zenoh::open(zenoh::Config::from_file(&client_config)?).await?);
    tracing::info!(config = %client_config, "Opened client Zenoh session");

    let device_id = match std::env::var("PROBE_DEVICE_ID") {
        Ok(id) => id,
        Err(_) => discover_device_id(&session, timeout_secs).await?,
    };

    let image_key = viva_zenoh_api::keys::image(&device_id);
    let meta_key = viva_zenoh_api::keys::image_meta(&device_id);
    let meta_sub = session.declare_subscriber(&meta_key).await?;
    let image_sub = session.declare_subscriber(&image_key).await?;

    // Give subscriber interest a moment to propagate before AcquisitionStart.
    tokio::time::sleep(Duration::from_millis(500)).await;

    send_acquisition_command(&session, &device_id, AcquisitionCommand::Start)
        .await
        .map_err(|e| format!("AcquisitionStart failed: {e}"))?;
    tracing::info!(device_id = %device_id, "Acquisition started");

    let meta_sample = timeout(Duration::from_secs(timeout_secs), meta_sub.recv_async())
        .await
        .map_err(|_| format!("timeout waiting for image/meta after {timeout_secs}s"))?
        .map_err(|e| format!("image/meta recv: {e}"))?;
    let meta: viva_zenoh_api::ImageMeta = serde_json::from_slice(&meta_sample.payload().to_bytes())
        .map_err(|e| format!("parse image/meta: {e}"))?;
    tracing::info!(
        width = meta.width,
        height = meta.height,
        pixel_format = ?meta.pixel_format,
        payload_size = meta.payload_size,
        "Received image/meta"
    );

    let image_sample = timeout(Duration::from_secs(timeout_secs), image_sub.recv_async())
        .await
        .map_err(|_| format!("timeout waiting for raw image after {timeout_secs}s"))?
        .map_err(|e| format!("image recv: {e}"))?;
    let image_bytes = image_sample.payload().to_bytes();
    if image_bytes.len() <= HEADER_SIZE {
        return Err(format!("raw image sample too small: {} bytes", image_bytes.len()).into());
    }

    let (header, pixel_data) =
        FrameHeader::decode(&image_bytes).map_err(|e| format!("decode frame header: {e}"))?;
    tracing::info!(
        seq = header.seq,
        width = header.width,
        height = header.height,
        pixel_format = ?header.pixel_format,
        payload = pixel_data.len(),
        "Received raw image frame"
    );

    send_acquisition_command(&session, &device_id, AcquisitionCommand::Stop)
        .await
        .map_err(|e| format!("AcquisitionStop failed: {e}"))?;
    tracing::info!(device_id = %device_id, "Acquisition stopped");

    Ok(())
}
