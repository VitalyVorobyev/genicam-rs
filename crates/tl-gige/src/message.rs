//! GVCP message/event channel handling.

use std::io;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use bytes::{Buf, Bytes};
use socket2::{Domain, Protocol, Socket, Type};
use thiserror::Error;
use tokio::net::UdpSocket;
use tokio::sync::Mutex;
use tracing::{debug, info, trace, warn};

#[cfg(test)]
use crate::gvcp::consts;

/// Default size of the receive buffer requested for the event socket.
const DEFAULT_RCVBUF: usize = 1 << 20; // 1 MiB.
/// GVCP message header length in bytes.
const MESSAGE_HEADER_LEN: usize = 8;
/// GVCP event message payload header length (following the GVCP header).
const EVENT_HEADER_LEN: usize = 20;
/// Maximum size of a GVCP event packet we are willing to process.
const MAX_EVENT_PACKET: usize = 2048;

/// Errors that can occur while operating the GVCP message channel.
#[derive(Debug, Error)]
pub enum MessageError {
    #[error("io: {0}")]
    Io(#[from] io::Error),
    #[error("protocol: {0}")]
    Protocol(&'static str),
    #[error("invalid packet: {0}")]
    Invalid(&'static str),
}

/// Parsed representation of an incoming GVCP event packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessagePacket {
    pub src: SocketAddr,
    pub event_id: u16,
    pub ts_dev: u64,
    pub stream_channel: u16,
    pub block_id: u16,
    pub payload: Bytes,
}

impl MessagePacket {
    fn parse(src: SocketAddr, data: &[u8]) -> Result<Self, MessageError> {
        if data.len() < MESSAGE_HEADER_LEN + EVENT_HEADER_LEN {
            return Err(MessageError::Invalid("packet too short"));
        }
        if data.len() > MAX_EVENT_PACKET {
            return Err(MessageError::Invalid("packet too large"));
        }
        let mut cursor = data;
        let status = cursor.get_u16();
        let opcode = cursor.get_u16();
        let length = cursor.get_u16() as usize;
        let _request_id = cursor.get_u16();

        if status != 0 {
            return Err(MessageError::Protocol("device reported failure"));
        }
        const EVENT_DATA_ACK: u16 = 0x000D;
        if opcode != EVENT_DATA_ACK {
            return Err(MessageError::Invalid("unexpected opcode"));
        }
        if length + MESSAGE_HEADER_LEN != data.len() {
            return Err(MessageError::Invalid("length mismatch"));
        }

        let event_id = cursor.get_u16();
        let _notification = cursor.get_u16();
        let ts_high = cursor.get_u32() as u64;
        let ts_low = cursor.get_u32() as u64;
        let ts_dev = (ts_high << 32) | ts_low;
        let stream_channel = cursor.get_u16();
        let block_id = cursor.get_u16();
        let payload_length = cursor.get_u16() as usize;
        let _reserved = cursor.get_u16();

        if EVENT_HEADER_LEN + payload_length != length {
            return Err(MessageError::Invalid("payload length mismatch"));
        }
        if cursor.remaining() != payload_length {
            return Err(MessageError::Invalid("payload truncated"));
        }
        let payload = cursor.copy_to_bytes(payload_length);
        Ok(Self {
            src,
            event_id,
            ts_dev,
            stream_channel,
            block_id,
            payload,
        })
    }
}

/// Async wrapper around a GVCP message socket.
pub struct EventSocket {
    socket: Arc<UdpSocket>,
    buffer: Mutex<Vec<u8>>,
}

impl EventSocket {
    /// Bind a new GVCP event socket on the provided local address.
    pub async fn bind(local_ip: IpAddr, port: u16) -> Result<Self, MessageError> {
        let domain = match local_ip {
            IpAddr::V4(_) => Domain::IPV4,
            IpAddr::V6(_) => Domain::IPV6,
        };
        let socket = Socket::new(domain, Type::DGRAM, Some(Protocol::UDP))?;
        socket.set_reuse_address(true)?;
        socket.set_nonblocking(true)?;
        if let Err(err) = socket.set_recv_buffer_size(DEFAULT_RCVBUF) {
            warn!(?err, "failed to set event socket receive buffer");
        }
        let addr = SocketAddr::new(local_ip, port);
        socket.bind(&addr.into())?;
        let udp = UdpSocket::from_std(socket.into())?;
        info!(local = %addr, "bound GVCP event socket");
        Ok(Self {
            socket: Arc::new(udp),
            buffer: Mutex::new(vec![0u8; MAX_EVENT_PACKET]),
        })
    }

    /// Receive and parse the next event packet, skipping malformed datagrams.
    pub async fn recv_event(&self) -> Result<MessagePacket, MessageError> {
        loop {
            let mut guard = self.buffer.lock().await;
            let (len, src) = self.socket.recv_from(&mut guard[..]).await?;
            trace!(bytes = len, %src, "received raw event packet");
            match MessagePacket::parse(src, &guard[..len]) {
                Ok(packet) => {
                    debug!(event_id = packet.event_id, %src, "parsed event packet");
                    return Ok(packet);
                }
                Err(err) => {
                    warn!(%src, ?err, "discarding malformed event packet");
                    continue;
                }
            }
        }
    }

    /// Access the underlying UDP socket (for tests).
    pub fn socket(&self) -> Arc<UdpSocket> {
        Arc::clone(&self.socket)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    #[tokio::test]
    async fn parse_valid_event() {
        let src = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), consts::PORT);
        let mut payload = Vec::new();
        payload.extend_from_slice(&0u16.to_be_bytes());
        payload.extend_from_slice(&0x000Du16.to_be_bytes());
        let length_placeholder = payload.len();
        payload.extend_from_slice(&0u16.to_be_bytes());
        payload.extend_from_slice(&0xCAFEu16.to_be_bytes());
        payload.extend_from_slice(&0x1234u16.to_be_bytes());
        payload.extend_from_slice(&0x0001u16.to_be_bytes());
        payload.extend_from_slice(&0x0002_0003u32.to_be_bytes());
        payload.extend_from_slice(&0x0004_0005u32.to_be_bytes());
        payload.extend_from_slice(&7u16.to_be_bytes());
        payload.extend_from_slice(&8u16.to_be_bytes());
        payload.extend_from_slice(&4u16.to_be_bytes());
        payload.extend_from_slice(&0u16.to_be_bytes());
        payload.extend_from_slice(&[1, 2, 3, 4]);
        let length = (payload.len() - MESSAGE_HEADER_LEN) as u16;
        payload[length_placeholder..length_placeholder + 2].copy_from_slice(&length.to_be_bytes());
        let packet = MessagePacket::parse(src, &payload).expect("valid packet");
        assert_eq!(packet.event_id, 0x1234);
        assert_eq!(packet.stream_channel, 7);
        assert_eq!(packet.block_id, 8);
        assert_eq!(packet.payload.len(), 4);
    }

    #[tokio::test]
    async fn reject_short_packet() {
        let src = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), consts::PORT);
        let payload = vec![0u8; 4];
        let err = MessagePacket::parse(src, &payload).unwrap_err();
        assert!(matches!(err, MessageError::Invalid("packet too short")));
    }
}
