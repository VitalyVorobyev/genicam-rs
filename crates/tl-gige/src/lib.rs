//! GigE Vision TL: discovery (GVCP), control (GenCP/GVCP), streaming (GVSP).

use std::collections::HashMap;
use std::io::Cursor;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use bytes::{Buf, BufMut, BytesMut};
use genicp::{decode_ack, encode_cmd, CommandFlags, GenCpAck, GenCpCmd, OpCode, StatusCode};
use if_addrs::{get_if_addrs, IfAddr};
use thiserror::Error;
use tokio::net::UdpSocket;
use tokio::task::JoinSet;
use tokio::time;
use tracing::{info, trace};

/// GVCP control port as defined by the GigE Vision specification (section 7.3).
pub const GVCP_PORT: u16 = 3956;

const GVCP_DISCOVERY_COMMAND: u16 = 0x0002;
const GVCP_DISCOVERY_ACK: u16 = 0x0003;
const GENCP_HEADER_SIZE: usize = genicp::HEADER_SIZE;
const DISCOVERY_BUFFER: usize = 2048;
const CONTROL_TIMEOUT: Duration = Duration::from_millis(500);
const GENCP_MAX_BLOCK: usize = 512;
const GENCP_WRITE_OVERHEAD: usize = 8;

#[derive(Debug, Error)]
pub enum GigeError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("protocol: {0}")]
    Protocol(String),
    #[error("timeout waiting for acknowledgement")]
    Timeout,
    #[error("GenCP: {0}")]
    GenCp(#[from] genicp::GenCpError),
    #[error("device reported status {0:?}")]
    Status(StatusCode),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceInfo {
    pub ip: Ipv4Addr,
    pub mac: [u8; 6],
    pub model: Option<String>,
    pub manufacturer: Option<String>,
}

impl DeviceInfo {
    fn mac_string(&self) -> String {
        self.mac
            .iter()
            .map(|byte| format!("{byte:02X}"))
            .collect::<Vec<_>>()
            .join(":")
    }
}

/// Discover GigE Vision devices on the local network by broadcasting a GVCP discovery command.
pub async fn discover(timeout: Duration) -> Result<Vec<DeviceInfo>, GigeError> {
    discover_filtered(timeout, None).await
}

/// Discover devices only on the specified interface name.
pub async fn discover_on_interface(
    timeout: Duration,
    interface: &str,
) -> Result<Vec<DeviceInfo>, GigeError> {
    discover_filtered(timeout, Some(interface)).await
}

async fn discover_filtered(
    timeout: Duration,
    iface_filter: Option<&str>,
) -> Result<Vec<DeviceInfo>, GigeError> {
    let mut interfaces = Vec::new();
    for iface in get_if_addrs()? {
        let IfAddr::V4(v4) = iface.addr else {
            continue;
        };
        if v4.ip.is_loopback() {
            continue;
        }
        if let Some(filter) = iface_filter {
            if iface.name != filter {
                continue;
            }
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
            let socket = UdpSocket::bind(local_addr).await?;
            socket.set_broadcast(true)?;
            let broadcast = v4.broadcast.unwrap_or(Ipv4Addr::BROADCAST);
            let destination = SocketAddr::new(IpAddr::V4(broadcast), GVCP_PORT);

            let mut packet = BytesMut::with_capacity(GENCP_HEADER_SIZE);
            packet.put_u16((CommandFlags::ACK_REQUIRED | CommandFlags::BROADCAST).bits());
            packet.put_u16(GVCP_DISCOVERY_COMMAND);
            packet.put_u16(0);
            packet.put_u16(request_id);
            info!(%interface_name, local = %v4.ip, dest = %destination, "sending GVCP discovery");
            trace!(%interface_name, bytes = packet.len(), "GVCP discovery payload size");
            socket.send_to(&packet, destination).await?;

            let mut responses = Vec::new();
            let mut buffer = vec![0u8; DISCOVERY_BUFFER];
            let timer = time::sleep(timeout);
            tokio::pin!(timer);
            loop {
                tokio::select! {
                    _ = &mut timer => break,
                    recv = socket.recv_from(&mut buffer) => {
                        let (len, src) = recv?;
                        info!(%interface_name, %src, "received GVCP response");
                        trace!(%interface_name, bytes = len, "GVCP response length");
                        if let Some(info) = parse_discovery_ack(&buffer[..len], request_id)? {
                            trace!(ip = %info.ip, mac = %info.mac_string(), "parsed discovery ack");
                            responses.push(info);
                        }
                    }
                }
            }
            Ok::<_, GigeError>(responses)
        });
    }

    let mut seen = HashMap::new();
    while let Some(res) = join_set.join_next().await {
        let devices =
            res.map_err(|e| GigeError::Protocol(format!("discovery task failed: {e}")))??;
        for dev in devices {
            seen.entry((dev.ip, dev.mac)).or_insert(dev);
        }
    }

    let mut devices: Vec<_> = seen.into_values().collect();
    devices.sort_by_key(|d| d.ip);
    Ok(devices)
}

