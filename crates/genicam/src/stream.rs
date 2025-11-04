//! Streaming builder and configuration helpers bridging `tl-gige` with
//! higher-level GenICam consumers.
//!
//! The builder performs control-plane negotiation (packet size, delay) and
//! prepares a UDP socket configured for reception. Applications can retrieve the
//! socket handle to drive their own async pipelines while relying on the shared
//! [`StreamStats`] accumulator for monitoring.

use std::net::{IpAddr, Ipv4Addr};

use tokio::net::UdpSocket;
use tracing::info;

use crate::GenicamError;
use tl_gige::gvcp::{GigeDevice, StreamParams};
use tl_gige::gvsp::StreamConfig;
use tl_gige::nic::{self, Iface, McOptions, DEFAULT_RCVBUF_BYTES};
use tl_gige::stats::{StreamStats, StreamStatsAccumulator};

pub use tl_gige::gvsp::StreamDest;

/// Builder for configuring a GVSP stream.
pub struct StreamBuilder<'a> {
    device: &'a mut GigeDevice,
    iface: Option<Iface>,
    dest: Option<StreamDest>,
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
            dest: None,
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

    /// Configure the stream destination.
    pub fn dest(mut self, dest: StreamDest) -> Self {
        self.dest = Some(dest);
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
        if let Some(dest) = &mut self.dest {
            *dest = match *dest {
                StreamDest::Unicast { dst_ip, .. } => StreamDest::Unicast {
                    dst_ip,
                    dst_port: port,
                },
                StreamDest::Multicast {
                    group,
                    loopback,
                    ttl,
                    ..
                } => StreamDest::Multicast {
                    group,
                    port,
                    loopback,
                    ttl,
                },
            };
        }
        self
    }

    /// Configure multicast reception when the device is set to multicast mode.
    pub fn multicast(mut self, group: Option<Ipv4Addr>) -> Self {
        if let Some(group) = group {
            self.dest = Some(StreamDest::Multicast {
                group,
                port: self.dst_port,
                loopback: false,
                ttl: 1,
            });
        } else {
            self.dest = None;
        }
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
        let default_port = if self.dst_port == 0 {
            0x5FFF
        } else {
            self.dst_port
        };
        let mut dest = self.dest.unwrap_or(StreamDest::Unicast {
            dst_ip: host_ip,
            dst_port: default_port,
        });
        match &mut dest {
            StreamDest::Unicast { dst_port, .. } => {
                if *dst_port == 0 {
                    *dst_port = default_port;
                }
            }
            StreamDest::Multicast { port, .. } => {
                if *port == 0 {
                    *port = default_port;
                }
            }
        }

        let iface_mtu = nic::mtu(&iface).map_err(|err| GenicamError::transport(err.to_string()))?;
        let mtu = self
            .target_mtu
            .map_or(iface_mtu, |limit| limit.min(iface_mtu));
        let packet_size = if self.auto_packet_size {
            nic::best_packet_size(mtu)
        } else {
            self.packet_size
                .unwrap_or_else(|| nic::best_packet_size(1500))
        };
        let packet_delay = if self.auto_packet_size {
            if mtu <= 1500 {
                const DELAY_NS: u32 = 2_000;
                DELAY_NS / 80
            } else {
                0
            }
        } else {
            self.packet_delay.unwrap_or(0)
        };

        match &dest {
            StreamDest::Unicast { dst_ip, dst_port } => {
                info!(%dst_ip, dst_port, channel = self.channel, "configuring unicast stream");
                self.device
                    .set_stream_destination(self.channel, *dst_ip, *dst_port)
                    .await
                    .map_err(|err| GenicamError::transport(err.to_string()))?;
            }
            StreamDest::Multicast { .. } => {
                info!(
                    channel = self.channel,
                    port = dest.port(),
                    addr = %dest.addr(),
                    "configuring multicast stream parameters"
                );
            }
        }

        self.device
            .set_stream_packet_size(self.channel, packet_size)
            .await
            .map_err(|err| GenicamError::transport(err.to_string()))?;
        self.device
            .set_stream_packet_delay(self.channel, packet_delay)
            .await
            .map_err(|err| GenicamError::transport(err.to_string()))?;

        let socket = match &dest {
            StreamDest::Unicast { dst_port, .. } => {
                let bind_ip = IpAddr::V4(Ipv4Addr::UNSPECIFIED);
                nic::bind_udp(bind_ip, *dst_port, Some(iface.clone()), self.rcvbuf_bytes)
                    .await
                    .map_err(|err| GenicamError::transport(err.to_string()))?
            }
            StreamDest::Multicast {
                group,
                port,
                loopback,
                ttl,
            } => {
                let mut opts = McOptions::default();
                opts.loopback = *loopback;
                opts.ttl = *ttl;
                opts.rcvbuf_bytes = self.rcvbuf_bytes.unwrap_or(DEFAULT_RCVBUF_BYTES);
                nic::bind_multicast(&iface, *group, *port, &opts)
                    .await
                    .map_err(|err| GenicamError::transport(err.to_string()))?
            }
        };

        let source_filter = if dest.is_multicast() {
            None
        } else {
            Some(dest.addr())
        };
        let resend_enabled = !dest.is_multicast();

        let params = StreamParams {
            packet_size,
            packet_delay,
            mtu,
            host: dest.addr(),
            port: dest.port(),
        };

        let config = StreamConfig {
            dest,
            iface: iface.clone(),
            packet_size: Some(packet_size),
            packet_delay: Some(packet_delay),
            source_filter,
            resend_enabled,
        };

        let stats = StreamStatsAccumulator::new();
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
    stats: StreamStatsAccumulator,
    params: StreamParams,
    config: StreamConfig,
}

impl Stream {
    /// Borrow the underlying UDP socket.
    pub fn socket(&self) -> &UdpSocket {
        &self.socket
    }

    /// Consume the stream and return the UDP socket together with the shared statistics handle.
    pub fn into_parts(
        self,
    ) -> (
        UdpSocket,
        StreamStatsAccumulator,
        StreamParams,
        StreamConfig,
    ) {
        (self.socket, self.stats, self.params, self.config)
    }

    /// Access the negotiated stream parameters.
    pub fn params(&self) -> StreamParams {
        self.params
    }

    /// Obtain a clone of the statistics accumulator handle for updates.
    pub fn stats_handle(&self) -> StreamStatsAccumulator {
        self.stats.clone()
    }

    /// Snapshot the collected statistics.
    pub fn stats(&self) -> StreamStats {
        self.stats.snapshot()
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
