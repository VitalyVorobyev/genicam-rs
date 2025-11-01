use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use genapi_xml::{self, XmlError};
use tl_gige::{self, GVCP_PORT};
use tokio::sync::Mutex;

fn format_mac(mac: &[u8; 6]) -> String {
    mac.iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(":")
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    let timeout = Duration::from_millis(500);
    let mut devices = tl_gige::discover(timeout).await?;
    if devices.is_empty() {
        println!("No cameras found.");
        return Ok(());
    }
    let device = devices.remove(0);
    println!("Connecting to {} ({})", device.ip, format_mac(&device.mac));
    let addr = SocketAddr::new(IpAddr::V4(device.ip), GVCP_PORT);
    let camera = Arc::new(Mutex::new(tl_gige::GigeDevice::open(addr).await?));

    let xml = {
        let cam = Arc::clone(&camera);
        genapi_xml::fetch_and_load_xml(move |address, length| {
            let cam = Arc::clone(&cam);
            async move {
                let mut guard = cam.lock().await;
                guard
                    .read_mem(address, length)
                    .await
                    .map_err(|err| XmlError::Transport(err.to_string()))
            }
        })
        .await?
    };
    println!("Fetched XML ({} bytes)", xml.len());
    let meta = genapi_xml::parse_into_minimal_nodes(&xml)?;
    if let Some(version) = meta.schema_version.as_deref() {
        println!("Schema version: {version}");
    }
    println!("Top level features ({}):", meta.top_level_features.len());
    for feature in meta.top_level_features.iter().take(8) {
        println!("  - {feature}");
    }
    if meta.top_level_features.len() > 8 {
        println!("  ... ({} more)", meta.top_level_features.len() - 8);
    }

    const DEVICE_VENDOR_NAME_REG: u64 = 0x0000_0000_0000_0048;
    println!(
        "Stub: would map register 0x{DEVICE_VENDOR_NAME_REG:016X} to a GenApi feature for DeviceVendorName"
    );
    println!("       -> read via camera.read_mem(...) and expose as string node");

    Ok(())
}
