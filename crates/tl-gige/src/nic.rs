//! Network helper utilities for GigE Vision streaming.

use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use tokio::net::UdpSocket;

/// Bind a UDP socket suitable for receiving GVSP packets.
pub async fn bind_stream_socket(bind_ip: IpAddr, port: u16) -> io::Result<UdpSocket> {
    let addr = SocketAddr::new(bind_ip, port);
    let socket = UdpSocket::bind(addr).await?;
    Ok(socket)
}

/// Compute the maximum GVSP payload size for a given MTU.
pub fn max_payload_from_mtu(mtu: u32) -> u32 {
    // IPv4 header (20) + UDP header (8) + GVSP header (~8).
    let overhead = 20 + 8 + 8;
    mtu.saturating_sub(overhead)
}

/// Set a jumbo frame hint on the interface (documentation placeholder).
pub fn set_jumbo_hint(_iface: &str, _enabled: bool) {
    tracing::info!("jumbo frame hint not implemented; configure via OS tools");
}

/// Helper returning the default bind address for discovery convenience.
pub fn default_bind_addr() -> IpAddr {
    IpAddr::V4(Ipv4Addr::UNSPECIFIED)
}
