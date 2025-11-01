//! Streaming statistics helpers.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// Basic streaming statistics shared between GVSP and higher layers.
#[derive(Debug)]
pub struct StreamStats {
    packets: AtomicU64,
    resends: AtomicU64,
    dropped_frames: AtomicU64,
    start: Instant,
}

impl StreamStats {
    /// Create a new statistics accumulator.
    pub fn new() -> Self {
        Self {
            packets: AtomicU64::new(0),
            resends: AtomicU64::new(0),
            dropped_frames: AtomicU64::new(0),
            start: Instant::now(),
        }
    }

    /// Record a received packet.
    pub fn record_packet(&self) {
        self.packets.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a resend request.
    pub fn record_resend(&self) {
        self.resends.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a dropped frame event.
    pub fn record_drop(&self) {
        self.dropped_frames.fetch_add(1, Ordering::Relaxed);
    }

    /// Snapshot the current counters.
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            packets: self.packets.load(Ordering::Relaxed),
            resends: self.resends.load(Ordering::Relaxed),
            dropped_frames: self.dropped_frames.load(Ordering::Relaxed),
            elapsed: self.start.elapsed().as_secs_f32(),
        }
    }
}

impl Default for StreamStats {
    fn default() -> Self {
        Self::new()
    }
}

/// Immutable view of collected statistics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Snapshot {
    pub packets: u64,
    pub resends: u64,
    pub dropped_frames: u64,
    pub elapsed: f32,
}
