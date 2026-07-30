//! GVCP control plane utilities.

use std::collections::HashMap;
use std::io::Cursor;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use bytes::{Buf, BufMut, Bytes, BytesMut};
use fastrand::Rng;
use if_addrs::{IfAddr, get_if_addrs};
use thiserror::Error;
use tokio::net::UdpSocket;
use tokio::task::JoinSet;
use tokio::time;
use tracing::{debug, info, trace, warn};
use viva_gencp::{AckHeader, CommandFlags, GenCpAck, OpCode, StatusCode, decode_ack};

use crate::nic::{self, Iface};

/// GVCP protocol constants grouped by semantic area.
pub mod consts {
    use std::time::Duration;

    /// GVCP control port as defined by the GigE Vision specification (section 7.3).
    pub const PORT: u16 = 3956;
    /// Opcode of the discovery command.
    pub const DISCOVERY_COMMAND: u16 = 0x0002;
    /// Opcode of the discovery acknowledgement.
    pub const DISCOVERY_ACK: u16 = 0x0003;
    /// Opcode of the FORCEIP command.
    pub const FORCEIP_COMMAND: u16 = 0x0004;
    /// Opcode of the FORCEIP acknowledgement.
    pub const FORCEIP_ACK: u16 = 0x0005;
    /// Opcode for requesting packet resends.
    pub const PACKET_RESEND_COMMAND: u16 = 0x0040;
    /// Opcode of the packet resend acknowledgement.
    pub const PACKET_RESEND_ACK: u16 = 0x0041;
    /// Opcode of the PENDING_ACK acknowledgement (GigE Vision 1.2, section 18.5).
    ///
    /// A device that cannot complete a command within the controller's timeout
    /// answers with this instead of the real acknowledgement, asking for more
    /// time. It is not a GenCP opcode — the U3V side of GenCP signals the same
    /// condition with status `0x8006` — so it is handled in the GVCP layer.
    pub const PENDING_ACK: u16 = 0x0089;

    // ── Event channel and action commands ───────────────────────────────
    //
    // These four opcodes are the ones a device sends *to* us on the message
    // channel, plus the two we broadcast for actions. They live here rather
    // than in `action`/`message` because keeping every GVCP opcode in one
    // table is what makes a collision visible: `ACTION_COMMAND` was 0x0080 —
    // `READ_REGISTER` — for as long as it had its own private constant.
    // GigE Vision 2.0 section 18; corroborated by Wireshark's `packet-gvcp.c`
    // (`GVCP_ACTION_CMD`, `GVCP_EVENT_CMD`, `GVCP_EVENTDATA_CMD`) and, for the
    // register commands it shadowed, `../aravis/src/arvgvcpprivate.h`.

    /// Opcode of the event command sent by a device on the message channel.
    pub const EVENT_COMMAND: u16 = 0x00C0;
    /// Opcode of the acknowledgement a controller returns for an `EVENT_CMD`.
    pub const EVENT_ACK: u16 = 0x00C1;
    /// Opcode of the event command that carries device-specific event data.
    pub const EVENTDATA_COMMAND: u16 = 0x00C2;
    /// Opcode of the acknowledgement returned for an `EVENTDATA_CMD`.
    pub const EVENTDATA_ACK: u16 = 0x00C3;
    /// Opcode of the action command.
    pub const ACTION_COMMAND: u16 = 0x0100;
    /// Opcode of the action acknowledgement.
    pub const ACTION_ACK: u16 = 0x0101;

    /// Size of one event entry in an `EVENT_CMD` with 16-bit block IDs.
    pub const EVENT_ENTRY: usize = 16;
    /// Size of one event entry in an `EVENT_CMD` with 64-bit block IDs.
    ///
    /// GigE Vision 2.0 extended block IDs, signalled by bit 4 of the GVCP
    /// flags byte.
    pub const EVENT_ENTRY_EXTENDED: usize = 24;

    /// Current IP configuration flags register.
    ///
    /// Bit 2 = DHCP, bit 1 = persistent IP, bit 0 = LLA.
    pub const CURRENT_IP_CONFIG: u64 = 0x0014;

    /// Persistent IP address register (4 bytes at the end of a 16-byte block).
    pub const PERSISTENT_IP_ADDRESS: u64 = 0x064C;
    /// Persistent subnet mask register.
    pub const PERSISTENT_SUBNET_MASK: u64 = 0x065C;
    /// Persistent default gateway register.
    pub const PERSISTENT_DEFAULT_GATEWAY: u64 = 0x066C;

    /// Address of the Control Channel Privilege (CCP) register.
    ///
    /// A controller must write `CONTROL_PRIVILEGE` to this register before the
    /// device accepts stream configuration or acquisition commands.
    pub const CONTROL_CHANNEL_PRIVILEGE: u64 = 0x0a00;
    /// CCP value claiming exclusive control.
    pub const CCP_CONTROL: u32 = 1 << 1;
    /// CCP value indicating an exclusive owner.
    pub const CCP_EXCLUSIVE: u32 = 1 << 0;

    // ── Message channel bootstrap registers ─────────────────────────────
    //
    // These sit in the 0x0B00 block, next to CCP at 0x0A00 and the stream
    // channels at 0x0D00. They previously read 0x0900_0200 / 0x0900_0204,
    // which is not a bootstrap address at all: 0x0900 is
    // `GevNumberOfMessageChannels`, and the low half was being used as if it
    // were a base. Every event-channel write therefore landed ~150 MB into
    // the device's register space. Wireshark's `packet-gvcp.c`
    // (`GVCP_MC_DESTINATION_PORT`, `GVCP_MC_DESTINATION_ADDRESS`) gives the
    // real values, and the CCP and stream-channel addresses we already had
    // right corroborate the scheme.

    /// Number of message channels the device implements (`GevNumberOfMessageChannels`).
    pub const NUMBER_OF_MESSAGE_CHANNELS: u64 = 0x0000_0900;
    /// Message channel destination port register (`GevMCP`).
    ///
    /// A 32-bit register; the UDP port occupies the low 16 bits.
    pub const MESSAGE_DESTINATION_PORT: u64 = 0x0000_0B00;
    /// Message channel destination address register (`GevMCDA`).
    pub const MESSAGE_DESTINATION_ADDRESS: u64 = 0x0000_0B10;
    /// Message channel transmission timeout in milliseconds (`GevMCTT`).
    pub const MESSAGE_CHANNEL_TIMEOUT: u64 = 0x0000_0B14;
    /// Message channel retry count (`GevMCRC`).
    pub const MESSAGE_CHANNEL_RETRY_COUNT: u64 = 0x0000_0B18;

    /// Maximum number of bytes we read per GenCP `ReadMem` operation.
    pub const GENCP_MAX_BLOCK: usize = 512;
    /// Additional bytes that accompany a GenCP `WriteMem` block.
    pub const GENCP_WRITE_OVERHEAD: usize = 8;

    /// Default timeout for control transactions.
    pub const CONTROL_TIMEOUT: Duration = Duration::from_millis(500);
    /// Maximum number of automatic retries for a control transaction.
    pub const MAX_RETRIES: usize = 4;
    /// Maximum number of consecutive PENDING_ACKs honoured for one command.
    ///
    /// A device is free to keep asking for more time; this bounds a
    /// misbehaving one that never finishes.
    pub const MAX_PENDING_ACKS: usize = 100;
    /// Ceiling on the extension a single PENDING_ACK may request.
    ///
    /// The field is 16-bit milliseconds, so the wire maximum is ~65 s. Cap it
    /// well below that: an honest device asks for tens or hundreds of
    /// milliseconds, and this keeps a garbage value from hanging the caller.
    pub const MAX_PENDING_ACK_WAIT: Duration = Duration::from_secs(10);
    /// Base delay used for retry backoff.
    pub const RETRY_BASE_DELAY: Duration = Duration::from_millis(20);
    /// Upper bound for the random jitter added to the retry delay (inclusive).
    pub const RETRY_JITTER: Duration = Duration::from_millis(10);

    /// Maximum number of bytes captured while listening for discovery responses.
    pub const DISCOVERY_BUFFER: usize = 2048;

