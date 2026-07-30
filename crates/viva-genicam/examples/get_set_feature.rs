//! Read and write a GenApi feature by name on the first camera discovered.
//!
//! ```bash
//! cargo run -p viva-genicam --example get_set_feature
//! cargo run -p viva-genicam --example get_set_feature -- --name Gain --value 3.0
//! ```
//!
//! The equivalent from the command line is `viva-camctl get --ip <IP> --name
//! <FEATURE>` / `viva-camctl set --ip <IP> --name <FEATURE> --value <VALUE>`.

use std::env;
use std::time::Duration;

use viva_genicam::{connect_gige, gige};

fn parse_args() -> (String, Option<String>) {
    let mut args = env::args().skip(1);
    let mut name = "ExposureTime".to_string();
    let mut value: Option<String> = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--name" => {
                if let Some(next) = args.next() {
                    name = next;
                }
            }
            "--value" => {
                value = args.next();
            }
            _ => {}
        }
    }
    (name, value)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    let (name, value) = parse_args();

    let mut devices = gige::discover(Duration::from_millis(500)).await?;
    if devices.is_empty() {
        println!("No cameras found.");
        return Ok(());
    }
    let device = devices.remove(0);
    println!("Connecting to {} ...", device.ip);

    // ANCHOR: get_set
    // `connect_gige` fetches the GenApi XML and builds the NodeMap, so every
    // feature the camera declares is addressable by name from here on.
    let mut camera = connect_gige(&device).await?;

    // Feature access is synchronous even inside an async program: the register
    // I/O behind it blocks, and `GigeRegisterIo` steps off the async worker on
    // its own rather than making every caller do it.
    println!("{name} = {}", camera.get(&name)?);

    if let Some(value) = value.as_deref() {
        camera.set(&name, value)?;
        println!("{name} = {} (after write)", camera.get(&name)?);
    }
    // ANCHOR_END: get_set

    // An enumeration knows which values it will accept; anything else answers
    // with an error, which is the cheapest way to ask "is this an enum?".
    if let Ok(entries) = camera.enum_entries(&name) {
        println!("allowed values: {}", entries.join(", "));
    }

    // Access mode is evaluated live, so `pIsLocked` and `pIsAvailable` are
    // already accounted for — a feature reported RO here will refuse a write.
    if let Some(node) = camera.nodemap().node(&name) {
        println!("kind: {}", node.kind_name());
    }

    Ok(())
}
