//! Helpers for synchronising device tick counters with host wall-clock time.

use std::cmp::Ordering;
use std::collections::VecDeque;
use std::time::{Duration, Instant, SystemTime};

use tracing::trace;

/// Maintains a sliding window of timestamp samples and computes a linear model
/// mapping device ticks to host time.
#[derive(Debug)]
pub struct TimeSync {
    /// Linear fit slope (seconds per tick).
    a: f64,
    /// Linear fit intercept (seconds).
    b: f64,
    /// Optional device tick frequency when reported by the camera.
    freq_hz: Option<f64>,
    /// Sample window storing device ticks and host instants.
    window: VecDeque<(u64, Instant)>,
    /// Maximum number of samples retained in the window.
    cap: usize,
    /// Host instant corresponding to the first recorded sample.
    origin_instant: Option<Instant>,
    /// Host system time captured alongside the origin instant.
    origin_system: Option<SystemTime>,
}

impl TimeSync {
    /// Create a new synchroniser retaining up to `cap` samples.
    pub fn new(cap: usize) -> Self {
        Self {
            a: 0.0,
            b: 0.0,
            freq_hz: None,
            window: VecDeque::with_capacity(cap),
            cap,
            origin_instant: None,
            origin_system: None,
        }
    }

    /// Record a new `(device_ticks, host_instant)` sample.
    pub fn update(&mut self, dev_ticks: u64, host: Instant) {
        if self.origin_instant.is_none() {
            self.origin_instant = Some(host);
            self.origin_system = Some(SystemTime::now());
        }
        if self.window.len() == self.cap {
            self.window.pop_front();
        }
        self.window.push_back((dev_ticks, host));
    }

    /// Number of samples retained in the sliding window.
    pub fn len(&self) -> usize {
        self.window.len()
    }

    /// Iterator over the samples contained in the sliding window.
    pub fn samples(&self) -> impl Iterator<Item = (u64, Instant)> + '_ {
        self.window.iter().copied()
    }

    /// Maximum number of samples stored in the sliding window.
    pub fn capacity(&self) -> usize {
        self.cap
    }

    /// Access the origin instant if at least one sample has been recorded.
    pub fn origin_instant(&self) -> Option<Instant> {
        self.origin_instant
    }

    /// Access the origin system time if available.
    pub fn origin_system(&self) -> Option<SystemTime> {
        self.origin_system
    }

    /// Retrieve the linear fit coefficients.
    pub fn coefficients(&self) -> (f64, f64) {
        (self.a, self.b)
    }

    /// Retrieve the reported device tick frequency.
    pub fn freq_hz(&self) -> Option<f64> {
        self.freq_hz
    }

    /// Return the first and last sample retained in the window.
    pub fn sample_bounds(&self) -> Option<((u64, Instant), (u64, Instant))> {
        let first = *self.window.front()?;
        let last = *self.window.back()?;
        Some((first, last))
    }

    /// Fit a linear model mapping device ticks to host seconds relative to the
    /// first recorded instant. Returns the updated `(a, b)` coefficients when
    /// enough samples are available.
    pub fn fit(&mut self, freq_hz: Option<f64>) -> Option<(f64, f64)> {
        if self.window.len() < 2 {
            return None;
        }
        if let Some(freq) = freq_hz {
            self.freq_hz = Some(freq);
        }
        let origin = self.origin_instant?;
        let base_tick = self.window.front()?.0 as f64;
        let samples: Vec<(f64, f64)> = self
            .window
            .iter()
            .map(|(ticks, host)| {
                let x = (*ticks as f64) - base_tick;
                let y = host.duration_since(origin).as_secs_f64();
                (x, y)
            })
            .collect();

        let (mut slope, mut intercept_rel) = compute_fit(&samples)?;
        if samples.len() >= 10 {
            let mut residuals: Vec<(usize, f64)> = samples
                .iter()
                .enumerate()
                .map(|(idx, (x, y))| {
                    let predicted = slope * *x + intercept_rel;
                    (idx, y - predicted)
                })
                .collect();
            residuals.sort_by(|a, b| match a.1.partial_cmp(&b.1) {
                Some(order) => order,
                None => Ordering::Equal,
            });
            let trim = ((residuals.len() as f64) * 0.1).floor() as usize;
            if trim > 0 && residuals.len() > trim * 2 {
                let trimmed_samples: Vec<(f64, f64)> = residuals[trim..residuals.len() - trim]
                    .iter()
                    .map(|(idx, _)| samples[*idx])
                    .collect();
                if let Some((s, i)) = compute_fit(&trimmed_samples) {
                    slope = s;
                    intercept_rel = i;
                }
            }
        }

        let intercept = intercept_rel - slope * base_tick;
        self.a = slope;
        self.b = intercept;

        for (ticks, host) in &self.window {
            let predicted = self.a * (*ticks as f64) + self.b;
            let actual = host.duration_since(origin).as_secs_f64();
            trace!(
                ticks = *ticks,
                predicted_s = predicted,
                actual_s = actual,
                residual_s = actual - predicted,
                "timestamp fit residual"
            );
        }

        Some((self.a, self.b))
    }

    /// Convert device ticks into a [`SystemTime`] using the fitted model.
    pub fn to_host_time(&self, dev_ticks: u64) -> SystemTime {
        let Some(origin) = self.origin_system else {
            return SystemTime::now();
        };
        let secs = self.a * (dev_ticks as f64) + self.b;
        if !secs.is_finite() || secs <= 0.0 {
            return origin;
        }
        match Duration::try_from_secs_f64(secs) {
            Ok(duration) => origin + duration,
            Err(_) => origin,
        }
    }
}

