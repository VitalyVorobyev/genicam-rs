//! High-level device timestamp synchronisation helpers.

use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use crate::GenicamError;
use tl_gige::{
    stats::TimeStats,
    time::{self, ControlChannel, TimeError, TimeSync},
};
use tokio::sync::Mutex;
use tokio::time::sleep;

/// Shared wrapper around a timestamp synchroniser and control interface.
pub struct TimeMapper<C: ControlChannel> {
    control: Arc<C>,
    sync: Mutex<TimeSync>,
    stats: TimeStats,
}

impl<C> TimeMapper<C>
where
    C: ControlChannel + 'static,
{
    pub fn new(control: C) -> Self {
        Self {
            control: Arc::new(control),
            sync: Mutex::new(TimeSync::new()),
            stats: TimeStats::new(),
        }
    }

    pub fn stats(&self) -> &TimeStats {
        &self.stats
    }

    pub async fn calibrate(&self, samples: usize, interval_ms: u64) -> Result<(), GenicamError> {
        for _ in 0..samples {
            self.stats.record_latch();
            time::timestamp_latch(self.control.as_ref())
                .await
                .map_err(|err| GenicamError::transport(err.to_string()))?;
            let host = Instant::now();
            let dev = time::read_timestamp_value(self.control.as_ref())
                .await
                .map_err(|err| GenicamError::transport(err.to_string()))?;
            {
                let mut guard = self.sync.lock().await;
                guard.update(dev, host);
            }
            self.stats.record_sample();
            if interval_ms > 0 {
                sleep(Duration::from_millis(interval_ms)).await;
            }
        }
        Ok(())
    }

    pub async fn reset(&self) -> Result<(), GenicamError> {
        self.stats.record_reset();
        time::timestamp_reset(self.control.as_ref())
            .await
            .map_err(|err| GenicamError::transport(err.to_string()))
    }

    pub async fn tick_frequency(&self) -> Result<u64, GenicamError> {
        time::read_tick_frequency(self.control.as_ref())
            .await
            .map_err(|err| GenicamError::transport(err.to_string()))
    }

    pub async fn coefficients(&self) -> (f64, f64) {
        let guard = self.sync.lock().await;
        (guard.a, guard.b)
    }

    pub async fn map_dev_ts(&self, ts: u64) -> SystemTime {
        let guard = self.sync.lock().await;
        guard.to_host_time(ts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockControl;

    #[async_trait::async_trait]
    impl ControlChannel for MockControl {
        async fn read_register(&self, _addr: u64, len: usize) -> Result<Vec<u8>, TimeError> {
            Ok(vec![0u8; len])
        }

        async fn write_register(&self, _addr: u64, _data: &[u8]) -> Result<(), TimeError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn calibrate_collects_samples() {
        let mapper = TimeMapper::new(MockControl);
        mapper.calibrate(4, 0).await.unwrap();
        let snapshot = mapper.stats().snapshot();
        assert_eq!(snapshot.samples, 4);
        assert_eq!(snapshot.latches, 4);
    }
}
