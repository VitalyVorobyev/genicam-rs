//! GVSP packet parsing and reassembly (placeholder implementation).

use bytes::Bytes;
use thiserror::Error;

/// Errors raised while handling GVSP packets.
#[derive(Debug, Error)]
pub enum GvspError {
    #[error("unsupported packet type: {0}")]
    Unsupported(&'static str),
    #[error("invalid packet: {0}")]
    Invalid(&'static str),
}

/// Representation of a GVSP packet.
#[derive(Debug, Clone)]
pub enum GvspPacket {
    /// Start-of-frame leader packet with metadata.
    Leader {
        block_id: u16,
        packet_id: u16,
        payload_type: u8,
        timestamp: u64,
        width: u32,
        height: u32,
        pixel_format: u32,
    },
    /// Payload data packet carrying pixel bytes.
    Payload {
        block_id: u16,
        packet_id: u16,
        data: Bytes,
    },
    /// End-of-frame trailer packet.
    Trailer {
        block_id: u16,
        packet_id: u16,
        status: u16,
    },
}

/// Parse a raw UDP payload into a GVSP packet.
pub fn parse_packet(_payload: &[u8]) -> Result<GvspPacket, GvspError> {
    Err(GvspError::Unsupported("GVSP parser not yet implemented"))
}