    /// Base register for stream channel 0 (GigE Vision bootstrap register map).
    ///
    /// The GigE Vision specification defines stream channel bootstrap registers
    /// starting at 0x0d00. Note: some cameras may use different offsets declared
    /// in their GenICam XML (e.g. SFNC `GevSCDA` nodes). The bootstrap offsets
    /// here match the aravis implementation and the GigE Vision 2.x standard.
    pub const STREAM_CHANNEL_BASE: u64 = 0x0d00;
    /// Stride in bytes between successive stream channel blocks.
    pub const STREAM_CHANNEL_STRIDE: u64 = 0x40;
    /// Offset for `GevSCPHostPort` within a stream channel block.
    pub const STREAM_DESTINATION_PORT: u64 = 0x00;
    /// Offset for `GevSCPSPacketSize` within a stream channel block.
    pub const STREAM_PACKET_SIZE: u64 = 0x04;
    /// Offset for `GevSCPD` (packet delay) within a stream channel block.
    pub const STREAM_PACKET_DELAY: u64 = 0x08;
    /// Offset for `GevSCDA` (stream destination IP address) within a stream channel block.
    pub const STREAM_DESTINATION_ADDRESS: u64 = 0x18;
}

/// Public alias for the GVCP well-known port.
pub use consts::PORT as GVCP_PORT;

/// GVCP request header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GvcpRequestHeader {
    /// Request flags (acknowledgement, broadcast).
    pub flags: CommandFlags,
    /// Raw command/opcode value.
    pub command: u16,
    /// Payload length in bytes.
    pub length: u16,
    /// Request identifier.
    pub request_id: u16,
}

/// GVCP command message key value (first byte of every GVCP command packet).
const GVCP_CMD_KEY: u8 = 0x42;

impl GvcpRequestHeader {
    /// Encode the header into a `Bytes` buffer ready to be transmitted.
    ///
    /// Uses proper GVCP wire format: byte 0 = `0x42` (command key),
    /// byte 1 = flags byte (bit 0 = ACK_REQUIRED, bit 4 = BROADCAST).
    pub fn encode(self, payload: &[u8]) -> Bytes {
        let mut buf = BytesMut::with_capacity(viva_gencp::HEADER_SIZE + payload.len());
        // GVCP command header: key byte + flags byte (not a u16 flags field).
        buf.put_u8(GVCP_CMD_KEY);
        buf.put_u8(self.gvcp_flags_byte());
        buf.put_u16(self.command);
        buf.put_u16(self.length);
        buf.put_u16(self.request_id);
        buf.extend_from_slice(payload);
        buf.freeze()
    }

    /// Convert `CommandFlags` to the single-byte GVCP flag field.
    ///
    /// Bit 4 is overloaded by the specification: it means "allow broadcast
    /// acknowledge" on `DISCOVERY_CMD` and "64-bit block IDs" on
    /// `PACKETRESEND_CMD`/`EVENT_CMD`/`EVENTDATA_CMD`. We only ever set it for
    /// the former and only ever read it for the latter, so one mapping covers
    /// both.
    fn gvcp_flags_byte(&self) -> u8 {
        let mut byte = 0u8;
        if self.flags.contains(CommandFlags::ACK_REQUIRED) {
            byte |= 0x01;
        }
        if self.flags.contains(CommandFlags::BROADCAST) {
            byte |= 0x10;
        }
        if self.flags.contains(CommandFlags::SCHEDULED_ACTION) {
            byte |= 0x80;
        }
        byte
    }
}

/// GVCP acknowledgement header wrapper.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GvcpAckHeader {
    /// Status reported by the device.
    pub status: StatusCode,
    /// Raw command/opcode value.
    pub command: u16,
    /// Payload length in bytes.
    pub length: u16,
    /// Identifier of the answered request.
    pub request_id: u16,
}

impl From<AckHeader> for GvcpAckHeader {
    fn from(value: AckHeader) -> Self {
        Self {
            status: value.status,
            command: value.opcode.ack_code(),
            length: value.length,
            request_id: value.request_id,
        }
    }
}

/// Errors that can occur when operating the GVCP control path.
#[derive(Debug, Error)]
pub enum GigeError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("protocol: {0}")]
    Protocol(String),
    #[error("timeout waiting for acknowledgement")]
    Timeout,
    #[error("GenCP: {0}")]
    GenCp(#[from] viva_gencp::GenCpError),
    #[error("device reported status {0}")]
    Status(StatusCode),
}

/// Information returned by GVCP discovery packets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceInfo {
    pub ip: Ipv4Addr,
    pub mac: [u8; 6],
    pub model: Option<String>,
    pub manufacturer: Option<String>,
    /// Device version string (Discovery ACK offset 136).
    pub version: Option<String>,
    /// Serial number as printed on the device (Discovery ACK offset 216).
    pub serial: Option<String>,
    /// User-programmable device name (Discovery ACK offset 232).
    pub user_name: Option<String>,
}

impl DeviceInfo {
    /// A minimal record for a device addressed directly by IP.
    ///
    /// Used when the caller names a camera by address and there is no
    /// Discovery ACK to populate the identity fields.
    pub fn from_ip(ip: Ipv4Addr) -> Self {
        Self {
            ip,
            mac: [0; 6],
            model: None,
            manufacturer: None,
            version: None,
            serial: None,
            user_name: None,
        }
    }

    /// Format the MAC address as `AA:BB:CC:DD:EE:FF`.
    pub fn mac_string(&self) -> String {
        self.mac
            .iter()
            .map(|byte| format!("{byte:02X}"))
            .collect::<Vec<_>>()
            .join(":")
    }
}

/// Discover GigE Vision devices on the local network by broadcasting a GVCP discovery command.
pub async fn discover(timeout: Duration) -> Result<Vec<DeviceInfo>, GigeError> {
    discover_impl(timeout, None, false).await
}

/// Discover devices only on the specified interface name.
/// Discover devices only on the specified interface name.
///
/// When a user explicitly names an interface (including loopback like `lo0`),
/// it is always included — the loopback filter only applies to the unfiltered
/// [`discover`] call.
pub async fn discover_on_interface(
    timeout: Duration,
    interface: &str,
) -> Result<Vec<DeviceInfo>, GigeError> {
    discover_impl(timeout, Some(interface), true).await
}

/// Discover devices on all interfaces including loopback.
///
/// This is useful for testing with simulated cameras (e.g. `arv-fake-gv-camera`)
/// bound to `127.0.0.1`.
pub async fn discover_all(timeout: Duration) -> Result<Vec<DeviceInfo>, GigeError> {
    discover_impl(timeout, None, true).await
}

/// Send a FORCEIP command to temporarily assign an IP address to a device.
///
/// FORCEIP is a broadcast command that targets a device by its MAC address.
/// The assigned IP is temporary — it does not survive a power cycle. Use
/// [`GigeDevice::write_persistent_ip`] + [`GigeDevice::enable_persistent_ip`]
/// for permanent configuration.
///
/// FORCEIP payload layout (56 bytes, big-endian):
/// ```text
/// [0..2]   reserved
/// [2..8]   target MAC address (6 bytes)
/// [8..20]  reserved
/// [20..24] static IP address
/// [24..36] reserved
/// [36..40] subnet mask
/// [40..52] reserved
/// [52..56] gateway
/// ```
pub async fn force_ip(
    mac: [u8; 6],
    ip: Ipv4Addr,
    subnet: Ipv4Addr,
    gateway: Ipv4Addr,
    iface: Option<&Iface>,
) -> Result<(), GigeError> {
    // Build the 56-byte FORCEIP payload.
    let payload = encode_forceip_payload(mac, ip, subnet, gateway);

    let local_ip = match iface {
        Some(iface) => iface
            .ipv4()
            .ok_or_else(|| GigeError::Protocol("interface lacks IPv4 address".into()))?,
        None => Ipv4Addr::UNSPECIFIED,
    };

    let socket = UdpSocket::bind(SocketAddr::new(IpAddr::V4(local_ip), 0)).await?;
    socket.set_broadcast(true)?;
    let dest = SocketAddr::new(IpAddr::V4(Ipv4Addr::BROADCAST), consts::PORT);

    let header = GvcpRequestHeader {
        flags: CommandFlags::ACK_REQUIRED | CommandFlags::BROADCAST,
        command: consts::FORCEIP_COMMAND,
        length: payload.len() as u16,
        request_id: 1,
    };
    let packet = header.encode(&payload);
    info!(mac = ?mac, %ip, %subnet, %gateway, "sending FORCEIP command");
    socket.send_to(&packet, dest).await?;

    // Wait for FORCEIP_ACK.
    let mut buf = vec![0u8; consts::DISCOVERY_BUFFER];
    match time::timeout(consts::CONTROL_TIMEOUT, socket.recv_from(&mut buf)).await {
        Ok(Ok((len, _src))) => {
            if len < viva_gencp::HEADER_SIZE {
                return Err(GigeError::Protocol("FORCEIP ack too short".into()));
            }
            let mut cursor = &buf[..];
            let status = cursor.get_u16();
            let command = cursor.get_u16();
            if command != consts::FORCEIP_ACK {
                return Err(GigeError::Protocol(format!(
                    "unexpected FORCEIP ack opcode {command:#06x}"
                )));
            }
            if status != 0 {
                return Err(GigeError::Protocol(format!(
                    "FORCEIP returned status {status:#06x}"
                )));
            }
            info!(%ip, "FORCEIP accepted");
            Ok(())
        }
        Ok(Err(e)) => Err(e.into()),
        Err(_) => Err(GigeError::Timeout),
    }
}

