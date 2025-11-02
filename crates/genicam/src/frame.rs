//! Frame representation combining pixel data with optional chunk metadata.

use bytes::Bytes;

use crate::chunks::{ChunkKind, ChunkMap, ChunkValue};

/// Image frame produced by the GigE Vision stream reassembler.
#[derive(Debug, Clone)]
pub struct Frame {
    /// Contiguous image payload containing pixel data.
    pub payload: Bytes,
    /// Optional map of chunk values decoded from the GVSP trailer.
    pub chunks: Option<ChunkMap>,
}

impl Frame {
    /// Retrieve a chunk value by kind if it exists.
    pub fn chunk(&self, kind: ChunkKind) -> Option<&ChunkValue> {
        self.chunks.as_ref()?.get(&kind)
    }
}
