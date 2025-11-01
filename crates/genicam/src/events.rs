//! High-level event stream helpers built on the GVCP message channel.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use bytes::Bytes;
use tl_gige::{
    gvcp::GigeDevice,
    message::{EventSocket, MessageError},
    stats::EventStats,
};
use tracing::info;

use crate::GenicamError;

/// Public representation of a camera event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub id: u16,
    pub ts_dev: u64,
    pub data: Bytes,
}

/// Asynchronous stream over GVCP event packets.
pub struct EventStream {
    socket: EventSocket,
    stats: EventStats,
}

impl EventStream {
    pub fn new(socket: EventSocket) -> Self {
        Self {
            socket,
            stats: EventStats::new(),
        }
    }

    pub fn stats(&self) -> &EventStats {
        &self.stats
    }

    pub async fn next(&self) -> Result<Event, GenicamError> {
        match self.socket.recv_event().await {
            Ok(packet) => {
                self.stats.record_event();
                Ok(Event {
                    id: packet.event_id,
                    ts_dev: packet.ts_dev,
                    data: packet.payload,
                })
            }
            Err(err) => {
                self.stats.record_malformed();
                Err(GenicamError::transport(err.to_string()))
            }
        }
    }

    pub fn local_addr(&self) -> Option<SocketAddr> {
        self.socket.socket().local_addr().ok()
    }
}

/// Configure the remote device to send events to the provided socket.
pub async fn configure_message_channel(
    device: &mut GigeDevice,
    local_ip: Ipv4Addr,
    port: u16,
    events: &[u16],
) -> Result<(), GenicamError> {
    device
        .set_message_destination(local_ip, port)
        .await
        .map_err(|err| GenicamError::transport(err.to_string()))?;
    for &id in events {
        info!(event_id = id, "enabling event via GVCP");
        device
            .enable_event(id, true)
            .await
            .map_err(|err| GenicamError::transport(err.to_string()))?;
    }
    Ok(())
}

/// Bind an `EventSocket` with a large receive buffer.
pub async fn bind_event_socket(ip: IpAddr, port: u16) -> Result<EventSocket, GenicamError> {
    EventSocket::bind(ip, port)
        .await
        .map_err(|err: MessageError| GenicamError::transport(err.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[tokio::test]
    async fn bind_socket_localhost() {
        let socket = bind_event_socket(IpAddr::V4(Ipv4Addr::LOCALHOST), 0)
            .await
            .expect("bind");
        let addr = socket.socket().local_addr().unwrap();
        assert_eq!(addr.ip(), IpAddr::V4(Ipv4Addr::LOCALHOST));
    }
}