/// Encode the 56-byte FORCEIP payload.
fn encode_forceip_payload(
    mac: [u8; 6],
    ip: Ipv4Addr,
    subnet: Ipv4Addr,
    gateway: Ipv4Addr,
) -> Vec<u8> {
    let mut buf = vec![0u8; 56];
    // [0..2]   reserved
    // [2..8]   MAC address
    buf[2..8].copy_from_slice(&mac);
    // [8..20]  reserved
    // [20..24] IP address
    buf[20..24].copy_from_slice(&ip.octets());
    // [24..36] reserved
    // [36..40] subnet mask
    buf[36..40].copy_from_slice(&subnet.octets());
    // [40..52] reserved
    // [52..56] gateway
    buf[52..56].copy_from_slice(&gateway.octets());
    buf
}

async fn discover_impl(
    timeout: Duration,
    iface_filter: Option<&str>,
    include_loopback: bool,
) -> Result<Vec<DeviceInfo>, GigeError> {
    let mut interfaces = Vec::new();
    for iface in get_if_addrs()? {
        let IfAddr::V4(v4) = iface.addr else {
            continue;
        };
        if !include_loopback && v4.ip.is_loopback() {
            continue;
        }
        if let Some(filter) = iface_filter
            && iface.name != filter
        {
            continue;
        }
        interfaces.push((iface.name, v4));
    }

    if interfaces.is_empty() {
        return Ok(Vec::new());
    }

    let mut join_set = JoinSet::new();
    for (idx, (name, v4)) in interfaces.into_iter().enumerate() {
        let request_id = 0x0100u16.wrapping_add(idx as u16);
        let interface_name = name.clone();
        join_set.spawn(async move {
            let local_addr = SocketAddr::new(IpAddr::V4(v4.ip), 0);
            let socket = match UdpSocket::bind(local_addr).await {
                Ok(socket) => socket,
                Err(err) => {
                    warn!(%interface_name, local = %v4.ip, error = %err,
                          "skipping interface: bind failed");
                    return Vec::new();
                }
            };
            // On loopback, broadcast is not supported on some platforms (macOS).
            // Send unicast discovery directly to the local address instead.
            let destination = if v4.ip.is_loopback() {
                SocketAddr::new(IpAddr::V4(v4.ip), consts::PORT)
            } else {
                if let Err(err) = socket.set_broadcast(true) {
                    warn!(%interface_name, local = %v4.ip, error = %err,
                          "skipping interface: SO_BROADCAST failed");
                    return Vec::new();
                }
                let broadcast = v4.broadcast.unwrap_or(Ipv4Addr::BROADCAST);
                SocketAddr::new(IpAddr::V4(broadcast), consts::PORT)
            };

            let header = GvcpRequestHeader {
                flags: CommandFlags::ACK_REQUIRED | CommandFlags::BROADCAST,
                command: consts::DISCOVERY_COMMAND,
                length: 0,
                request_id,
            };
            let packet = header.encode(&[]);
            info!(%interface_name, local = %v4.ip, dest = %destination, "sending GVCP discovery");
            trace!(%interface_name, bytes = packet.len(), "GVCP discovery payload size");
            if let Err(err) = socket.send_to(&packet, destination).await {
                warn!(%interface_name, dest = %destination, error = %err,
                      "skipping interface: discovery send failed");
                return Vec::new();
            }

            let mut responses = Vec::new();
            let mut buffer = vec![0u8; consts::DISCOVERY_BUFFER];
            let timer = time::sleep(timeout);
            tokio::pin!(timer);
            loop {
                tokio::select! {
                    _ = &mut timer => break,
                    recv = socket.recv_from(&mut buffer) => {
                        // A receive error must not discard the cameras we have
                        // already found on this interface. Windows in particular
                        // reports WSAECONNRESET (10054) here when an earlier
                        // broadcast drew an ICMP port-unreachable (#57).
                        let (len, src) = match recv {
                            Ok(v) => v,
                            Err(err) => {
                                debug!(%interface_name, error = %err,
                                       "discovery receive failed; keeping results so far");
                                break;
                            }
                        };
                        info!(%interface_name, %src, "received GVCP response");
                        trace!(%interface_name, bytes = len, "GVCP response length");
                        if let Some(info) = parse_discovery_ack(&buffer[..len], request_id) {
                            trace!(ip = %info.ip, mac = %info.mac_string(), "parsed discovery ack");
                            responses.push(info);
                        }
                    }
                }
            }
            responses
        });
    }

    // One interface failing must never fail the whole call: a host commonly has
    // a down NIC, a Hyper-V switch, or a VPN adapter alongside the one the
    // camera is on.
    let mut seen = HashMap::new();
    while let Some(res) = join_set.join_next().await {
        match res {
            Ok(devices) => {
                for dev in devices {
                    seen.entry((dev.ip, dev.mac)).or_insert(dev);
                }
            }
            Err(err) => warn!(error = %err, "discovery task failed"),
        }
    }

    let mut devices: Vec<_> = seen.into_values().collect();
    devices.sort_by_key(|d| d.ip);
    Ok(devices)
}

/// Decode one datagram received on the discovery socket.
///
/// Returns `None` — never an error — for anything that is not a usable
/// Discovery ACK for us. The socket is bound to an ephemeral port on a
/// broadcast network, so unrelated GVCP traffic is expected; treating it as
/// fatal would discard the cameras already found on this interface (#57).
fn parse_discovery_ack(buf: &[u8], expected_request: u16) -> Option<DeviceInfo> {
    if buf.len() < viva_gencp::HEADER_SIZE {
        trace!(len = buf.len(), "ignoring short GVCP datagram");
        return None;
    }
    let mut header = buf;
    let status = header.get_u16();
    let command = header.get_u16();
    let length = header.get_u16() as usize;
    let request_id = header.get_u16();
    if request_id != expected_request {
        return None;
    }
    if command != consts::DISCOVERY_ACK {
        debug!(
            opcode = format_args!("{command:#06x}"),
            "ignoring non-discovery GVCP ack"
        );
        return None;
    }
    if status != 0 {
        debug!(
            status = format_args!("{status:#06x}"),
            "discovery ack reported a non-zero status"
        );
        return None;
    }
    if buf.len() < viva_gencp::HEADER_SIZE + length {
        debug!(
            declared = length,
            actual = buf.len() - viva_gencp::HEADER_SIZE,
            "ignoring truncated discovery payload"
        );
        return None;
    }
    let payload = &buf[viva_gencp::HEADER_SIZE..viva_gencp::HEADER_SIZE + length];
    match parse_discovery_payload(payload) {
        Ok(info) => Some(info),
        Err(err) => {
            debug!(error = %err, "ignoring unparsable discovery payload");
            None
        }
    }
}

