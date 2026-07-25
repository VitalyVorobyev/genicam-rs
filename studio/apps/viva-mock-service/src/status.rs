use std::sync::Arc;

use tokio::sync::watch;
use tracing::error;

use viva_zenoh_api::DeviceStatus;

use crate::config::MockConfig;

pub async fn run(
    session: Arc<zenoh::Session>,
    config: MockConfig,
    mut shutdown: watch::Receiver<bool>,
) {
    let key = viva_zenoh_api::keys::status(&config.device_id);
    let status = DeviceStatus {
        connected: true,
        error: None,
    };
    let payload = match serde_json::to_vec(&status) {
        Ok(p) => p,
        Err(e) => {
            error!("Failed to serialize DeviceStatus: {e}");
            return;
        }
    };

    // Publish connected status at startup
    if let Err(e) = session.put(&key, payload.clone()).await {
        error!("Failed to publish initial status: {e}");
        return;
    }

    // Keep alive until shutdown, then publish disconnected status
    loop {
        tokio::select! {
            _ = shutdown.changed() => {
                if *shutdown.borrow() {
                    let disconnected = DeviceStatus {
                        connected: false,
                        error: None,
                    };
                    if let Ok(payload) = serde_json::to_vec(&disconnected) {
                        let _ = session.put(&key, payload).await;
                    }
                    break;
                }
            }
        }
    }
}
