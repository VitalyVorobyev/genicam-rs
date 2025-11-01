//! GigE Vision TL: discovery (GVCP), control (GenCP/GVCP), streaming (GVSP).
use thiserror::Error;
use std::net::{UdpSocket, SocketAddr};
use std::time::Duration;

#[derive(Debug, Error)]
pub enum GigeError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("protocol: {0}")]
    Protocol(&'static str),
}

#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub ip: String,
    pub mac: [u8; 6],
    pub model: Option<String>,
    pub manufacturer: Option<String>,
}

pub fn discover(timeout: Duration) -> Result<Vec<DeviceInfo>, GigeError> {
    // Broadcast GVCP discovery; parse replies
    // Placeholder: return empty; implement per spec.
    Ok(vec![])
}

pub struct GigeDevice {
    ctrl: UdpSocket, // GVCP control socket
    // stream sockets, state, etc.
}

impl GigeDevice {
    pub fn open(addr: SocketAddr) -> Result<Self, GigeError> {
        let sock = UdpSocket::bind("0.0.0.0:0")?;
        sock.set_read_timeout(Some(Duration::from_millis(500)))?;
        Ok(Self { ctrl: sock })
    }
    // read_mem, write_mem using GenCP over GVCP...
}