/// Parse a GigE Vision Discovery ACK payload (248 bytes).
///
/// The payload mirrors the device's bootstrap register block, so the MAC is
/// split across `DeviceMACAddressHigh` (0x08, whose low half holds the top two
/// octets) and `DeviceMACAddressLow` (0x0C) — six contiguous bytes at offset
/// **10**, not 12. Cross-checked against Wireshark's `dissect_discovery_ack()`,
/// which reads the MAC from `offset + 10` and the IP, manufacturer and model
/// from 36, 72 and 104. Reported in #57 against a JAI FS-3200T-10GE-NNC, whose
/// `00:0C:DF:06:5B:2F` was read as `DF:06:5B:2F:C0:00`.
///
/// | Offset | Size | Field                        |
/// |--------|------|------------------------------|
/// |      0 |    2 | Spec version major           |
/// |      2 |    2 | Spec version minor           |
/// |      4 |    4 | Device mode                  |
/// |      8 |    2 | Reserved (MAC-high padding)  |
/// |     10 |    6 | MAC address                  |
/// |     16 |    4 | Supported IP config          |
/// |     20 |    4 | Current IP config            |
/// |     24 |   12 | Reserved                     |
/// |     36 |    4 | Current IP address           |
/// |     40 |   12 | Reserved                     |
/// |     52 |    4 | Current subnet mask          |
/// |     56 |   12 | Reserved                     |
/// |     68 |    4 | Default gateway              |
/// |     72 |   32 | Manufacturer name            |
/// |    104 |   32 | Model name                   |
/// |    136 |   32 | Device version               |
/// |    168 |   48 | Manufacturer specific info   |
/// |    216 |   16 | Serial number                |
/// |    232 |   16 | User defined name            |
fn parse_discovery_payload(payload: &[u8]) -> Result<DeviceInfo, GigeError> {
    // Minimum size to reach past the current IP field. Everything after it is
    // read leniently: a short-but-valid ACK should still yield a usable device
    // rather than failing discovery on that interface.
    if payload.len() < 40 {
        return Err(GigeError::Protocol("discovery payload too small".into()));
    }
    let mut cursor = Cursor::new(payload);
    let _spec_major = cursor.get_u16(); // 0
    let _spec_minor = cursor.get_u16(); // 2
    let _device_mode = cursor.get_u32(); // 4

    // Only the two padding bytes of the MAC-high register precede the address.
    cursor.advance(2); // 8..10

    // MAC: 2 bytes from the high register + 4 from the low register.
    let mut mac = [0u8; 6];
    cursor.copy_to_slice(&mut mac); // 10..16

    let _supported_ip_config = cursor.get_u32(); // 16
    let _current_ip_config = cursor.get_u32(); // 20

    // 12 bytes reserved before current IP.
    cursor.advance(12); // 24..36
    let ip = Ipv4Addr::from(cursor.get_u32()); // 36

    // Everything past the IP is optional, so every step from here on has to
    // tolerate the payload ending. Subnet and gateway are not retained.
    skip(&mut cursor, 12 + 4); // 40..52 reserved, 52 subnet
    skip(&mut cursor, 12 + 4); // 56..68 reserved, 68 gateway

    // String fields. All optional: a device that truncates the payload still
    // gives us an addressable camera.
    let manufacturer = read_fixed_string(&mut cursor, 32); // 72
    let model = read_fixed_string(&mut cursor, 32); // 104
    let version = read_fixed_string(&mut cursor, 32); // 136
    skip(&mut cursor, 48); // 168 manufacturer-specific info
    let serial = read_fixed_string(&mut cursor, 16); // 216
    let user_name = read_fixed_string(&mut cursor, 16); // 232

    Ok(DeviceInfo {
        ip,
        mac,
        manufacturer,
        model,
        version,
        serial,
        user_name,
    })
}

/// Read a NUL-padded fixed-width string, or `None` if the payload ends early.
fn read_fixed_string(cursor: &mut Cursor<&[u8]>, len: usize) -> Option<String> {
    if cursor.remaining() < len {
        return None;
    }
    let mut buf = vec![0u8; len];
    cursor.copy_to_slice(&mut buf);
    parse_string(&buf)
}

/// Advance past a field, stopping at the end of a truncated payload.
fn skip(cursor: &mut Cursor<&[u8]>, len: usize) {
    let n = len.min(cursor.remaining());
    cursor.advance(n);
}

fn parse_string(bytes: &[u8]) -> Option<String> {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    let slice = &bytes[..end];
    let s = String::from_utf8_lossy(slice).trim().to_string();
    if s.is_empty() { None } else { Some(s) }
}

/// Outcome of waiting for one acknowledgement.
///
/// Mirrors the three cases the caller already distinguishes — a datagram, a
/// socket error, and a deadline — so that PENDING_ACK handling can be factored
/// out without changing the retry policy around it.
enum AckRecv {
    Received(usize),
    Io(std::io::Error),
    TimedOut,
}

/// Decode a GVCP PENDING_ACK, returning the request id it refers to and the
/// extra time the device is asking for.
///
/// Returns `None` for anything that is not a PENDING_ACK, so the caller can
/// hand the datagram to the normal GenCP decoder.
///
/// Layout per GigE Vision 1.2 section 18.5: the 8-byte GVCP acknowledgement
/// header, then two reserved bytes and a 16-bit `time_to_completion` in
/// milliseconds.
///
/// This diverges deliberately from aravis, whose
/// `arv_gvcp_packet_get_pending_ack_timeout` reads all four payload bytes as a
/// big-endian `u32`. The two agree whenever the reserved field is zero, which
/// is what the specification requires of the device; reading the `u16` the
/// specification actually defines is the safer of the two, because a device
/// that leaves junk in the reserved field cannot then talk us into a wait
/// three orders of magnitude too long. Callers clamp the result regardless.
fn parse_pending_ack(buf: &[u8]) -> Option<(u16, Duration)> {
    if buf.len() < viva_gencp::HEADER_SIZE {
        return None;
    }
    if u16::from_be_bytes([buf[2], buf[3]]) != consts::PENDING_ACK {
        return None;
    }
    let request_id = u16::from_be_bytes([buf[6], buf[7]]);
    let payload = &buf[viva_gencp::HEADER_SIZE..];
    // A truncated PENDING_ACK is still an unambiguous request for more time;
    // grant the default rather than discarding it and resending the command.
    let millis = match payload {
        [_, _, hi, lo, ..] => u16::from_be_bytes([*hi, *lo]),
        _ => return Some((request_id, consts::CONTROL_TIMEOUT)),
    };
    Some((request_id, Duration::from_millis(u64::from(millis))))
}

/// GVCP device handle.
pub struct GigeDevice {
    socket: UdpSocket,
    remote: SocketAddr,
    request_id: u16,
    rng: Rng,
}

/// Stream negotiation outcome describing the values written to the device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamParams {
    /// Selected GVSP packet size (bytes).
    pub packet_size: u32,
    /// Packet delay expressed in GVSP clock ticks (80 ns units).
    pub packet_delay: u32,
    /// Link MTU used to derive the packet size.
    pub mtu: u32,
    /// Host IPv4 address configured on the device.
    pub host: Ipv4Addr,
    /// Host port configured on the device.
    pub port: u16,
}

impl GigeDevice {
    /// Connect to a device GVCP endpoint.
    ///
    /// The connection is ready for register read/write but does not claim
    /// control privilege. Call [`Self::claim_control`] before configuring streaming
    /// or starting acquisition.
    pub async fn open(addr: SocketAddr) -> Result<Self, GigeError> {
        let local_ip = match addr.ip() {
            IpAddr::V4(_) => IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            IpAddr::V6(_) => {
                return Err(GigeError::Protocol("IPv6 GVCP is not supported".into()));
            }
        };
        let socket = UdpSocket::bind(SocketAddr::new(local_ip, 0)).await?;
        socket.connect(addr).await?;
        Ok(Self {
            socket,
            remote: addr,
            request_id: 1,
            rng: Rng::new(),
        })
    }

    /// Claim control channel privilege (CCP).
    ///
    /// Required by the GigE Vision specification before the device accepts
    /// stream configuration or acquisition commands.
    pub async fn claim_control(&mut self) -> Result<(), GigeError> {
        self.write_register(
            consts::CONTROL_CHANNEL_PRIVILEGE as u32,
            consts::CCP_CONTROL,
        )
        .await?;
        debug!(addr = %self.remote, "claimed control channel privilege");
        Ok(())
    }

    /// Release control channel privilege.
    pub async fn release_control(&mut self) -> Result<(), GigeError> {
        self.write_register(consts::CONTROL_CHANNEL_PRIVILEGE as u32, 0)
            .await
    }

    /// Return the remote GVCP socket address associated with this device.
    pub fn remote_addr(&self) -> SocketAddr {
        self.remote
    }

    fn next_request_id(&mut self) -> u16 {
        let id = self.request_id;
        self.request_id = self.request_id.wrapping_add(1);
        if self.request_id == 0 {
            self.request_id = 1;
        }
        id
    }

