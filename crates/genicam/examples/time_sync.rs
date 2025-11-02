use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use genicam::gige::GVCP_PORT;
use genicam::TimeMapper;
use tokio::sync::Mutex;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    let timeout = Duration::from_millis(500);
    let mut devices = genicam::gige::discover(timeout).await?;
    if devices.is_empty() {
        println!("No devices available for time synchronisation test");
        return Ok(());
    }
    let device = devices.remove(0);
    let addr = SocketAddr::new(IpAddr::V4(device.ip), GVCP_PORT);
    let control = Mutex::new(genicam::gige::GigeDevice::open(addr).await?);
    let mapper = TimeMapper::new(control);
    mapper.reset().await?;
    mapper.calibrate(16, 50).await?;
    let (a, b) = mapper.coefficients().await;
    println!("Linear fit host_time = {a:.6} * ticks + {b:.3}");
    let example_ts = 1_000_000u64;
    let mapped = mapper.map_dev_ts(example_ts).await;
    println!("Example timestamp {example_ts} -> {:?}", mapped);
    Ok(())
}
