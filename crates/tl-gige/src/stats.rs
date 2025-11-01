//! Streaming statistics helpers.

use std::f64;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// Basic streaming statistics shared between GVSP and higher layers.
#[derive(Debug)]
pub struct StreamStats {
    packets: AtomicU64,
    resends: AtomicU64,
    resend_ranges: AtomicU64,
    dropped_frames: AtomicU64,
    backpressure_drops: AtomicU64,
    late_frames: AtomicU64,
    pool_exhaustions: AtomicU64,
    start: Instant,
}

impl StreamStats {
    /// Create a new statistics accumulator.
    pub fn new() -> Self {
        Self {
            packets: AtomicU64::new(0),
            resends: AtomicU64::new(0),
            resend_ranges: AtomicU64::new(0),
            dropped_frames: AtomicU64::new(0),
            backpressure_drops: AtomicU64::new(0),
            late_frames: AtomicU64::new(0),
            pool_exhaustions: AtomicU64::new(0),
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

    /// Record the number of packet ranges covered by a resend request.
    pub fn record_resend_ranges(&self, ranges: u64) {
        if ranges > 0 {
            self.resend_ranges.fetch_add(ranges, Ordering::Relaxed);
        }
    }

    /// Record a dropped frame event.
    pub fn record_drop(&self) {
        self.dropped_frames.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a drop caused by application backpressure.
    pub fn record_backpressure_drop(&self) {
        self.backpressure_drops.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a frame that missed its presentation deadline.
    pub fn record_late_frame(&self) {
        self.late_frames.fetch_add(1, Ordering::Relaxed);
    }

    /// Record an exhausted frame buffer pool event.
    pub fn record_pool_exhaustion(&self) {
        self.pool_exhaustions.fetch_add(1, Ordering::Relaxed);
    }

    /// Snapshot the current counters.
    pub fn snapshot(&self) -> Snapshot {
        let elapsed = self.start.elapsed().as_secs_f64().max(f64::EPSILON) as f32;
        Snapshot {
            packets: self.packets.load(Ordering::Relaxed),
            resends: self.resends.load(Ordering::Relaxed),
            resend_ranges: self.resend_ranges.load(Ordering::Relaxed),
            dropped_frames: self.dropped_frames.load(Ordering::Relaxed),
            backpressure_drops: self.backpressure_drops.load(Ordering::Relaxed),
            late_frames: self.late_frames.load(Ordering::Relaxed),
            pool_exhaustions: self.pool_exhaustions.load(Ordering::Relaxed),
            elapsed,
            packets_per_second: self.packets.load(Ordering::Relaxed) as f32 / elapsed,
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
    pub resend_ranges: u64,
    pub dropped_frames: u64,
    pub backpressure_drops: u64,
    pub late_frames: u64,
    pub pool_exhaustions: u64,
    pub elapsed: f32,
    pub packets_per_second: f32,
}

/// Event channel statistics.
#[derive(Debug)]
pub struct EventStats {
    received: AtomicU64,
    malformed: AtomicU64,
    filtered: AtomicU64,
    start: Instant,
}

impl EventStats {
    /// Create a new accumulator for GVCP events.
    pub fn new() -> Self {
        Self {
            received: AtomicU64::new(0),
            malformed: AtomicU64::new(0),
            filtered: AtomicU64::new(0),
            start: Instant::now(),
        }
    }

    /// Record a successfully parsed event packet.
    pub fn record_event(&self) {
        self.received.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a dropped or malformed event packet.
    pub fn record_malformed(&self) {
        self.malformed.fetch_add(1, Ordering::Relaxed);
    }

    /// Record an event filtered out by the application.
    pub fn record_filtered(&self) {
        self.filtered.fetch_add(1, Ordering::Relaxed);
    }

    /// Snapshot the collected counters.
    pub fn snapshot(&self) -> EventSnapshot {
        EventSnapshot {
            received: self.received.load(Ordering::Relaxed),
            malformed: self.malformed.load(Ordering::Relaxed),
            filtered: self.filtered.load(Ordering::Relaxed),
            elapsed: self.start.elapsed().as_secs_f32(),
        }
    }
}

impl Default for EventStats {
    fn default() -> Self {
        Self::new()
    }
}

/// Immutable view of event statistics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EventSnapshot {
    pub received: u64,
    pub malformed: u64,
    pub filtered: u64,
    pub elapsed: f32,
}

/// Action command dispatch statistics.
#[derive(Debug)]
pub struct ActionStats {
    sent: AtomicU64,
    acknowledgements: AtomicU64,
    failures: AtomicU64,
}

impl ActionStats {
    /// Create a new accumulator for action command metrics.
    pub fn new() -> Self {
        Self {
            sent: AtomicU64::new(0),
            acknowledgements: AtomicU64::new(0),
            failures: AtomicU64::new(0),
        }
    }

    /// Record a dispatched action.
    pub fn record_send(&self) {
        self.sent.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a received acknowledgement.
    pub fn record_ack(&self) {
        self.acknowledgements.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a failure while dispatching or waiting for acknowledgements.
    pub fn record_failure(&self) {
        self.failures.fetch_add(1, Ordering::Relaxed);
    }

    /// Snapshot the collected counters.
    pub fn snapshot(&self) -> ActionSnapshot {
        ActionSnapshot {
            sent: self.sent.load(Ordering::Relaxed),
            acknowledgements: self.acknowledgements.load(Ordering::Relaxed),
            failures: self.failures.load(Ordering::Relaxed),
        }
    }
}

impl Default for ActionStats {
    fn default() -> Self {
        Self::new()
    }
}

/// Immutable view of action statistics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionSnapshot {
    pub sent: u64,
    pub acknowledgements: u64,
    pub failures: u64,
}

/// Timestamp synchronisation statistics.
#[derive(Debug)]
pub struct TimeStats {
    samples: AtomicU64,
    latches: AtomicU64,
    resets: AtomicU64,
}

impl TimeStats {
    /// Create a new accumulator for timestamp operations.
    pub fn new() -> Self {
        Self {
            samples: AtomicU64::new(0),
            latches: AtomicU64::new(0),
            resets: AtomicU64::new(0),
        }
    }

    /// Record a calibration sample.
    pub fn record_sample(&self) {
        self.samples.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a timestamp latch request.
    pub fn record_latch(&self) {
        self.latches.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a timestamp reset operation.
    pub fn record_reset(&self) {
        self.resets.fetch_add(1, Ordering::Relaxed);
    }

    /// Snapshot the current counters.
    pub fn snapshot(&self) -> TimeSnapshot {
        TimeSnapshot {
            samples: self.samples.load(Ordering::Relaxed),
            latches: self.latches.load(Ordering::Relaxed),
            resets: self.resets.load(Ordering::Relaxed),
        }
    }
}

impl Default for TimeStats {
    fn default() -> Self {
        Self::new()
    }
}

/// Immutable view of timestamp statistics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeSnapshot {
    pub samples: u64,
    pub latches: u64,
    pub resets: u64,
}