fn parse_discovery_ack(buf: &[u8], expected_request: u16) -> Result<Option<DeviceInfo>, GigeError> {
    if buf.len() < GENCP_HEADER_SIZE {
        return Err(GigeError::Protocol("GVCP ack too short".into()));
    }
    let mut header = buf;
    let status = header.get_u16();
    let command = header.get_u16();
    let length = header.get_u16() as usize;
    let request_id = header.get_u16();
    if request_id != expected_request {
        return Ok(None);
    }
    if command != GVCP_DISCOVERY_ACK {
        return Err(GigeError::Protocol(format!(
            "unexpected discovery opcode {command:#06x}"
        )));
    }
    if status != 0 {
        return Err(GigeError::Protocol(format!(
            "discovery returned status {status:#06x}"
        )));
    }
    if buf.len() < GENCP_HEADER_SIZE + length {
        return Err(GigeError::Protocol("discovery payload truncated".into()));
    }
    let payload = &buf[GENCP_HEADER_SIZE..GENCP_HEADER_SIZE + length];
    let info = parse_discovery_payload(payload)?;
    Ok(Some(info))
}

fn parse_discovery_payload(payload: &[u8]) -> Result<DeviceInfo, GigeError> {
    let mut cursor = Cursor::new(payload);
    if cursor.remaining() < 32 {
        return Err(GigeError::Protocol("discovery payload too small".into()));
    }
    let _spec_major = cursor.get_u16();
    let _spec_minor = cursor.get_u16();
    let _device_mode = cursor.get_u32();
    let _device_class = cursor.get_u16();
    let _device_capability = cursor.get_u16();
    let mut mac = [0u8; 6];
    cursor.copy_to_slice(&mut mac);
    let _ip_config_options = cursor.get_u16();
    let _ip_config_current = cursor.get_u16();
    let ip = Ipv4Addr::from(cursor.get_u32());
    let _subnet = cursor.get_u32();
    let _gateway = cursor.get_u32();
    let manufacturer = read_fixed_string(&mut cursor, 32)?;
    let model = read_fixed_string(&mut cursor, 32)?;
    // Skip device version, serial, user-defined name if present.
    let _ = skip_string(&mut cursor, 32);
    let _ = skip_string(&mut cursor, 16);
    let _ = skip_string(&mut cursor, 16);

    Ok(DeviceInfo {
        ip,
        mac,
        manufacturer,
        model,
    })
}

fn read_fixed_string(cursor: &mut Cursor<&[u8]>, len: usize) -> Result<Option<String>, GigeError> {
    if cursor.remaining() < len {
        return Err(GigeError::Protocol("discovery string truncated".into()));
    }
    let mut buf = vec![0u8; len];
    cursor.copy_to_slice(&mut buf);
    Ok(parse_string(&buf))
}

fn skip_string(cursor: &mut Cursor<&[u8]>, len: usize) -> Option<()> {
    if cursor.remaining() < len {
        return None;
    }
    cursor.advance(len);
    Some(())
}

