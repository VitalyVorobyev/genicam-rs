//! Streaming builder and configuration helpers bridging `tl-gige` with
//! higher-level GenICam consumers.
//!
//! The builder performs control-plane negotiation (packet size, delay) and
//! prepares a UDP socket configured for reception. Applications can retrieve the
//! socket handle to drive their own async pipelines while relying on the shared
//! [`StreamStats`] accumulator for monitoring.

use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;

use tokio::net::UdpSocket;
use tracing::info;

use crate::GenicamError;
use tl_gige::gvcp::{GigeDevice, StreamParams};
use tl_gige::gvsp::StreamConfig;
use tl_gige::nic::{self, Iface};
use tl_gige::stats::StreamStats;

/// Builder for configuring a GVSP stream.
pub struct StreamBuilder<'a> {
    device: &'a mut GigeDevice,
    iface: Option<Iface>,
    multicast: Option<Ipv4Addr>,
    rcvbuf_bytes: Option<usize>,
    auto_packet_size: bool,
    target_mtu: Option<u32>,
    packet_size: Option<u32>,
    packet_delay: Option<u32>,
    channel: u32,
    dst_port: u16,
}

impl<'a> StreamBuilder<'a> {
    /// Create a new builder bound to an opened [`GigeDevice`].
    pub fn new(device: &'a mut GigeDevice) -> Self {
        Self {
            device,
            iface: None,
            multicast: None,
            rcvbuf_bytes: None,
            auto_packet_size: true,
            target_mtu: None,
            packet_size: None,
            packet_delay: None,
            channel: 0,
            dst_port: 0,
        }
    }

    /// Select the interface used for receiving GVSP packets.
    pub fn iface(mut self, iface: Iface) -> Self {
        self.iface = Some(iface);
        self
    }

    /// Enable or disable automatic packet-size negotiation.
    pub fn auto_packet_size(mut self, enable: bool) -> Self {
        self.auto_packet_size = enable;
        self
    }

    /// Target MTU used when computing the optimal GVSP packet size.
    pub fn target_mtu(mut self, mtu: u32) -> Self {
        self.target_mtu = Some(mtu);
        self
    }

    /// Override the GVSP packet size when automatic negotiation is disabled.
    pub fn packet_size(mut self, size: u32) -> Self {
        self.packet_size = Some(size);
        self
    }

    /// Override the GVSP packet delay when automatic negotiation is disabled.
    pub fn packet_delay(mut self, delay: u32) -> Self {
        self.packet_delay = Some(delay);
        self
    }

    /// Configure the UDP port used for streaming (defaults to 0 => device chosen).
    pub fn destination_port(mut self, port: u16) -> Self {
        self.dst_port = port;
        self
    }

    /// Configure multicast reception when the device is set to multicast mode.
    pub fn multicast(mut self, group: Option<Ipv4Addr>) -> Self {
        self.multicast = group;
        self
    }

    /// Custom receive buffer size for the UDP socket.
    pub fn rcvbuf_bytes(mut self, size: usize) -> Self {
        self.rcvbuf_bytes = Some(size);
        self
    }

    /// Select the GigE Vision stream channel to configure (defaults to 0).
    pub fn channel(mut self, channel: u32) -> Self {
        self.channel = channel;
        self
    }

    /// Finalise the builder and return a configured [`Stream`].
    pub async fn build(self) -> Result<Stream, GenicamError> {
        let iface = self
            .iface
            .ok_or_else(|| GenicamError::transport("stream requires a network interface"))?;
        let host_ip = iface
            .ipv4()
            .ok_or_else(|| GenicamError::transport("interface lacks IPv4 address"))?;
        let port = if self.dst_port == 0 {
            // Allow the device to pick a port, defaulting to the standard GVSP port range.
            0x5FFF
        } else {
            self.dst_port
        };

        let mut config = StreamConfig {
            multicast: self.multicast,
            iface: iface.clone(),
            dst_port: port,
            packet_size: None,
            packet_delay: None,
        };

        let params = if self.auto_packet_size {
            info!(%host_ip, port, channel = self.channel, "negotiating GVSP stream");
            let negotiated = self
                .device
                .negotiate_stream(self.channel, &iface, port, self.target_mtu)
                .await
                .map_err(|err| GenicamError::transport(err.to_string()))?;
            config.packet_size = Some(negotiated.packet_size);
            config.packet_delay = Some(negotiated.packet_delay);
            negotiated
        } else {
            let packet_size = self
                .packet_size
                .unwrap_or_else(|| nic::best_packet_size(1500));
            let packet_delay = self.packet_delay.unwrap_or(0);
            self.device
                .set_stream_destination(self.channel, host_ip, port)
                .await
                .map_err(|err| GenicamError::transport(err.to_string()))?;
            self.device
                .set_stream_packet_size(self.channel, packet_size)
                .await
                .map_err(|err| GenicamError::transport(err.to_string()))?;
            self.device
                .set_stream_packet_delay(self.channel, packet_delay)
                .await
                .map_err(|err| GenicamError::transport(err.to_string()))?;
            config.packet_size = Some(packet_size);
            config.packet_delay = Some(packet_delay);
            let mtu = nic::mtu(&iface).map_err(|err| GenicamError::transport(err.to_string()))?;
            StreamParams {
                packet_size,
                packet_delay,
                mtu,
                host: host_ip,
                port,
            }
        };

        let bind_ip = IpAddr::V4(Ipv4Addr::UNSPECIFIED);
        let socket = nic::bind_udp(bind_ip, port, Some(iface.clone()), self.rcvbuf_bytes)
            .await
            .map_err(|err| GenicamError::transport(err.to_string()))?;
        if let Some(group) = config.multicast {
            nic::join_multicast(&socket, group, &iface)
                .map_err(|err| GenicamError::transport(err.to_string()))?;
        }

        let stats = Arc::new(StreamStats::new());
        Ok(Stream {
            socket,
            stats,
            params,
            config,
        })
    }
}

/// Handle returned by [`StreamBuilder`] providing access to the configured socket
/// and statistics.
pub struct Stream {
    socket: UdpSocket,
    stats: Arc<StreamStats>,
    params: StreamParams,
    config: StreamConfig,
}

impl Stream {
    /// Borrow the underlying UDP socket.
    pub fn socket(&self) -> &UdpSocket {
        &self.socket
    }

    /// Consume the stream and return the UDP socket together with the shared statistics handle.
    pub fn into_parts(self) -> (UdpSocket, Arc<StreamStats>, StreamParams, StreamConfig) {
        (self.socket, self.stats, self.params, self.config)
    }

    /// Access the negotiated stream parameters.
    pub fn params(&self) -> StreamParams {
        self.params
    }

    /// Obtain a clone of the statistics accumulator.
    pub fn stats(&self) -> Arc<StreamStats> {
        Arc::clone(&self.stats)
    }

    /// Immutable view of the stream configuration.
    pub fn config(&self) -> &StreamConfig {
        &self.config
    }
}

impl<'a> From<&'a mut GigeDevice> for StreamBuilder<'a> {
    fn from(device: &'a mut GigeDevice) -> Self {
        StreamBuilder::new(device)
    }
}
