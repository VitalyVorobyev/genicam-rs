//! `viva-camctl report` — one command that produces everything we ask for.
//!
//! Three issues, three real defects, and in every one of them the diagnosis
//! came from an artifact the reporter had to assemble by hand: a debug log, a
//! register dump, the GenApi XML. The maintainer has no cameras, so a report
//! is not a support burden — it is the only evidence this project can obtain,
//! and the cost of producing one is the rate limit on fixing bugs.
//!
//! So the report gathers all of it in one pass and keeps going when a step
//! fails: a camera we cannot open is precisely the camera worth reporting, and
//! a bundle that aborts at the first error would describe nothing. Every
//! section records either its findings or why it has none.
//!
//! Output is plain text, in one file, because that is what an issue tracker
//! accepts as an attachment.

use std::fmt::Write as _;
use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use tokio::sync::Mutex;
use viva_genapi_xml::XmlModel;
use viva_genicam::genapi::NodeMap;
use viva_gige::DeviceInfo;
use viva_gige::gvcp::{GigeDevice, consts};
use viva_gige::nic::{Iface, IfaceSelector};

use crate::common;

/// How to render a register's value beside its raw hex.
///
/// The raw word is always printed — a report exists to carry facts, and a
/// decoding we got wrong must not be the only thing it says.
#[derive(Clone, Copy)]
enum Fmt {
    /// Hex only: bitmasks, where a decimal reading tells nobody anything.
    Bits,
    Dec,
    Ipv4,
}

/// Bootstrap registers worth dumping, as `(address, name, format)`.
///
/// Chosen for diagnostic value rather than completeness: the identity block,
/// the capability words, the IP configuration that #57 turned on, and the
/// channel registers whose addresses we had wrong until TC-13.
const BOOTSTRAP: &[(u64, &str, Fmt)] = &[
    (0x0000, "Version", Fmt::Bits),
    (0x0004, "DeviceMode", Fmt::Bits),
    (0x0008, "DeviceMACAddressHigh", Fmt::Bits),
    (0x000C, "DeviceMACAddressLow", Fmt::Bits),
    (0x0010, "SupportedIPConfiguration", Fmt::Bits),
    (
        consts::CURRENT_IP_CONFIG,
        "CurrentIPConfiguration",
        Fmt::Bits,
    ),
    (0x0024, "CurrentIPAddress", Fmt::Ipv4),
    (0x0034, "CurrentSubnetMask", Fmt::Ipv4),
    (0x0044, "CurrentDefaultGateway", Fmt::Ipv4),
    (
        consts::PERSISTENT_IP_ADDRESS,
        "PersistentIPAddress",
        Fmt::Ipv4,
    ),
    (
        consts::PERSISTENT_SUBNET_MASK,
        "PersistentSubnetMask",
        Fmt::Ipv4,
    ),
    (
        consts::PERSISTENT_DEFAULT_GATEWAY,
        "PersistentDefaultGateway",
        Fmt::Ipv4,
    ),
    (0x0900, "GevNumberOfMessageChannels", Fmt::Dec),
    (0x0904, "GevNumberOfStreamChannels", Fmt::Dec),
    (0x0938, "GevHeartbeatTimeout", Fmt::Dec),
    (consts::CONTROL_CHANNEL_PRIVILEGE, "GevCCP", Fmt::Bits),
    (consts::MESSAGE_DESTINATION_PORT, "GevMCP", Fmt::Dec),
    (consts::MESSAGE_DESTINATION_ADDRESS, "GevMCDA", Fmt::Ipv4),
    (consts::MESSAGE_CHANNEL_TIMEOUT, "GevMCTT", Fmt::Dec),
    (consts::MESSAGE_CHANNEL_RETRY_COUNT, "GevMCRC", Fmt::Dec),
    (
        consts::STREAM_CHANNEL_BASE + consts::STREAM_DESTINATION_PORT,
        "GevSCPHostPort[0]",
        Fmt::Dec,
    ),
    (
        consts::STREAM_CHANNEL_BASE + consts::STREAM_PACKET_SIZE,
        "GevSCPSPacketSize[0]",
        Fmt::Bits,
    ),
    (
        consts::STREAM_CHANNEL_BASE + consts::STREAM_PACKET_DELAY,
        "GevSCPD[0]",
        Fmt::Dec,
    ),
    (
        consts::STREAM_CHANNEL_BASE + consts::STREAM_DESTINATION_ADDRESS,
        "GevSCDA[0]",
        Fmt::Ipv4,
    ),
];