fn parse_string(bytes: &[u8]) -> Option<String> {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    let slice = &bytes[..end];
    let s = String::from_utf8_lossy(slice).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

pub struct GigeDevice {
    socket: UdpSocket,
    remote: SocketAddr,
    request_id: u16,
}

impl GigeDevice {
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
        })
    }

    fn next_request_id(&mut self) -> u16 {
        let id = self.request_id;
        self.request_id = self.request_id.wrapping_add(1);
        if self.request_id == 0 {
            self.request_id = 1;
        }
        id
    }

    async fn transact(&mut self, opcode: OpCode, payload: BytesMut) -> Result<GenCpAck, GigeError> {
        let request_id = self.next_request_id();
        let payload_bytes = payload.freeze();
        let cmd = GenCpCmd {
            header: genicp::CommandHeader {
                flags: CommandFlags::ACK_REQUIRED,
                opcode,
                length: payload_bytes.len() as u16,
                request_id,
            },
            payload: payload_bytes,
        };
        let encoded = encode_cmd(&cmd);
        trace!(request_id, opcode = ?opcode, bytes = encoded.len(), "sending GenCP command");
        self.socket.send(&encoded).await?;

        let mut buf = vec![0u8; GENCP_HEADER_SIZE + GENCP_MAX_BLOCK + GENCP_WRITE_OVERHEAD];
        let len = match time::timeout(CONTROL_TIMEOUT, self.socket.recv(&mut buf)).await {
            Ok(Ok(len)) => len,
            Ok(Err(err)) => return Err(err.into()),
            Err(_) => return Err(GigeError::Timeout),
        };
        trace!(request_id, bytes = len, "received GenCP ack");
        let ack = decode_ack(&buf[..len])?;
        if ack.header.request_id != request_id {
            return Err(GigeError::Protocol("acknowledgement id mismatch".into()));
        }
        if ack.header.opcode != opcode {
            return Err(GigeError::Protocol(
                "unexpected opcode in acknowledgement".into(),
            ));
        }
        match ack.header.status {
            StatusCode::Success => Ok(ack),
            other => Err(GigeError::Status(other)),
        }
    }

    pub async fn read_mem(&mut self, addr: u64, len: usize) -> Result<Vec<u8>, GigeError> {
        let mut remaining = len;
        let mut offset = 0usize;
        let mut data = Vec::with_capacity(len);
        while remaining > 0 {
            let chunk = remaining.min(GENCP_MAX_BLOCK);
            let mut payload = BytesMut::with_capacity(12);
            payload.put_u64(addr + offset as u64);
            payload.put_u32(chunk as u32);
            let ack = self.transact(OpCode::ReadMem, payload).await?;
            if ack.payload.len() != chunk {
                return Err(GigeError::Protocol(format!(
                    "expected {chunk} bytes but device returned {}",
                    ack.payload.len()
                )));
            }
            data.extend_from_slice(&ack.payload);
            remaining -= chunk;
            offset += chunk;
        }
        Ok(data)
    }

    pub async fn write_mem(&mut self, addr: u64, data: &[u8]) -> Result<(), GigeError> {
        let mut offset = 0usize;
        while offset < data.len() {
            let chunk = (data.len() - offset).min(GENCP_MAX_BLOCK - GENCP_WRITE_OVERHEAD);
            if chunk == 0 {
                return Err(GigeError::Protocol("write chunk size is zero".into()));
            }
            let mut payload = BytesMut::with_capacity(GENCP_WRITE_OVERHEAD + chunk);
            payload.put_u64(addr + offset as u64);
            payload.extend_from_slice(&data[offset..offset + chunk]);
            let ack = self.transact(OpCode::WriteMem, payload).await?;
            if !ack.payload.is_empty() {
                return Err(GigeError::Protocol(
                    "write acknowledgement carried unexpected payload".into(),
                ));
            }
            offset += chunk;
        }
        Ok(())
    }

    pub fn remote_addr(&self) -> SocketAddr {
        self.remote
    }
}
