use std::sync::Arc;

use tokio::sync::watch;
use tracing::{debug, error};

use viva_zenoh_api::DeviceXmlResponse;

use crate::config::MockConfig;

pub async fn run(
    session: Arc<zenoh::Session>,
    config: MockConfig,
    xml: String,
    mut shutdown: watch::Receiver<bool>,
) {
    let key = viva_zenoh_api::keys::xml(&config.device_id);
    let queryable = match session.declare_queryable(&key).await {
        Ok(q) => q,
        Err(e) => {
            error!("Failed to declare XML queryable: {e}");
            return;
        }
    };
    let response = DeviceXmlResponse { xml };
    let payload = match serde_json::to_vec(&response) {
        Ok(p) => p,
        Err(e) => {
            error!("Failed to serialize XML response: {e}");
            return;
        }
    };

    loop {
        tokio::select! {
            _ = shutdown.changed() => {
                if *shutdown.borrow() { break; }
            }
            query = queryable.recv_async() => {
                match query {
                    Ok(query) => {
                        debug!("Serving XML for {}", config.device_id);
                        let _ = query.reply(&key, payload.clone()).await;
                    }
                    Err(e) => {
                        error!("XML queryable recv error: {e}");
                        break;
                    }
                }
            }
        }
    }
}