fn decode(value: u32, fmt: Fmt) -> String {
    match fmt {
        Fmt::Bits => String::new(),
        Fmt::Dec => value.to_string(),
        Fmt::Ipv4 => Ipv4Addr::from(value).to_string(),
    }
}

pub struct ReportArgs {
    pub ip: Option<Ipv4Addr>,
    pub index: Option<usize>,
    pub iface: Option<IfaceSelector>,
    pub out: Option<PathBuf>,
    pub timeout_ms: u64,
    /// Omit the GenApi XML. It is the single most useful artifact, so this is
    /// opt-out rather than opt-in.
    pub no_xml: bool,
}

pub async fn run(args: ReportArgs) -> Result<()> {
    let mut out = String::new();
    let _ = writeln!(out, "# viva-genicam diagnostic report");
    let _ = writeln!(out);

    environment(&mut out);
    interfaces(&mut out);
    let devices = discovery(&mut out, args.iface.as_ref(), args.timeout_ms).await;
    camera(&mut out, &args, &devices).await;

    let _ = writeln!(out, "## End of report");

    match args.out {
        Some(path) => {
            std::fs::write(&path, out.as_bytes())
                .with_context(|| format!("write {}", path.display()))?;
            eprintln!(
                "wrote {} bytes to {}\n\nAttach this file to \
                 https://github.com/VitalyVorobyev/viva-genicam/issues",
                out.len(),
                path.display()
            );
        }
        None => print!("{out}"),
    }
    Ok(())
}

