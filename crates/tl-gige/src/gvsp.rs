//! GVSP packet parsing and reassembly (placeholder implementation).

use bytes::{Buf, Bytes};
use thiserror::Error;

/// Errors raised while handling GVSP packets.
#[derive(Debug, Error)]
pub enum GvspError {
    #[error("unsupported packet type: {0}")]
    Unsupported(&'static str),
    #[error("invalid packet: {0}")]
    Invalid(&'static str),
}

/// Raw GVSP chunk extracted from a payload or trailer block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkRaw {
    pub id: u16,
    pub data: Bytes,
}

/// Parse a chunk payload following the `[id][reserved][length][data...]` layout.
pub fn parse_chunks(mut payload: &[u8]) -> Result<Vec<ChunkRaw>, GvspError> {
    let mut chunks = Vec::new();
    while !payload.is_empty() {
        if payload.len() < 8 {
            return Err(GvspError::Invalid("chunk header truncated"));
        }
        let mut cursor = &payload[..];
        let id = cursor.get_u16();
        let _reserved = cursor.get_u16();
        let length = cursor.get_u32() as usize;
        let total = 8 + length;
        if payload.len() < total {
            return Err(GvspError::Invalid("chunk data truncated"));
        }
        let data = Bytes::copy_from_slice(&payload[8..total]);
        chunks.push(ChunkRaw { id, data });
        payload = &payload[total..];
    }
    Ok(chunks)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_multiple_chunks() {
        let mut payload = Vec::new();
        payload.extend_from_slice(&0x0001u16.to_be_bytes());
        payload.extend_from_slice(&0u16.to_be_bytes());
        payload.extend_from_slice(&4u32.to_be_bytes());
        payload.extend_from_slice(&[1, 2, 3, 4]);
        payload.extend_from_slice(&0x0002u16.to_be_bytes());
        payload.extend_from_slice(&0u16.to_be_bytes());
        payload.extend_from_slice(&2u32.to_be_bytes());
        payload.extend_from_slice(&[5, 6]);
        let chunks = parse_chunks(&payload).expect("chunks");
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].id, 0x0001);
        assert_eq!(chunks[0].data.as_ref(), &[1, 2, 3, 4]);
        assert_eq!(chunks[1].id, 0x0002);
        assert_eq!(chunks[1].data.as_ref(), &[5, 6]);
    }

    #[test]
    fn reject_truncated_chunk() {
        let payload = vec![0u8; 6];
        let err = parse_chunks(&payload).unwrap_err();
        assert!(matches!(err, GvspError::Invalid("chunk header truncated")));
    }
}
