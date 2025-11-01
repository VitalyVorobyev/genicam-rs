use std::env;
use std::error::Error;
use std::net::Ipv4Addr;

use tl_gige::nic::Iface;

fn print_usage() {
    eprintln!("usage: grab_gige [--iface <name>] [--auto] [--multicast <ip>] [--port <n>]");
}

fn parse_args() -> Result<(Option<Iface>, bool, Option<Ipv4Addr>, Option<u16>), Box<dyn Error>> {
    let mut args = env::args().skip(1);
    let mut iface = None;
    let mut auto = false;
    let mut multicast = None;
    let mut port = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--iface" => {
                let name = args
                    .next()
                    .ok_or_else(|| "--iface requires an argument".to_string())?;
                iface = Some(Iface::from_system(&name)?);
            }
            "--auto" => auto = true,
            "--multicast" => {
                let ip = args
                    .next()
                    .ok_or_else(|| "--multicast requires an IPv4 address".to_string())?;
                multicast = Some(ip.parse()?);
            }
            "--port" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--port requires a value".to_string())?;
                port = Some(value.parse()?);
            }
            "--help" => {
                print_usage();
                std::process::exit(0);
            }
            other => {
                return Err(format!("unknown argument: {other}").into());
            }
        }
    }

    Ok((iface, auto, multicast, port))
}

fn main() -> Result<(), Box<dyn Error>> {
    let (iface, auto, multicast, port) = parse_args()?;
    println!("GigE stream setup");
    if let Some(iface) = iface {
        println!("  interface: {} (index {})", iface.name(), iface.index());
        if let Some(ip) = iface.ipv4() {
            println!("  interface IPv4: {ip}");
        }
    } else {
        println!("  interface: <system default>");
    }
    println!("  auto packet negotiation: {auto}");
    if let Some(group) = multicast {
        println!("  multicast group: {group}");
    }
    if let Some(port) = port {
        println!("  destination port: {port}");
    }
    println!("Configure the device via StreamBuilder before starting capture.");
    Ok(())
}