    async fn transact_with_retry(
        &mut self,
        opcode: OpCode,
        payload: BytesMut,
    ) -> Result<GenCpAck, GigeError> {
        let mut attempt = 0usize;
        let mut payload = payload;
        loop {
            attempt += 1;
            let request_id = self.next_request_id();
            let payload_bytes = payload.clone().freeze();
            let header = GvcpRequestHeader {
                flags: CommandFlags::ACK_REQUIRED,
                command: opcode.command_code(),
                length: payload_bytes.len() as u16,
                request_id,
            };
            let encoded = header.encode(&payload_bytes);
            trace!(request_id, opcode = ?opcode, bytes = encoded.len(), attempt, "sending GVCP command");
            if let Err(err) = self.socket.send(&encoded).await {
                if attempt >= consts::MAX_RETRIES {
                    return Err(err.into());
                }
                warn!(request_id, ?opcode, attempt, "send failed, retrying");
                self.backoff(attempt).await;
                payload = BytesMut::from(&payload_bytes[..]);
                continue;
            }

            let mut buf = vec![
                0u8;
                viva_gencp::HEADER_SIZE
                    + consts::GENCP_MAX_BLOCK
                    + consts::GENCP_WRITE_OVERHEAD
            ];
            match self.recv_absorbing_pending(&mut buf, request_id).await {
                AckRecv::Received(len) => {
                    trace!(request_id, bytes = len, attempt, "received GenCP ack");
                    let ack = decode_ack(&buf[..len])?;
                    if ack.header.request_id != request_id {
                        debug!(
                            request_id,
                            got = ack.header.request_id,
                            attempt,
                            "acknowledgement id mismatch"
                        );
                        if attempt >= consts::MAX_RETRIES {
                            return Err(GigeError::Protocol("acknowledgement id mismatch".into()));
                        }
                        self.backoff(attempt).await;
                        payload = BytesMut::from(&payload_bytes[..]);
                        continue;
                    }
                    if ack.header.opcode != opcode {
                        return Err(GigeError::Protocol(
                            "unexpected opcode in acknowledgement".into(),
                        ));
                    }
                    match ack.header.status {
                        StatusCode::Success => return Ok(ack),
                        // Only `BUSY` (0x8007) is congestion. This used to
                        // match `DeviceBusy`, which was mapped to 0x8004 —
                        // `WRITE_PROTECT` — so the retry loop burned its
                        // budget on a read-only register that could never
                        // accept the write, and gave up immediately on the
                        // one status retrying is for.
                        status if status.is_retryable() && attempt < consts::MAX_RETRIES => {
                            warn!(request_id, attempt, %status, "device busy, retrying");
                            self.backoff(attempt).await;
                            payload = BytesMut::from(&payload_bytes[..]);
                            continue;
                        }
                        other => return Err(GigeError::Status(other)),
                    }
                }
                AckRecv::Io(err) => {
                    if attempt >= consts::MAX_RETRIES {
                        return Err(err.into());
                    }
                    warn!(request_id, ?opcode, attempt, "receive error, retrying");
                    self.backoff(attempt).await;
                    payload = BytesMut::from(&payload_bytes[..]);
                }
                AckRecv::TimedOut => {
                    if attempt >= consts::MAX_RETRIES {
                        return Err(GigeError::Timeout);
                    }
                    warn!(request_id, ?opcode, attempt, "command timeout, retrying");
                    self.backoff(attempt).await;
                    payload = BytesMut::from(&payload_bytes[..]);
                }
            }
        }
    }

    /// Receive one acknowledgement, granting the device the extra time it asks
    /// for via PENDING_ACK.
    ///
    /// A PENDING_ACK is not a failure and must not be retried: the command is
    /// still executing on the device, so resending it risks running it twice —
    /// which for a `WriteMem` to flash is exactly the operation you least want
    /// duplicated. The GigE Vision specification instead has the controller
    /// extend its own deadline by the time the device requests and keep
    /// waiting on the same request id.
    async fn recv_absorbing_pending(&mut self, buf: &mut [u8], request_id: u16) -> AckRecv {
        let mut wait = consts::CONTROL_TIMEOUT;
        let mut pending_seen = 0usize;
        loop {
            match time::timeout(wait, self.socket.recv(buf)).await {
                Ok(Ok(len)) => {
                    let Some((pending_id, requested)) = parse_pending_ack(&buf[..len]) else {
                        return AckRecv::Received(len);
                    };
                    if pending_id != request_id {
                        debug!(
                            request_id,
                            got = pending_id,
                            "ignoring PENDING_ACK for another request"
                        );
                        continue;
                    }
                    pending_seen += 1;
                    if pending_seen > consts::MAX_PENDING_ACKS {
                        warn!(
                            request_id,
                            pending_seen, "device kept requesting more time, giving up"
                        );
                        return AckRecv::TimedOut;
                    }
                    wait = requested.clamp(consts::CONTROL_TIMEOUT, consts::MAX_PENDING_ACK_WAIT);
                    debug!(
                        request_id,
                        requested_ms = requested.as_millis() as u64,
                        waiting_ms = wait.as_millis() as u64,
                        pending_seen,
                        "device requested more time (PENDING_ACK)"
                    );
                }
                Ok(Err(err)) => return AckRecv::Io(err),
                Err(_) => return AckRecv::TimedOut,
            }
        }
    }

    async fn backoff(&mut self, attempt: usize) {
        let multiplier = 1u32 << (attempt.saturating_sub(1)).min(3);
        let base_ms = consts::RETRY_BASE_DELAY.as_millis() as u64;
        let base = Duration::from_millis(base_ms.saturating_mul(multiplier as u64).max(base_ms));
        let jitter_ms = self.rng.u64(..=consts::RETRY_JITTER.as_millis() as u64);
        let jitter = Duration::from_millis(jitter_ms);
        let delay = base + jitter;
        debug!(attempt, delay = ?delay, "gvcp retry backoff");
        time::sleep(delay).await;
    }

    /// Read a single 32-bit bootstrap or device register.
    ///
    /// Uses GVCP READREG format: 4-byte register address.
    /// The acknowledgement carries the 4-byte register value.
    pub async fn read_register(&mut self, addr: u32) -> Result<u32, GigeError> {
        let mut payload = BytesMut::with_capacity(4);
        payload.put_u32(addr);
        let ack = self
            .transact_with_retry(OpCode::ReadRegister, payload)
            .await?;
        if ack.payload.len() != 4 {
            return Err(GigeError::Protocol(format!(
                "expected 4-byte register ack but device returned {} bytes",
                ack.payload.len()
            )));
        }
        let mut cursor = &ack.payload[..];
        Ok(cursor.get_u32())
    }

    /// Write a single 32-bit bootstrap or device register.
    ///
    /// Uses GVCP WRITEREG format: 4-byte register address + 4-byte value.
    /// The acknowledgement carries a 4-byte data index placeholder.
    pub async fn write_register(&mut self, addr: u32, value: u32) -> Result<(), GigeError> {
        let mut payload = BytesMut::with_capacity(8);
        payload.put_u32(addr);
        payload.put_u32(value);
        let ack = self
            .transact_with_retry(OpCode::WriteRegister, payload)
            .await?;
        if ack.payload.len() != 4 {
            return Err(GigeError::Protocol(format!(
                "expected 4-byte register write ack but device returned {} bytes",
                ack.payload.len()
            )));
        }
        Ok(())
    }

    /// Read a block of memory from the remote device with chunking and retries.
    ///
    /// Uses GVCP READMEM format: 4-byte address + 2-byte reserved + 2-byte count.
    /// The acknowledgement carries: 4-byte address echo + data bytes.
    pub async fn read_mem(&mut self, addr: u64, len: usize) -> Result<Vec<u8>, GigeError> {
        let mut remaining = len;
        let mut offset = 0usize;
        let mut data = Vec::with_capacity(len);
        while remaining > 0 {
            let chunk = remaining.min(consts::GENCP_MAX_BLOCK);
            // GVCP requires the READMEM byte count to be a multiple of 4.
            // Strict cameras (e.g. Hikrobot) reject unaligned counts with
            // InvalidParameter, so request the aligned size and drop the
            // padding bytes below. Device memory regions are 4-byte aligned,
            // so reading past the end of e.g. the XML blob is safe.
            let request = chunk.next_multiple_of(4);
            let mut payload = BytesMut::with_capacity(8);
            payload.put_u32((addr + offset as u64) as u32);
            payload.put_u16(0); // reserved
            payload.put_u16(request as u16);
            let ack = self.transact_with_retry(OpCode::ReadMem, payload).await?;
            // GVCP READMEM_ACK: 4-byte address prefix + data.
            let ack_data = if ack.payload.len() >= 4 + request {
                &ack.payload[4..4 + request]
            } else if ack.payload.len() == request {
                // Some devices omit the address echo.
                &ack.payload[..request]
            } else {
                return Err(GigeError::Protocol(format!(
                    "expected {} bytes but device returned {}",
                    request,
                    ack.payload.len()
                )));
            };
            data.extend_from_slice(&ack_data[..chunk]);
            remaining -= chunk;
            offset += chunk;
        }
        Ok(data)
    }

