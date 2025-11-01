//! Decode GVSP chunk payloads into typed values.

use std::collections::HashMap;

use bytes::Buf;
use tl_gige::gvsp::{self, ChunkRaw};
use thiserror::Error;
use tracing::trace;

/// Known chunk identifiers defined by SFNC.
pub mod ids {
    /// Timestamp chunk (device time in ticks).
    pub const TIMESTAMP: u16 = 0x0001;
    /// Exposure time chunk (in microseconds).
    pub const EXPOSURE_TIME: u16 = 0x0002;
    /// Gain chunk (linear gain value).
    pub const GAIN: u16 = 0x0003;
    /// Line status bitfield chunk.
    pub const LINE_STATUS_ALL: u16 = 0x0004;
}

/// Typed representation of known chunk kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChunkKind {
    Timestamp,
    ExposureTime,
    Gain,
    LineStatusAll,
}

/// Decoded value of a chunk entry.
#[derive(Debug, Clone, PartialEq)]
pub enum ChunkValue {
    Timestamp(u64),
    ExposureTime(f64),
    Gain(f64),
    LineStatusAll(u64),
}

pub type ChunkMap = HashMap<ChunkKind, ChunkValue>;

/// Errors that can occur while decoding chunk payloads.
#[derive(Debug, Error)]
pub enum ChunkError {
    #[error("gvsp: {0}")]
    Gvsp(#[from] gvsp::GvspError),
    #[error("invalid payload for chunk {0:#06x}")]
    InvalidPayload(u16),
}

pub fn decode_raw_chunks(chunks: &[ChunkRaw]) -> Result<ChunkMap, ChunkError> {
    let mut map = HashMap::new();
    for chunk in chunks {
        trace!(chunk_id = chunk.id, len = chunk.data.len(), "decoding chunk");
        match chunk.id {
            ids::TIMESTAMP => {
                if chunk.data.len() != 8 {
                    return Err(ChunkError::InvalidPayload(chunk.id));
                }
                let mut buf = chunk.data.clone();
                let value = buf.get_u64();
                map.insert(ChunkKind::Timestamp, ChunkValue::Timestamp(value));
            }
            ids::EXPOSURE_TIME => {
                if chunk.data.len() != 8 {
                    return Err(ChunkError::InvalidPayload(chunk.id));
                }
                let mut buf = chunk.data.clone();
                let value = buf.get_f64();
                map.insert(ChunkKind::ExposureTime, ChunkValue::ExposureTime(value));
            }
            ids::GAIN => {
                if chunk.data.len() != 8 {
                    return Err(ChunkError::InvalidPayload(chunk.id));
                }
                let mut buf = chunk.data.clone();
                let value = buf.get_f64();
                map.insert(ChunkKind::Gain, ChunkValue::Gain(value));
            }
            ids::LINE_STATUS_ALL => {
                let mut bytes = [0u8; 8];
                let len = chunk.data.len().min(bytes.len());
                bytes[..len].copy_from_slice(&chunk.data[..len]);
                let value = u64::from_be_bytes(bytes);
                map.insert(ChunkKind::LineStatusAll, ChunkValue::LineStatusAll(value));
            }
            _ => {}
        }
    }
    Ok(map)
}

/// Parse raw bytes into chunks and decode known values.
pub fn parse_chunk_bytes(data: &[u8]) -> Result<ChunkMap, ChunkError> {
    let raw = gvsp::parse_chunks(data)?;
    decode_raw_chunks(&raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_known_chunks() {
        let mut data = Vec::new();
        data.extend_from_slice(&ids::TIMESTAMP.to_be_bytes());
        data.extend_from_slice(&0u16.to_be_bytes());
        data.extend_from_slice(&8u32.to_be_bytes());
        data.extend_from_slice(&0x1234_5678_9ABC_DEF0u64.to_be_bytes());
        data.extend_from_slice(&ids::EXPOSURE_TIME.to_be_bytes());
        data.extend_from_slice(&0u16.to_be_bytes());
        data.extend_from_slice(&8u32.to_be_bytes());
        data.extend_from_slice(&1.5f64.to_be_bytes());
        let map = parse_chunk_bytes(&data).expect("decode");
        assert!(matches!(
            map.get(&ChunkKind::Timestamp),
            Some(ChunkValue::Timestamp(0x1234_5678_9ABC_DEF0))
        ));
        assert!(matches!(
            map.get(&ChunkKind::ExposureTime),
            Some(ChunkValue::ExposureTime(v)) if (*v - 1.5).abs() < f64::EPSILON
        ));
    }
}
