//! GVCP action command helpers.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::{Duration, Instant};

use bytes::{BufMut, BytesMut};
use thiserror::Error;
use tokio::net::UdpSocket;
use tokio::time;
use tracing::{debug, info, trace, warn};

use crate::gvcp::{GvcpAckHeader, GvcpRequestHeader};

/// Opcode for GVCP action command.
const ACTION_COMMAND: u16 = 0x0080;
/// Opcode for GVCP action acknowledgement.
const ACTION_ACK: u16 = 0x0081;
/// Size of an action command payload in bytes.
const ACTION_PAYLOAD_SIZE: usize = 24;
/// Default timeout while collecting acknowledgements.
const ACTION_ACK_TIMEOUT: Duration = Duration::from_millis(150);
/// Maximum number of acknowledgement datagrams we will accept per command.
const ACTION_MAX_ACKS: usize = 64;

/// Errors that can be raised while issuing an action command.
#[derive(Debug, Error)]
pub enum ActionError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("protocol: {0}")]
    Protocol(&'static str),
}

/// Parameters controlling an action command dispatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionParams {
    pub device_key: u32,
    pub group_key: u32,
    pub group_mask: u32,
    pub scheduled_time: Option<u64>,
    pub channel: u16,
}

/// Summary of a broadcast action dispatch.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct AckSummary {
    pub sent: usize,
    pub acks: usize,
}

fn encode_action_payload(params: &ActionParams) -> BytesMut {
    let mut payload = BytesMut::with_capacity(ACTION_PAYLOAD_SIZE);
    payload.put_u32(params.device_key);
    payload.put_u32(params.group_key);
    payload.put_u32(params.group_mask);
    let scheduled = params.scheduled_time.unwrap_or(0);
    payload.put_u32((scheduled >> 32) as u32);
    payload.put_u32(scheduled as u32);
    payload.put_u16(params.channel);
    payload.put_u16(0); // reserved
    payload
}

fn is_broadcast(addr: SocketAddr) -> bool {
    matches!(addr.ip(), IpAddr::V4(ip) if ip == Ipv4Addr::BROADCAST)
}

fn parse_ack(buffer: &[u8]) -> Result<GvcpAckHeader, ActionError> {
    if buffer.len() < 8 {
        return Err(ActionError::Protocol("acknowledgement too small"));
    }
    let status = u16::from_be_bytes([buffer[0], buffer[1]]);
    let opcode = u16::from_be_bytes([buffer[2], buffer[3]]);
    let length = u16::from_be_bytes([buffer[4], buffer[5]]);
    let request_id = u16::from_be_bytes([buffer[6], buffer[7]]);
    Ok(GvcpAckHeader {
        status: genicp::StatusCode::from_raw(status),
        command: opcode,
        length,
        request_id,
    })
}

/// Send a GVCP action command and collect acknowledgements.
pub async fn send_action(
    destination: SocketAddr,
    params: &ActionParams,
) -> Result<AckSummary, ActionError> {
    let local = SocketAddr::new(match destination.ip() {
        IpAddr::V4(_) => IpAddr::V4(Ipv4Addr::UNSPECIFIED),
        IpAddr::V6(_) => {
            return Err(ActionError::Protocol("IPv6 destinations are not supported"));
        }
    }, 0);
    let socket = UdpSocket::bind(local).await?;
    if is_broadcast(destination) {
        socket.set_broadcast(true)?;
    }
    let mut summary = AckSummary { sent: 0, acks: 0 };
    let request_id = fastrand::u16(0x8000..=0xFFFE);
    let flags = if is_broadcast(destination) {
        genicp::CommandFlags::ACK_REQUIRED | genicp::CommandFlags::BROADCAST
    } else {
        genicp::CommandFlags::ACK_REQUIRED
    };
    let payload = encode_action_payload(params);
    let header = GvcpRequestHeader {
        flags,
        command: ACTION_COMMAND,
        length: payload.len() as u16,
        request_id,
    };
    let packet = header.encode(&payload);
    trace!(bytes = packet.len(), request_id, dest = %destination, "sending action command");
    socket.send_to(&packet, destination).await?;
    summary.sent = 1;

    let start = Instant::now();
    let mut buf = vec![0u8; 512];
    loop {
        let remaining = ACTION_ACK_TIMEOUT
            .checked_sub(start.elapsed())
            .unwrap_or_default();
        if remaining.is_zero() {
            break;
        }
        match time::timeout(remaining, socket.recv_from(&mut buf)).await {
            Ok(Ok((len, src))) => {
                if len < 8 {
                    warn!(bytes = len, %src, "ignoring short acknowledgement");
                    continue;
                }
                trace!(bytes = len, %src, "received action acknowledgement");
                let header = parse_ack(&buf[..len])?;
                if header.command != ACTION_ACK {
                    debug!(opcode = header.command, "ignoring non-action acknowledgement");
                    continue;
                }
                if header.request_id != request_id {
                    debug!(expected = request_id, got = header.request_id, "ack id mismatch");
                    continue;
                }
                if header.status != genicp::StatusCode::Success {
                    warn!(status = ?header.status, %src, "device reported failure to action");
                    continue;
                }
                summary.acks += 1;
                if summary.acks >= ACTION_MAX_ACKS {
                    break;
                }
            }
            Ok(Err(err)) => {
                warn!(?err, "error receiving acknowledgement");
                break;
            }
            Err(_) => break,
        }
    }
    info!(acks = summary.acks, "action command completed");
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_layout() {
        let params = ActionParams {
            device_key: 1,
            group_key: 2,
            group_mask: 3,
            scheduled_time: Some(0x0102_0304_0506_0708),
            channel: 9,
        };
        let payload = encode_action_payload(&params);
        assert_eq!(payload.len(), ACTION_PAYLOAD_SIZE);
        assert_eq!(&payload[..4], &1u32.to_be_bytes());
        assert_eq!(&payload[4..8], &2u32.to_be_bytes());
        assert_eq!(&payload[8..12], &3u32.to_be_bytes());
        assert_eq!(&payload[12..16], &0x0102_0304u32.to_be_bytes());
        assert_eq!(&payload[16..20], &0x0506_0708u32.to_be_bytes());
        assert_eq!(&payload[20..22], &9u16.to_be_bytes());
    }
}