    /// Write a block of memory to the remote device with chunking and retries.
    ///
    /// Uses GVCP WRITEMEM format: 4-byte address + data bytes.
    /// The acknowledgement carries: 4-byte reserved (index).
    pub async fn write_mem(&mut self, addr: u64, data: &[u8]) -> Result<(), GigeError> {
        /// GVCP WRITEMEM overhead: 4-byte address prefix.
        const GVCP_WRITE_OVERHEAD: usize = 4;

        let mut offset = 0usize;
        while offset < data.len() {
            let chunk = (data.len() - offset).min(consts::GENCP_MAX_BLOCK - GVCP_WRITE_OVERHEAD);
            if chunk == 0 {
                return Err(GigeError::Protocol("write chunk size is zero".into()));
            }
            let mut payload = BytesMut::with_capacity(GVCP_WRITE_OVERHEAD + chunk);
            payload.put_u32((addr + offset as u64) as u32);
            payload.extend_from_slice(&data[offset..offset + chunk]);
            let ack = self.transact_with_retry(OpCode::WriteMem, payload).await?;
            // GVCP WRITEMEM_ACK: 4-byte reserved payload.
            if ack.payload.len() > 4 {
                return Err(GigeError::Protocol(
                    "write acknowledgement carried unexpected payload".into(),
                ));
            }
            offset += chunk;
        }
        Ok(())
    }

    /// Configure the message channel destination address/port.
    pub async fn set_message_destination(
        &mut self,
        ip: Ipv4Addr,
        port: u16,
    ) -> Result<(), GigeError> {
        info!(%ip, port, "configuring message channel destination");
        self.write_mem(consts::MESSAGE_DESTINATION_ADDRESS, &ip.octets())
            .await?;
        // GevMCP is a 32-bit register with the port in the low half. Writing
        // only the two port bytes lands them in the *high* half.
        self.write_mem(
            consts::MESSAGE_DESTINATION_PORT,
            &u32::from(port).to_be_bytes(),
        )
        .await?;
        Ok(())
    }

    fn stream_reg(channel: u32, offset: u64) -> u64 {
        consts::STREAM_CHANNEL_BASE + channel as u64 * consts::STREAM_CHANNEL_STRIDE + offset
    }

    /// Configure the GVSP host destination for the provided channel.
    pub async fn set_stream_destination(
        &mut self,
        channel: u32,
        ip: Ipv4Addr,
        port: u16,
    ) -> Result<(), GigeError> {
        info!(channel, %ip, port, "configuring stream destination");
        let addr = Self::stream_reg(channel, consts::STREAM_DESTINATION_ADDRESS);
        self.write_mem(addr, &ip.octets()).await?;
        let addr = Self::stream_reg(channel, consts::STREAM_DESTINATION_PORT);
        self.write_mem(addr, &(port as u32).to_be_bytes()).await?;
        Ok(())
    }

    /// Configure the packet size for the stream channel.
    pub async fn set_stream_packet_size(
        &mut self,
        channel: u32,
        packet_size: u32,
    ) -> Result<(), GigeError> {
        info!(channel, packet_size, "configuring stream packet size");
        let addr = Self::stream_reg(channel, consts::STREAM_PACKET_SIZE);
        self.write_mem(addr, &packet_size.to_be_bytes()).await
    }

    /// Configure the packet delay (`GevSCPD`).
    pub async fn set_stream_packet_delay(
        &mut self,
        channel: u32,
        packet_delay: u32,
    ) -> Result<(), GigeError> {
        debug!(channel, packet_delay, "configuring stream packet delay");
        let addr = Self::stream_reg(channel, consts::STREAM_PACKET_DELAY);
        self.write_mem(addr, &packet_delay.to_be_bytes()).await
    }

    /// Negotiate GVSP parameters with the device given the host interface.
    pub async fn negotiate_stream(
        &mut self,
        channel: u32,
        iface: &Iface,
        port: u16,
        target_mtu: Option<u32>,
    ) -> Result<StreamParams, GigeError> {
        let host_ip = iface
            .ipv4()
            .ok_or_else(|| GigeError::Protocol("interface lacks IPv4 address".into()))?;
        let iface_mtu = nic::mtu(iface)?;
        let mtu = target_mtu.map_or(iface_mtu, |limit| limit.min(iface_mtu));
        let packet_size = nic::best_packet_size(mtu);
        let packet_delay = if mtu <= 1500 {
            // When jumbo frames are unavailable we space out packets by 2 µs to
            // prevent excessive buffering pressure on receivers. GVSP expresses
            // `GevSCPD` in units of 80 ns.
            const DELAY_NS: u32 = 2_000; // 2 µs.
            DELAY_NS / 80
        } else {
            0
        };

        self.set_stream_destination(channel, host_ip, port).await?;
        self.set_stream_packet_size(channel, packet_size).await?;
        self.set_stream_packet_delay(channel, packet_delay).await?;

        Ok(StreamParams {
            packet_size,
            packet_delay,
            mtu,
            host: host_ip,
            port,
        })
    }
    /// Read the persistent IP configuration from the device.
    ///
    /// Returns `(ip, subnet, gateway)`.
    pub async fn read_persistent_ip(
        &mut self,
    ) -> Result<(Ipv4Addr, Ipv4Addr, Ipv4Addr), GigeError> {
        let ip = Ipv4Addr::from(
            self.read_register(consts::PERSISTENT_IP_ADDRESS as u32)
                .await?,
        );
        let subnet = Ipv4Addr::from(
            self.read_register(consts::PERSISTENT_SUBNET_MASK as u32)
                .await?,
        );
        let gateway = Ipv4Addr::from(
            self.read_register(consts::PERSISTENT_DEFAULT_GATEWAY as u32)
                .await?,
        );
        Ok((ip, subnet, gateway))
    }

    /// Write the persistent IP configuration to the device.
    pub async fn write_persistent_ip(
        &mut self,
        ip: Ipv4Addr,
        subnet: Ipv4Addr,
        gateway: Ipv4Addr,
    ) -> Result<(), GigeError> {
        self.write_register(consts::PERSISTENT_IP_ADDRESS as u32, u32::from(ip))
            .await?;
        self.write_register(consts::PERSISTENT_SUBNET_MASK as u32, u32::from(subnet))
            .await?;
        self.write_register(
            consts::PERSISTENT_DEFAULT_GATEWAY as u32,
            u32::from(gateway),
        )
        .await?;
        info!(%ip, %subnet, %gateway, "wrote persistent IP configuration");
        Ok(())
    }

    /// Enable persistent IP mode in the device configuration flags.
    ///
    /// Sets bit 1 (persistent IP) in the `CurrentIPConfiguration` register.
    pub async fn enable_persistent_ip(&mut self) -> Result<(), GigeError> {
        let current = self.read_register(consts::CURRENT_IP_CONFIG as u32).await?;
        let updated = current | 0x02; // bit 1 = persistent IP
        self.write_register(consts::CURRENT_IP_CONFIG as u32, updated)
            .await?;
        info!(config = format!("0x{updated:08x}"), "enabled persistent IP");
        Ok(())
    }

