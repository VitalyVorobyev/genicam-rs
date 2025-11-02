use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use genicam::gige::{action::send_action, GVCP_PORT};
use genicam::{AckSummary, ActionParams};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    let timeout = Duration::from_millis(500);
    let devices = genicam::gige::discover(timeout).await?;
    if devices.is_empty() {
        println!("No devices found for action test");
        return Ok(());
    }

    let broadcast = SocketAddr::new(IpAddr::V4(Ipv4Addr::BROADCAST), GVCP_PORT);
    let params = ActionParams {
        device_key: 0x1122_3344,
        group_key: 0x5566_7788,
        group_mask: 0xFFFF_FFFF,
        scheduled_time: None,
        channel: 0,
    };
    let AckSummary { sent, acks } = send_action(broadcast, &params).await?;
    println!("Action command sent ({sent} datagram), acknowledgements received: {acks}");
    Ok(())
}