fn environment(out: &mut String) {
    let _ = writeln!(out, "## Environment");
    let _ = writeln!(out);
    let _ = writeln!(out, "viva-camctl: {}", env!("CARGO_PKG_VERSION"));
    let _ = writeln!(
        out,
        "host:        {} {}",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    let _ = writeln!(out);
}

fn interfaces(out: &mut String) {
    let _ = writeln!(out, "## Network interfaces");
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "As the library sees them. An interface missing here is invisible to \
         discovery no matter what the OS reports elsewhere (#57)."
    );
    let _ = writeln!(out);
    // `mtu()` only queries the OS on Linux and returns 1500 everywhere else
    // (TC-11). Printing that as though it had been measured would put a
    // fabricated number in a document whose whole purpose is evidence.
    let mtu_measured = cfg!(target_os = "linux");
    match Iface::list() {
        Ok(ifaces) if ifaces.is_empty() => {
            let _ = writeln!(out, "(none reported)");
        }
        Ok(ifaces) => {
            for iface in ifaces {
                let addrs = iface.all_ipv4().unwrap_or_default();
                let addrs = if addrs.is_empty() {
                    "-".to_string()
                } else {
                    addrs
                        .iter()
                        .map(|ip| ip.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                };
                let mtu = if mtu_measured {
                    viva_gige::nic::mtu(&iface)
                        .map(|mtu| mtu.to_string())
                        .unwrap_or_else(|err| format!("unknown ({err})"))
                } else {
                    "assumed".to_string()
                };
                let _ = writeln!(
                    out,
                    "{:<16} index={:<5} mtu={:<9} ipv4=[{}]",
                    iface.name(),
                    iface.index(),
                    mtu,
                    addrs
                );
            }
            if !mtu_measured {
                let _ = writeln!(
                    out,
                    "\nMTU is not queried on {} — the library assumes 1500 and \
                     cannot select jumbo frames here (TC-11).",
                    std::env::consts::OS
                );
            }
        }
        Err(err) => {
            let _ = writeln!(out, "FAILED to enumerate interfaces: {err}");
        }
    }
    let _ = writeln!(out);
}

async fn discovery(
    out: &mut String,
    iface: Option<&IfaceSelector>,
    timeout_ms: u64,
) -> Vec<DeviceInfo> {
    let _ = writeln!(out, "## Discovery");
    let _ = writeln!(out);
    let timeout = Duration::from_millis(timeout_ms);
    match common::discover_devices(timeout, iface).await {
        Ok(devices) if devices.is_empty() => {
            let _ = writeln!(out, "no cameras answered within {timeout_ms} ms");
            let _ = writeln!(out);
            Vec::new()
        }
        Ok(devices) => {
            for (index, dev) in devices.iter().enumerate() {
                let _ = writeln!(out, "[{index}] {}", dev.ip);
                let _ = writeln!(out, "     mac:          {}", dev.mac_string());
                let _ = writeln!(out, "     manufacturer: {}", opt(&dev.manufacturer));
                let _ = writeln!(out, "     model:        {}", opt(&dev.model));
                let _ = writeln!(out, "     version:      {}", opt(&dev.version));
                let _ = writeln!(out, "     serial:       {}", opt(&dev.serial));
                let _ = writeln!(out, "     user name:    {}", opt(&dev.user_name));
            }
            let _ = writeln!(out);
            devices
        }
        Err(err) => {
            let _ = writeln!(out, "FAILED: {err:#}");
            let _ = writeln!(out);
            Vec::new()
        }
    }
}

async fn camera(out: &mut String, args: &ReportArgs, discovered: &[DeviceInfo]) {
    let _ = writeln!(out, "## Camera");
    let _ = writeln!(out);

    let device = match select(args, discovered) {
        Some(device) => device,
        None => {
            let _ = writeln!(
                out,
                "No camera selected. Pass --ip or --index to include register, \
                 XML and feature sections."
            );
            let _ = writeln!(out);
            return;
        }
    };
    let _ = writeln!(out, "selected: {}", device.ip);
    let _ = writeln!(out);

    let control = match common::open_control(&device).await {
        Ok(control) => control,
        Err(err) => {
            let _ = writeln!(out, "FAILED to open the control channel: {err:#}");
            let _ = writeln!(out);
            return;
        }
    };
    let control = Arc::new(Mutex::new(control));

    bootstrap_registers(out, &control).await;

    let xml = match common::fetch_xml(Arc::clone(&control)).await {
        Ok(xml) => xml,
        Err(err) => {
            let _ = writeln!(out, "## GenApi XML");
            let _ = writeln!(out);
            let _ = writeln!(out, "FAILED to fetch: {err:#}");
            let _ = writeln!(out);
            return;
        }
    };
    genapi(out, &xml, args.no_xml);
}

fn select(args: &ReportArgs, discovered: &[DeviceInfo]) -> Option<DeviceInfo> {
    if let Some(ip) = args.ip {
        // Falling back to a bare address matters here: a camera that discovery
        // cannot see may still answer on a known IP, and that gap is itself
        // worth reporting.
        return Some(
            discovered
                .iter()
                .find(|dev| dev.ip == ip)
                .cloned()
                .unwrap_or_else(|| DeviceInfo::from_ip(ip)),
        );
    }
    args.index.and_then(|index| discovered.get(index).cloned())
}

async fn bootstrap_registers(out: &mut String, control: &Arc<Mutex<GigeDevice>>) {
    let _ = writeln!(out, "## Bootstrap registers");
    let _ = writeln!(out);
    let mut guard = control.lock().await;
    for (addr, name, fmt) in BOOTSTRAP {
        let value = match guard.read_register(*addr as u32).await {
            Ok(value) => format!("0x{value:08X}  {}", decode(value, *fmt)),
            Err(err) => format!("read failed: {err}"),
        };
        let _ = writeln!(out, "0x{addr:04X}  {name:<26} {}", value.trim_end());
    }
    let _ = writeln!(out);
}

fn genapi(out: &mut String, xml: &str, no_xml: bool) {
    let _ = writeln!(out, "## GenApi");
    let _ = writeln!(out);
    let _ = writeln!(out, "XML size: {} bytes", xml.len());

    let model: Option<XmlModel> = match viva_genapi_xml::parse(xml) {
        Ok(model) => {
            let _ = writeln!(
                out,
                "schema:   {}\nparsed:   {} nodes",
                model.version,
                model.nodes.len()
            );
            Some(model)
        }
        Err(err) => {
            let _ = writeln!(out, "PARSE FAILED: {err}");
            None
        }
    };

    if let Some(model) = model {
        match NodeMap::try_from_xml(model) {
            Ok(nodemap) => {
                let _ = writeln!(out, "built:    {} features", nodemap.node_names().count());
                let _ = writeln!(out);
                let skipped = nodemap.skipped();
                if skipped.is_empty() {
                    let _ = writeln!(out, "No features were dropped.");
                } else {
                    let _ = writeln!(
                        out,
                        "{} feature(s) this camera has that we cannot expose — \
                         these are the interesting ones:",
                        skipped.len()
                    );
                    let _ = writeln!(out);
                    for node in skipped {
                        let _ = writeln!(
                            out,
                            "  <{}> {}: {}",
                            node.tag,
                            node.name.as_deref().unwrap_or("<unnamed>"),
                            node.error
                        );
                    }
                }
            }
            Err(err) => {
                let _ = writeln!(out, "NODEMAP BUILD FAILED: {err}");
            }
        }
    }
    let _ = writeln!(out);

    if no_xml {
        let _ = writeln!(
            out,
            "## GenApi XML\n\nOmitted (--no-xml). Re-run without it, or use \
             `viva-camctl xml`, if asked for the document."
        );
    } else {
        let _ = writeln!(out, "## GenApi XML");
        let _ = writeln!(out);
        let _ = writeln!(out, "{xml}");
    }
    let _ = writeln!(out);
}

fn opt(value: &Option<String>) -> &str {
    value.as_deref().unwrap_or("-")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bootstrap_addresses_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for (addr, name, _) in BOOTSTRAP {
            assert!(seen.insert(*addr), "duplicate address for {name}");
        }
    }

    #[test]
    fn ip_registers_decode_to_dotted_quads() {
        assert_eq!(decode(0x7F00_0001, Fmt::Ipv4), "127.0.0.1");
        assert_eq!(decode(0xFF00_0000, Fmt::Ipv4), "255.0.0.0");
        assert_eq!(decode(3000, Fmt::Dec), "3000");
        assert_eq!(decode(0x8000_0001, Fmt::Bits), "");
    }

    /// A camera we cannot understand is the one worth reporting, so the
    /// GenApi section must describe the failure rather than abort the report.
    #[test]
    fn unparsable_xml_still_produces_a_section() {
        let mut out = String::new();
        genapi(&mut out, "<not-genicam>", true);
        assert!(out.contains("PARSE FAILED"), "{out}");
        assert!(out.contains("XML size: 13 bytes"), "{out}");
    }

    #[test]
    fn skipped_features_are_listed() {
        const XML: &str = r#"
            <RegisterDescription SchemaMajorVersion="1" SchemaMinorVersion="1" SchemaSubMinorVersion="0">
                <ConfRom Name="DeviceConfRom">
                    <Address>0x2000</Address>
                    <Length>512</Length>
                </ConfRom>
            </RegisterDescription>
        "#;
        let mut out = String::new();
        genapi(&mut out, XML, true);
        assert!(out.contains("<ConfRom> DeviceConfRom"), "{out}");
        assert!(out.contains("cannot expose"), "{out}");
    }

    #[test]
    fn the_xml_is_included_unless_opted_out() {
        const XML: &str = r#"<RegisterDescription SchemaMajorVersion="1" SchemaMinorVersion="1" SchemaSubMinorVersion="0"/>"#;
        let mut with = String::new();
        genapi(&mut with, XML, false);
        assert!(with.contains("SchemaMajorVersion"), "{with}");

        let mut without = String::new();
        genapi(&mut without, XML, true);
        assert!(without.contains("Omitted (--no-xml)"), "{without}");
    }
}