    /// Request resend of a packet range for the provided block identifier.
    pub async fn request_resend(
        &mut self,
        block_id: u16,
        first_packet: u16,
        last_packet: u16,
    ) -> Result<(), GigeError> {
        let mut payload = BytesMut::with_capacity(8);
        payload.put_u16(block_id);
        payload.put_u16(0); // Reserved as per spec.
        payload.put_u16(first_packet);
        payload.put_u16(last_packet);

        let request_id = self.next_request_id();
        let header = GvcpRequestHeader {
            flags: CommandFlags::ACK_REQUIRED,
            command: consts::PACKET_RESEND_COMMAND,
            length: payload.len() as u16,
            request_id,
        };
        let packet = header.encode(&payload);
        trace!(
            block_id,
            first_packet, last_packet, request_id, "sending packet resend request"
        );
        self.socket.send(&packet).await?;
        let mut buf = [0u8; viva_gencp::HEADER_SIZE];
        match time::timeout(consts::CONTROL_TIMEOUT, self.socket.recv(&mut buf)).await {
            Ok(Ok(len)) => {
                if len != viva_gencp::HEADER_SIZE {
                    return Err(GigeError::Protocol("resend ack length mismatch".into()));
                }
                let mut cursor = &buf[..];
                let status = StatusCode::from_raw(cursor.get_u16());
                let command = cursor.get_u16();
                let length = cursor.get_u16();
                let ack_request_id = cursor.get_u16();
                if command == consts::PENDING_ACK {
                    // Legal, but not worth waiting for: by the time the device
                    // finished, the frame this resend belongs to would already
                    // have been completed or dropped. Report it accurately
                    // instead of as an unexpected opcode.
                    return Err(GigeError::Protocol(
                        "device requested more time for a packet resend".into(),
                    ));
                }
                if command != consts::PACKET_RESEND_ACK {
                    return Err(GigeError::Protocol("unexpected resend ack opcode".into()));
                }
                if length != 0 {
                    return Err(GigeError::Protocol("resend ack carried payload".into()));
                }
                if ack_request_id != request_id {
                    return Err(GigeError::Protocol("resend ack request id mismatch".into()));
                }
                if status != StatusCode::Success {
                    return Err(GigeError::Status(status));
                }
                Ok(())
            }
            Ok(Err(err)) => Err(err.into()),
            Err(_) => Err(GigeError::Timeout),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A Discovery ACK payload written out from the specification's field
    /// table, with a distinct recognisable value in every field.
    ///
    /// Built here rather than by round-tripping our own encoder: the point of
    /// this fixture is to disagree with the parser if the parser is wrong. See
    /// [ADR-0019]. The offsets are corroborated by Wireshark's
    /// `dissect_discovery_ack()`, which reads the MAC from `offset + 10` and
    /// the IP, manufacturer and model from 36, 72 and 104.
    ///
    /// [ADR-0019]: https://github.com/VitalyVorobyev/viva-genicam/blob/main/docs/adrs/adr0019-transport-conformance-and-spec-derived-fakes.md
    fn golden_discovery_payload() -> Vec<u8> {
        let mut p = vec![0u8; 248];
        p[0..2].copy_from_slice(&2u16.to_be_bytes()); // spec major
        p[2..4].copy_from_slice(&1u16.to_be_bytes()); // spec minor
        p[4..8].copy_from_slice(&0u32.to_be_bytes()); // device mode
        // 8..10 is the padding half of the MAC-high register.
        p[10..16].copy_from_slice(&[0x00, 0x0C, 0xDF, 0x06, 0x5B, 0x2F]); // MAC
        p[16..20].copy_from_slice(&7u32.to_be_bytes()); // supported IP config
        p[20..24].copy_from_slice(&5u32.to_be_bytes()); // current IP config
        p[36..40].copy_from_slice(&[169, 254, 78, 62]); // current IP
        p[52..56].copy_from_slice(&[255, 255, 0, 0]); // subnet
        p[68..72].copy_from_slice(&[0, 0, 0, 0]); // gateway
        let put =
            |p: &mut [u8], at: usize, s: &str| p[at..at + s.len()].copy_from_slice(s.as_bytes());
        put(&mut p, 72, "JAI Corporation"); // manufacturer
        put(&mut p, 104, "FS-3200T-10GE-NNC"); // model
        put(&mut p, 136, "1.2.3"); // device version
        put(&mut p, 168, "mfr-specific"); // manufacturer info
        put(&mut p, 216, "SN-12345"); // serial
        put(&mut p, 232, "left-camera"); // user-defined name
        p
    }

    #[test]
    fn discovery_payload_matches_spec_offsets() {
        let info = parse_discovery_payload(&golden_discovery_payload()).expect("parse");

        // The MAC begins at offset 10. Reading it at 12 — as we did before
        // #57 — yields DF:06:5B:2F:00:07, silently folding two bytes of
        // SupportedIPConfiguration into the address.
        assert_eq!(info.mac, [0x00, 0x0C, 0xDF, 0x06, 0x5B, 0x2F]);
        assert_eq!(info.mac_string(), "00:0C:DF:06:5B:2F");
        assert_eq!(info.ip, Ipv4Addr::new(169, 254, 78, 62));
        assert_eq!(info.manufacturer.as_deref(), Some("JAI Corporation"));
        assert_eq!(info.model.as_deref(), Some("FS-3200T-10GE-NNC"));
        assert_eq!(info.version.as_deref(), Some("1.2.3"));
        assert_eq!(info.serial.as_deref(), Some("SN-12345"));
        assert_eq!(info.user_name.as_deref(), Some("left-camera"));
    }

    #[test]
    fn discovery_payload_tolerates_truncation() {
        // A device that stops after the IP field still yields an addressable
        // camera rather than failing discovery on that interface.
        let short = golden_discovery_payload()[..40].to_vec();
        let info = parse_discovery_payload(&short).expect("short payload should parse");
        assert_eq!(info.ip, Ipv4Addr::new(169, 254, 78, 62));
        assert_eq!(info.mac, [0x00, 0x0C, 0xDF, 0x06, 0x5B, 0x2F]);
        assert_eq!(info.manufacturer, None);
        assert_eq!(info.serial, None);

        // Below the IP field there is nothing usable.
        assert!(parse_discovery_payload(&[0u8; 12]).is_err());
    }

    #[test]
    fn discovery_ack_ignores_foreign_traffic() {
        let payload = golden_discovery_payload();
        let ack = |status: u16, command: u16, request_id: u16| {
            let mut buf = Vec::new();
            buf.extend_from_slice(&status.to_be_bytes());
            buf.extend_from_slice(&command.to_be_bytes());
            buf.extend_from_slice(&(payload.len() as u16).to_be_bytes());
            buf.extend_from_slice(&request_id.to_be_bytes());
            buf.extend_from_slice(&payload);
            buf
        };

        // The real thing.
        assert!(parse_discovery_ack(&ack(0, consts::DISCOVERY_ACK, 0x0100), 0x0100).is_some());
        // Someone else's request id, a READREG ack that landed on our socket,
        // an error status, and a runt datagram must all be ignored rather than
        // failing discovery for every camera on the interface (#57).
        assert!(parse_discovery_ack(&ack(0, consts::DISCOVERY_ACK, 0x0999), 0x0100).is_none());
        assert!(parse_discovery_ack(&ack(0, 0x0081, 0x0100), 0x0100).is_none());
        assert!(parse_discovery_ack(&ack(0x8002, consts::DISCOVERY_ACK, 0x0100), 0x0100).is_none());
        assert!(parse_discovery_ack(&[0u8; 4], 0x0100).is_none());
    }

    /// A PENDING_ACK written out from the specification's field table:
    /// the 8-byte acknowledgement header, two reserved bytes, then a 16-bit
    /// `time_to_completion` in milliseconds.
    fn golden_pending_ack(request_id: u16, millis: u16) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&0u16.to_be_bytes()); // status: success
        buf.extend_from_slice(&consts::PENDING_ACK.to_be_bytes()); // 0x0089
        buf.extend_from_slice(&4u16.to_be_bytes()); // payload length
        buf.extend_from_slice(&request_id.to_be_bytes());
        buf.extend_from_slice(&0u16.to_be_bytes()); // reserved
        buf.extend_from_slice(&millis.to_be_bytes()); // time to completion
        buf
    }

    #[test]
    fn pending_ack_matches_spec_offsets() {
        let (id, wait) = parse_pending_ack(&golden_pending_ack(0x1234, 750)).expect("pending ack");
        assert_eq!(id, 0x1234);
        assert_eq!(wait, Duration::from_millis(750));

        // The time is the u16 at payload offset 2, not a u32 over the whole
        // payload. The two readings agree only while the reserved field is
        // zero; a device that leaves junk there would ask aravis for
        // 0xDEAD_02EE ms — around 41 days — where we read 750.
        let mut junk = golden_pending_ack(0x1234, 750);
        junk[8..10].copy_from_slice(&0xDEADu16.to_be_bytes());
        assert_eq!(
            parse_pending_ack(&junk).expect("pending ack").1.as_millis(),
            750
        );
    }

    #[test]
    fn pending_ack_is_distinguished_from_real_acks() {
        // A genuine READREG ack must not be mistaken for a request for time,
        // or every register read would hang until the retry budget ran out.
        let mut readreg = Vec::new();
        readreg.extend_from_slice(&0u16.to_be_bytes());
        readreg.extend_from_slice(&0x0081u16.to_be_bytes());
        readreg.extend_from_slice(&4u16.to_be_bytes());
        readreg.extend_from_slice(&0x1234u16.to_be_bytes());
        readreg.extend_from_slice(&0u32.to_be_bytes());
        assert!(parse_pending_ack(&readreg).is_none());
        assert!(parse_pending_ack(&[0u8; 4]).is_none());

        // A device that truncates the payload is still asking for time.
        let truncated = &golden_pending_ack(0x1234, 750)[..viva_gencp::HEADER_SIZE];
        let (id, wait) = parse_pending_ack(truncated).expect("truncated pending ack");
        assert_eq!(id, 0x1234);
        assert_eq!(wait, consts::CONTROL_TIMEOUT);
    }

