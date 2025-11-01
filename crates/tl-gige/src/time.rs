//! Device timestamp helpers and host mapping utilities.

use std::collections::VecDeque;
use std::convert::TryInto;
use std::time::{Duration, Instant, SystemTime};

use async_trait::async_trait;
use thiserror::Error;
use tracing::{debug, trace};

use crate::gvcp::GigeError;

/// Address of the SFNC `TimestampControl` register.
pub const REG_TIMESTAMP_CONTROL: u64 = 0x0900_0100;
/// Address of the SFNC `TimestampValue` register (64-bit).
pub const REG_TIMESTAMP_VALUE: u64 = 0x0900_0104;
/// Address of the SFNC `TimestampTickFrequency` register (64-bit).
pub const REG_TIMESTAMP_TICK_FREQUENCY: u64 = 0x0900_010C;
/// Bit flag to latch the timestamp counter.
pub const TIMESTAMP_LATCH_BIT: u32 = 0x0000_0002;
/// Bit flag to reset the timestamp counter.
pub const TIMESTAMP_RESET_BIT: u32 = 0x0000_0001;
/// Maximum number of samples kept for linear regression.
const MAX_TIME_WINDOW: usize = 32;

/// Errors encountered while interacting with timestamp control registers.
#[derive(Debug, Error)]
pub enum TimeError {
    #[error("control: {0}")]
    Control(#[from] GigeError),
    #[error("protocol: {0}")]
    Protocol(&'static str),
}

/// Minimal interface required to read/write timestamp registers.
#[async_trait]
pub trait ControlChannel: Send + Sync {
    async fn read_register(&self, addr: u64, len: usize) -> Result<Vec<u8>, TimeError>;
    async fn write_register(&self, addr: u64, data: &[u8]) -> Result<(), TimeError>;
}

fn write_u32_be(value: u32) -> [u8; 4] {
    value.to_be_bytes()
}

fn parse_u64_be(data: &[u8]) -> Result<u64, TimeError> {
    if data.len() != 8 {
        return Err(TimeError::Protocol("unexpected register size"));
    }
    Ok(u64::from_be_bytes(
        data.try_into().expect("slice length checked"),
    ))
}

/// Issue a timestamp reset using the SFNC control register.
pub async fn timestamp_reset<C: ControlChannel>(ctrl: &C) -> Result<(), TimeError> {
    trace!("triggering timestamp reset");
    ctrl.write_register(REG_TIMESTAMP_CONTROL, &write_u32_be(TIMESTAMP_RESET_BIT))
        .await
}

/// Latch the current timestamp counter to make it readable without jitter.
pub async fn timestamp_latch<C: ControlChannel>(ctrl: &C) -> Result<(), TimeError> {
    trace!("triggering timestamp latch");
    ctrl.write_register(REG_TIMESTAMP_CONTROL, &write_u32_be(TIMESTAMP_LATCH_BIT))
        .await
}

/// Read the current 64-bit timestamp value from the device.
pub async fn read_timestamp_value<C: ControlChannel>(ctrl: &C) -> Result<u64, TimeError> {
    let bytes = ctrl.read_register(REG_TIMESTAMP_VALUE, 8).await?;
    parse_u64_be(&bytes)
}

/// Read the device tick frequency.
pub async fn read_tick_frequency<C: ControlChannel>(ctrl: &C) -> Result<u64, TimeError> {
    let bytes = ctrl.read_register(REG_TIMESTAMP_TICK_FREQUENCY, 8).await?;
    parse_u64_be(&bytes)
}

/// Maintain a linear mapping between device ticks and host time.
#[derive(Debug, Clone)]
pub struct TimeSync {
    pub(crate) a: f64,
    pub(crate) b: f64,
    window: VecDeque<(u64, Instant)>,
    anchor_host: Instant,
    anchor_system: SystemTime,
}

impl TimeSync {
    /// Create an empty synchroniser.
    pub fn new() -> Self {
        let anchor_host = Instant::now();
        let anchor_system = SystemTime::now();
        Self {
            a: 1.0,
            b: 0.0,
            window: VecDeque::new(),
            anchor_host,
            anchor_system,
        }
    }

    /// Return the current slope and intercept of the time mapping.
    pub fn coefficients(&self) -> (f64, f64) {
        (self.a, self.b)
    }

    fn recompute(&mut self) {
        if self.window.len() < 2 {
            return;
        }
        let n = self.window.len() as f64;
        let mut sum_x = 0f64;
        let mut sum_y = 0f64;
        let mut sum_xx = 0f64;
        let mut sum_xy = 0f64;
        for (dev, host) in &self.window {
            let x = *dev as f64;
            let y = if *host >= self.anchor_host {
                host.duration_since(self.anchor_host).as_secs_f64()
            } else {
                0.0
            };
            sum_x += x;
            sum_y += y;
            sum_xx += x * x;
            sum_xy += x * y;
        }
        let denom = n * sum_xx - sum_x * sum_x;
        if denom.abs() < f64::EPSILON {
            return;
        }
        let slope = (n * sum_xy - sum_x * sum_y) / denom;
        let intercept = (sum_y - slope * sum_x) / n;
        self.a = slope;
        self.b = intercept;
        debug!(
            slope = self.a,
            intercept = self.b,
            "recomputed time mapping"
        );
    }

    /// Add a new measurement pair to the regression window.
    pub fn update(&mut self, dev_ts: u64, host_instant: Instant) {
        if self.window.is_empty() {
            self.anchor_host = host_instant;
            self.anchor_system = SystemTime::now();
        }
        if self.window.len() == MAX_TIME_WINDOW {
            self.window.pop_front();
        }
        self.window.push_back((dev_ts, host_instant));
        self.recompute();
    }

    /// Convert a device timestamp into a host `SystemTime`.
    pub fn to_host_time(&self, dev_ts: u64) -> SystemTime {
        let seconds = self.a * dev_ts as f64 + self.b;
        if seconds.is_finite() && seconds >= 0.0 {
            let duration = Duration::from_secs_f64(seconds);
            self.anchor_system + duration
        } else {
            self.anchor_system
        }
    }
}

impl Default for TimeSync {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ControlChannel for tokio::sync::Mutex<crate::gvcp::GigeDevice> {
    async fn read_register(&self, addr: u64, len: usize) -> Result<Vec<u8>, TimeError> {
        let mut guard = self.lock().await;
        guard.read_mem(addr, len).await.map_err(TimeError::from)
    }

    async fn write_register(&self, addr: u64, data: &[u8]) -> Result<(), TimeError> {
        let mut guard = self.lock().await;
        guard.write_mem(addr, data).await.map_err(TimeError::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regression_tracks_linear_relation() {
        let mut sync = TimeSync::new();
        let start = Instant::now();
        for i in 0..16u64 {
            let dev = i * 1000;
            let host = start + Duration::from_millis((i * 16) as u64);
            sync.update(dev, host);
        }
        let mapped = sync.to_host_time(64_000);
        let mapped_secs = mapped
            .duration_since(sync.anchor_system)
            .unwrap()
            .as_secs_f64();
        let expected_secs = Duration::from_millis(1024).as_secs_f64();
        assert!((mapped_secs - expected_secs).abs() < 0.1);
    }
}
