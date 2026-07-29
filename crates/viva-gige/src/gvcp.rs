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

    /// Address of the SFNC `GevMessageChannel0DestinationAddress` register.
    pub const MESSAGE_DESTINATION_ADDRESS: u64 = 0x0900_0200;
    /// Address of the SFNC `GevMessageChannel0DestinationPort` register.
    pub const MESSAGE_DESTINATION_PORT: u64 = 0x0900_0204;
    /// Base address of the event notification mask (`GevEventNotificationAll`).
    pub const EVENT_NOTIFICATION_BASE: u64 = 0x0900_0300;
    /// Stride between successive event notification mask registers (bytes).
    pub const EVENT_NOTIFICATION_STRIDE: u64 = 4;

    /// Maximum number of bytes we read per GenCP `ReadMem` operation.
    pub const GENCP_MAX_BLOCK: usize = 512;
    /// Additional bytes that accompany a GenCP `WriteMem` block.
    pub const GENCP_WRITE_OVERHEAD: usize = 8;

    /// Default timeout for control transactions.
    pub const CONTROL_TIMEOUT: Duration = Duration::from_millis(500);
    /// Maximum number of automatic retries for a control transaction.
    pub const MAX_RETRIES: usize = 4;
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
    fn gvcp_flags_byte(&self) -> u8 {
        let mut byte = 0u8;
        if self.flags.contains(CommandFlags::ACK_REQUIRED) {
            byte |= 0x01;
        }
        if self.flags.contains(CommandFlags::BROADCAST) {
            byte |= 0x10;
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
    #[error("device reported status {0:?}")]
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
            match time::timeout(consts::CONTROL_TIMEOUT, self.socket.recv(&mut buf)).await {
                Ok(Ok(len)) => {
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
                        StatusCode::DeviceBusy if attempt < consts::MAX_RETRIES => {
                            warn!(request_id, attempt, "device busy, retrying");
                            self.backoff(attempt).await;
                            payload = BytesMut::from(&payload_bytes[..]);
                            continue;
                        }
                        other => return Err(GigeError::Status(other)),
                    }
                }
                Ok(Err(err)) => {
                    if attempt >= consts::MAX_RETRIES {
                        return Err(err.into());
                    }
                    warn!(request_id, ?opcode, attempt, "receive error, retrying");
                    self.backoff(attempt).await;
                    payload = BytesMut::from(&payload_bytes[..]);
                }
                Err(_) => {
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
        self.write_mem(consts::MESSAGE_DESTINATION_PORT, &port.to_be_bytes())
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

    /// Enable or disable delivery of the provided event identifier.
    pub async fn enable_event_raw(&mut self, id: u16, on: bool) -> Result<(), GigeError> {
        let index = (id / 32) as u64;
        let bit = 1u32 << (id % 32);
        let addr = consts::EVENT_NOTIFICATION_BASE + index * consts::EVENT_NOTIFICATION_STRIDE;
        let current = self.read_mem(addr, 4).await?;
        if current.len() != 4 {
            return Err(GigeError::Protocol(
                "event notification register length mismatch".into(),
            ));
        }
        let mut bytes = [0u8; 4];
        bytes.copy_from_slice(&current);
        let mut value = u32::from_be_bytes(bytes);
        if on {
            value |= bit;
        } else {
            value &= !bit;
        }
        let new_bytes = value.to_be_bytes();
        self.write_mem(addr, &new_bytes).await?;
        debug!(event_id = id, enabled = on, "updated event mask");
        Ok(())
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
            status: StatusCode::DeviceBusy,
            opcode: OpCode::ReadMem,
            length: 12,
            request_id: 0x44,
        };
        let converted = GvcpAckHeader::from(ack);
        assert_eq!(converted.status, StatusCode::DeviceBusy);
        assert_eq!(converted.command, OpCode::ReadMem.ack_code());
        assert_eq!(converted.length, 12);
        assert_eq!(converted.request_id, 0x44);
    }
}