    /// A minimal GVCP device that answers `pending` PENDING_ACKs before the
    /// real acknowledgement, counting how many commands it was actually sent.
    ///
    /// The count is the point: a controller that treats PENDING_ACK as a
    /// failure and retries would execute the command more than once.
    async fn pending_ack_device(
        pending: usize,
        wait_ms: u16,
    ) -> (SocketAddr, tokio::task::JoinHandle<usize>) {
        let sock = UdpSocket::bind("127.0.0.1:0").await.expect("bind");
        let addr = sock.local_addr().expect("addr");
        let handle = tokio::spawn(async move {
            let mut buf = [0u8; 2048];
            let mut commands = 0usize;
            let (len, peer) = sock.recv_from(&mut buf).await.expect("recv");
            commands += 1;
            let request_id = u16::from_be_bytes([buf[6], buf[7]]);
            debug_assert!(len >= viva_gencp::HEADER_SIZE);
            for _ in 0..pending {
                let ack = golden_pending_ack(request_id, wait_ms);
                sock.send_to(&ack, peer).await.expect("send pending");
            }
            // The real READREG ack: one 4-byte register value.
            let mut ack = Vec::new();
            ack.extend_from_slice(&0u16.to_be_bytes());
            ack.extend_from_slice(&0x0081u16.to_be_bytes());
            ack.extend_from_slice(&4u16.to_be_bytes());
            ack.extend_from_slice(&request_id.to_be_bytes());
            ack.extend_from_slice(&0xCAFEBABEu32.to_be_bytes());
            sock.send_to(&ack, peer).await.expect("send ack");
            // Drain any retry the controller wrongly sent, so the count is
            // observable rather than lost in the socket buffer.
            let drain = time::timeout(Duration::from_millis(200), sock.recv_from(&mut buf)).await;
            if drain.is_ok() {
                commands += 1;
            }
            commands
        });
        (addr, handle)
    }

    #[tokio::test]
    async fn pending_ack_extends_the_deadline_without_resending() {
        let (addr, server) = pending_ack_device(1, 300).await;
        let mut device = GigeDevice::open(addr).await.expect("open");
        let value = device.read_register(0x0a00).await.expect("read register");
        assert_eq!(value, 0xCAFEBABE);
        assert_eq!(server.await.expect("join"), 1, "command must not be resent");
    }

    #[tokio::test]
    async fn repeated_pending_acks_are_all_honoured() {
        // A flash write can take several rounds. Each one restarts the clock;
        // the command is still sent exactly once.
        let (addr, server) = pending_ack_device(3, 200).await;
        let mut device = GigeDevice::open(addr).await.expect("open");
        let value = device.read_register(0x0a00).await.expect("read register");
        assert_eq!(value, 0xCAFEBABE);
        assert_eq!(server.await.expect("join"), 1, "command must not be resent");
    }

    /// A device that answers `error_replies` commands with `status_raw` and
    /// then, if it is asked again, succeeds. Returns the number of commands it
    /// received, which is what distinguishes "retried" from "reported".
    async fn status_device(
        status_raw: u16,
        error_replies: usize,
    ) -> (SocketAddr, tokio::task::JoinHandle<usize>) {
        let sock = UdpSocket::bind("127.0.0.1:0").await.expect("bind");
        let addr = sock.local_addr().expect("addr");
        let handle = tokio::spawn(async move {
            let mut buf = [0u8; 2048];
            let mut commands = 0usize;
            loop {
                let recv =
                    time::timeout(Duration::from_millis(600), sock.recv_from(&mut buf)).await;
                let Ok(Ok((_len, peer))) = recv else { break };
                commands += 1;
                let request_id = u16::from_be_bytes([buf[6], buf[7]]);
                let mut ack = Vec::new();
                if commands <= error_replies {
                    ack.extend_from_slice(&status_raw.to_be_bytes());
                    ack.extend_from_slice(&0x0081u16.to_be_bytes());
                    ack.extend_from_slice(&0u16.to_be_bytes());
                    ack.extend_from_slice(&request_id.to_be_bytes());
                } else {
                    ack.extend_from_slice(&0u16.to_be_bytes());
                    ack.extend_from_slice(&0x0081u16.to_be_bytes());
                    ack.extend_from_slice(&4u16.to_be_bytes());
                    ack.extend_from_slice(&request_id.to_be_bytes());
                    ack.extend_from_slice(&0xCAFEBABEu32.to_be_bytes());
                }
                sock.send_to(&ack, peer).await.expect("send ack");
            }
            commands
        });
        (addr, handle)
    }

    #[tokio::test]
    async fn busy_is_retried() {
        // 0x8007 BUSY is congestion: the command can succeed if asked again.
        let (addr, server) = status_device(0x8007, 1).await;
        let mut device = GigeDevice::open(addr).await.expect("open");
        let value = device.read_register(0x0a00).await.expect("read register");
        assert_eq!(value, 0xCAFEBABE);
        assert_eq!(
            server.await.expect("join"),
            2,
            "BUSY must be retried, so the device sees a second command"
        );
    }

    #[tokio::test]
    async fn write_protect_is_reported_not_retried() {
        // 0x8004 WRITE_PROTECT is permanent. It used to be decoded as
        // `DeviceBusy`, so the retry loop spent its whole budget on a register
        // that could never accept the write.
        let (addr, server) = status_device(0x8004, usize::MAX).await;
        let mut device = GigeDevice::open(addr).await.expect("open");
        let err = device
            .read_register(0x0a00)
            .await
            .expect_err("write protect must surface");
        assert!(
            matches!(err, GigeError::Status(StatusCode::WriteProtect)),
            "expected WRITE_PROTECT, got {err:?}"
        );
        assert_eq!(
            server.await.expect("join"),
            1,
            "a permanent refusal must not be retried"
        );
    }

    #[tokio::test]
    async fn access_denied_names_itself_in_the_error() {
        // The #45 case: the user saw `Unknown(32774)` and could tell us nothing.
        let (addr, server) = status_device(0x8006, usize::MAX).await;
        let mut device = GigeDevice::open(addr).await.expect("open");
        let err = device
            .read_register(0x0a00)
            .await
            .expect_err("access denied must surface");
        assert_eq!(
            err.to_string(),
            "device reported status ACCESS_DENIED (0x8006)"
        );
        assert_eq!(server.await.expect("join"), 1);
    }

    #[test]
    fn request_header_roundtrip() {
        let header = GvcpRequestHeader {
            flags: CommandFlags::ACK_REQUIRED,
            command: 0x1234,
            length: 4,
            request_id: 0xBEEF,
        };
        let payload = [1u8, 2, 3, 4];
        let encoded = header.encode(&payload);
        assert_eq!(encoded.len(), viva_gencp::HEADER_SIZE + payload.len());
        // GVCP wire format: byte 0 = 0x42 key, byte 1 = flags byte.
        assert_eq!(encoded[0], GVCP_CMD_KEY);
        assert_eq!(encoded[1], 0x01); // ACK_REQUIRED
        assert_eq!(&encoded[2..4], &header.command.to_be_bytes());
        assert_eq!(&encoded[4..6], &header.length.to_be_bytes());
        assert_eq!(&encoded[6..8], &header.request_id.to_be_bytes());
        assert_eq!(&encoded[8..], &payload);
    }

    #[test]
    fn forceip_payload_encoding() {
        let mac = [0xDE, 0xAD, 0xBE, 0xEF, 0xCA, 0xFE];
        let ip = Ipv4Addr::new(192, 168, 1, 100);
        let subnet = Ipv4Addr::new(255, 255, 255, 0);
        let gateway = Ipv4Addr::new(192, 168, 1, 1);
        let payload = encode_forceip_payload(mac, ip, subnet, gateway);
        assert_eq!(payload.len(), 56);
        // MAC at offset 2..8
        assert_eq!(&payload[2..8], &mac);
        // IP at offset 20..24
        assert_eq!(&payload[20..24], &ip.octets());
        // Subnet at offset 36..40
        assert_eq!(&payload[36..40], &subnet.octets());
        // Gateway at offset 52..56
        assert_eq!(&payload[52..56], &gateway.octets());
        // Reserved bytes should be zero
        assert_eq!(&payload[0..2], &[0, 0]);
        assert_eq!(&payload[8..20], &[0u8; 12]);
        assert_eq!(&payload[24..36], &[0u8; 12]);
        assert_eq!(&payload[40..52], &[0u8; 12]);
    }

    #[test]
    fn ack_header_conversion() {
        let ack = AckHeader {
            status: StatusCode::Busy,
            opcode: OpCode::ReadMem,
            length: 12,
            request_id: 0x44,
        };
        let converted = GvcpAckHeader::from(ack);
        assert_eq!(converted.status, StatusCode::Busy);
        assert_eq!(converted.command, OpCode::ReadMem.ack_code());
        assert_eq!(converted.length, 12);
        assert_eq!(converted.request_id, 0x44);
    }
}