fn compute_fit(samples: &[(f64, f64)]) -> Option<(f64, f64)> {
    if samples.len() < 2 {
        return None;
    }
    let mut sum_x = 0.0;
    let mut sum_y = 0.0;
    for (x, y) in samples {
        sum_x += x;
        sum_y += y;
    }
    let n = samples.len() as f64;
    let mean_x = sum_x / n;
    let mean_y = sum_y / n;
    let mut denom = 0.0;
    let mut numer = 0.0;
    for (x, y) in samples {
        let dx = x - mean_x;
        let dy = y - mean_y;
        denom += dx * dx;
        numer += dx * dy;
    }
    if denom.abs() < f64::EPSILON {
        return None;
    }
    let slope = numer / denom;
    let intercept = mean_y - slope * mean_x;
    Some((slope, intercept))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synthetic_fit_handles_jitter() {
        let mut sync = TimeSync::new(64);
        let freq_hz = 150_000_000.0;
        let start = Instant::now();
        for i in 0..64u64 {
            let ticks = i * 150_000;
            let ideal = start + Duration::from_secs_f64((i as f64) * 0.001);
            let jitter = (fastrand::f64() - 0.5) * 400e-6;
            let jitter_duration = Duration::from_secs_f64(jitter.abs());
            let host = if jitter >= 0.0 {
                ideal + jitter_duration
            } else {
                ideal.checked_sub(jitter_duration).unwrap_or(ideal)
            };
            sync.update(ticks, host);
        }
        sync.fit(Some(freq_hz));
        let (a, b) = sync.coefficients();
        let origin = sync.origin_instant().unwrap();
        let max_error = sync
            .samples()
            .map(|(ticks, host)| {
                let predicted = a * (ticks as f64) + b;
                let actual = host.duration_since(origin).as_secs_f64();
                (predicted - actual).abs()
            })
            .fold(0.0, f64::max);
        assert!(max_error < 5e-4, "max error {max_error} exceeds tolerance");
    }

    #[test]
    fn compute_fit_returns_none_for_single_sample() {
        let mut sync = TimeSync::new(4);
        sync.update(100, Instant::now());
        assert!(sync.fit(None).is_none());
    }
}
