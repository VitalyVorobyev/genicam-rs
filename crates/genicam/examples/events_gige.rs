use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use genicam::gige::GVCP_PORT;
use genicam::{bind_event_socket, configure_message_channel, Event, EventStream};

const EVENT_PORT: u16 = 3958;
const EVENT_IP: Ipv4Addr = Ipv4Addr::LOCALHOST;
const EVENTS_TO_PRINT: usize = 5;
const SAMPLE_EVENTS: &[u16] = &[0x9001, 0x9002];

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    let timeout = Duration::from_millis(500);
    let mut devices = genicam::gige::discover(timeout).await?;
    if devices.is_empty() {
        println!("No GigE Vision devices discovered.");
        return Ok(());
    }
    let device = devices.remove(0);
    println!("Using camera at {}", device.ip);
    let control_addr = SocketAddr::new(IpAddr::V4(device.ip), GVCP_PORT);
    let mut camera = genicam::gige::GigeDevice::open(control_addr).await?;
    let socket = bind_event_socket(IpAddr::V4(EVENT_IP), EVENT_PORT).await?;
    configure_message_channel(&mut camera, EVENT_IP, EVENT_PORT, SAMPLE_EVENTS).await?;
    let stream = EventStream::new(socket);

    for idx in 0..EVENTS_TO_PRINT {
        match stream.next().await {
            Ok(Event { id, ts_dev, data }) => {
                println!(
                    "Event #{idx}: id=0x{id:04X}, ts_dev={ts_dev}, payload={} bytes",
                    data.len()
                );
            }
            Err(err) => {
                eprintln!("Failed to receive event: {err}");
                break;
            }
        }
    }

    println!("Event stats: {:?}", stream.stats().snapshot());
    Ok(())
}
