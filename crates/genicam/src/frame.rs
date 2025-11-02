//! Frame representation combining pixel data with optional chunk metadata.

use std::time::SystemTime;

use bytes::Bytes;

use crate::chunks::{ChunkKind, ChunkMap, ChunkValue};

/// Image frame produced by the GigE Vision stream reassembler.
#[derive(Debug, Clone)]
pub struct Frame {
    /// Contiguous image payload containing pixel data.
    pub payload: Bytes,
    /// Optional map of chunk values decoded from the GVSP trailer.
    pub chunks: Option<ChunkMap>,
    /// Device timestamp reported by the camera when available.
    pub ts_dev: Option<u64>,
    /// Host timestamp obtained by mapping the device ticks.
    pub ts_host: Option<SystemTime>,
}

impl Frame {
    /// Retrieve a chunk value by kind if it exists.
    pub fn chunk(&self, kind: ChunkKind) -> Option<&ChunkValue> {
        self.chunks.as_ref()?.get(&kind)
    }

    /// Host-reconstructed timestamp if the camera reports a device timestamp.
    pub fn host_time(&self) -> Option<SystemTime> {
        self.ts_host
    }
}
