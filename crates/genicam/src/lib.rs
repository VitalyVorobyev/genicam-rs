#![cfg_attr(docsrs, feature(doc_cfg))]
//! High level GenICam facade that re-exports the workspace crates and provides
//! convenience wrappers.
//!
//! ```rust,no_run
//! use genicam::{gige, genapi, Camera, GenicamError};
//! use std::time::Duration;
//!
//! # struct DummyTransport;
//! # impl genapi::RegisterIo for DummyTransport {
//! #     fn read(&self, _addr: u64, len: usize) -> Result<Vec<u8>, genapi::GenApiError> {
//! #         Ok(vec![0; len])
//! #     }
//! #     fn write(&self, _addr: u64, _data: &[u8]) -> Result<(), genapi::GenApiError> {
//! #         Ok(())
//! #     }
//! # }
//! # #[allow(dead_code)]
//! # fn load_nodemap() -> genapi::NodeMap {
//! #     unimplemented!("replace with GenApi XML parsing")
//! # }
//! # #[allow(dead_code)]
//! # async fn open_transport() -> Result<DummyTransport, GenicamError> {
//! #     Ok(DummyTransport)
//! # }
//! # #[allow(dead_code)]
//! # async fn run() -> Result<(), GenicamError> {
//! let timeout = Duration::from_millis(500);
//! let devices = gige::discover(timeout)
//!     .await
//!     .expect("discover cameras");
//! println!("found {} cameras", devices.len());
//! let mut camera = Camera::new(open_transport().await?, load_nodemap());
//! camera.set("ExposureTime", "5000")?;
//! # Ok(())
//! # }
//! ```

pub use genapi_core as genapi;
pub use genicp;
pub use pfnc;
pub use sfnc;
pub use tl_gige as gige;

pub mod chunks;
pub mod events;
pub mod frame;
pub mod stream;
pub mod time;

use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime};

use crate::genapi::{GenApiError, Node, NodeMap, RegisterIo};
use gige::GigeDevice;
use thiserror::Error;
use tokio::time::sleep;
use tracing::{debug, info};

pub use chunks::{parse_chunk_bytes, ChunkKind, ChunkMap, ChunkValue};
pub use events::{bind_event_socket, configure_message_channel, Event, EventStream};
pub use frame::Frame;
pub use gige::action::{AckSummary, ActionParams};
pub use stream::{Stream, StreamBuilder};
pub use time::TimeSync;

use crate::time::TimeSync as TimeSyncModel;

/// Error type produced by the high level GenICam facade.
#[derive(Debug, Error)]
pub enum GenicamError {
    /// Wrapper around GenApi errors produced by the nodemap.
    #[error(transparent)]
    GenApi(#[from] GenApiError),
    /// Transport level failure while accessing registers.
    #[error("transport: {0}")]
    Transport(String),
    /// Parsing a user supplied value failed.
    #[error("parse error: {0}")]
    Parse(String),
    /// Required chunk feature missing from the nodemap.
    #[error("chunk feature '{0}' not found; verify camera supports chunk data")]
    MissingChunkFeature(String),
}

impl GenicamError {
    fn parse<S: Into<String>>(msg: S) -> Self {
        GenicamError::Parse(msg.into())
    }

    fn transport<S: Into<String>>(msg: S) -> Self {
        GenicamError::Transport(msg.into())
    }
}

/// Camera facade combining a nodemap with a transport implementing [`RegisterIo`].
#[derive(Debug)]
pub struct Camera<T: RegisterIo> {
    transport: T,
    nodemap: NodeMap,
    time_sync: TimeSyncModel,
}

impl<T: RegisterIo> Camera<T> {
    /// Create a new camera wrapper from a transport and a nodemap.
    pub fn new(transport: T, nodemap: NodeMap) -> Self {
        Self {
            transport,
            nodemap,
            time_sync: TimeSyncModel::new(64),
        }
    }

    /// Return a reference to the underlying transport.
    pub fn transport(&self) -> &T {
        &self.transport
    }

    /// Return a mutable reference to the underlying transport.
    pub fn transport_mut(&mut self) -> &mut T {
        &mut self.transport
    }

    /// Access the nodemap metadata.
    pub fn nodemap(&self) -> &NodeMap {
        &self.nodemap
    }

    /// Mutable access to the nodemap.
    pub fn nodemap_mut(&mut self) -> &mut NodeMap {
        &mut self.nodemap
    }

    /// List available entries for an enumeration feature.
    pub fn enum_entries(&self, name: &str) -> Result<Vec<String>, GenicamError> {
        self.nodemap.enum_entries(name).map_err(Into::into)
    }

    /// Retrieve a feature value as a string using the nodemap type to format it.
    pub fn get(&self, name: &str) -> Result<String, GenicamError> {
        match self.nodemap.node(name) {
            Some(Node::Integer(_)) => {
                Ok(self.nodemap.get_integer(name, &self.transport)?.to_string())
            }
            Some(Node::Float(_)) => Ok(self.nodemap.get_float(name, &self.transport)?.to_string()),
            Some(Node::Enum(_)) => self
                .nodemap
                .get_enum(name, &self.transport)
                .map_err(Into::into),
            Some(Node::Boolean(_)) => Ok(self.nodemap.get_bool(name, &self.transport)?.to_string()),
            Some(Node::Command(_)) => {
                Err(GenicamError::GenApi(GenApiError::Type(name.to_string())))
            }
            Some(Node::Category(_)) => Ok(String::new()),
            None => Err(GenApiError::NodeNotFound(name.to_string()).into()),
        }
    }

    /// Set a feature value using a string representation.
    pub fn set(&mut self, name: &str, value: &str) -> Result<(), GenicamError> {
        match self.nodemap.node(name) {
            Some(Node::Integer(_)) => {
                let parsed: i64 = value
                    .parse()
                    .map_err(|_| GenicamError::parse(format!("invalid integer for {name}")))?;
                self.nodemap
                    .set_integer(name, parsed, &self.transport)
                    .map_err(Into::into)
            }
            Some(Node::Float(_)) => {
                let parsed: f64 = value
                    .parse()
                    .map_err(|_| GenicamError::parse(format!("invalid float for {name}")))?;
                self.nodemap
                    .set_float(name, parsed, &self.transport)
                    .map_err(Into::into)
            }
            Some(Node::Enum(_)) => self
                .nodemap
                .set_enum(name, value, &self.transport)
                .map_err(Into::into),
            Some(Node::Boolean(_)) => {
                let parsed = parse_bool(value).ok_or_else(|| {
                    GenicamError::parse(format!("invalid boolean for {name}: {value}"))
                })?;
                self.nodemap
                    .set_bool(name, parsed, &self.transport)
                    .map_err(Into::into)
            }
            Some(Node::Command(_)) => self
                .nodemap
                .exec_command(name, &self.transport)
                .map_err(Into::into),
            Some(Node::Category(_)) => Err(GenApiError::Type(name.to_string()).into()),
            None => Err(GenApiError::NodeNotFound(name.to_string()).into()),
        }
    }

    /// Convenience wrapper for exposure time features expressed in microseconds.
    pub fn set_exposure_time_us(&mut self, value: f64) -> Result<(), GenicamError> {
        // Use SFNC name directly to avoid cross-crate constant lookup issues in docs
        self.set_float_feature("ExposureTime", value)
    }

    /// Convenience wrapper for gain features expressed in decibel.
    pub fn set_gain_db(&mut self, value: f64) -> Result<(), GenicamError> {
        self.set_float_feature("Gain", value)
    }

    fn set_float_feature(&mut self, name: &str, value: f64) -> Result<(), GenicamError> {
        match self.nodemap.node(name) {
            Some(Node::Float(_)) => self
                .nodemap
                .set_float(name, value, &self.transport)
                .map_err(Into::into),
            Some(_) => Err(GenApiError::Type(name.to_string()).into()),
            None => Err(GenApiError::NodeNotFound(name.to_string()).into()),
        }
    }

    /// Capture device/host timestamp pairs and fit a mapping model.
    pub async fn time_calibrate(
        &mut self,
        samples: usize,
        interval_ms: u64,
    ) -> Result<(), GenicamError> {
        if samples < 2 {
            return Err(GenicamError::transport(
                "time calibration requires at least two samples",
            ));
        }

        let cap = samples.max(self.time_sync.capacity());
        self.time_sync = TimeSyncModel::new(cap);

        let latch_cmd = self.find_alias(sfnc::TS_LATCH_CMDS);
        let value_node = self
            .find_alias(sfnc::TS_VALUE_NODES)
            .ok_or_else(|| GenApiError::NodeNotFound("TimestampValue".into()))?;

        let mut freq_hz = if let Some(name) = self.find_alias(sfnc::TS_FREQ_NODES) {
            match self.nodemap.get_integer(name, &self.transport) {
                Ok(value) if value > 0 => Some(value as f64),
                Ok(_) => None,
                Err(err) => {
                    debug!(node = name, error = %err, "failed to read timestamp frequency");
                    None
                }
            }
        } else {
            None
        };

        info!(samples, interval_ms, "starting time calibration");
        let mut first_sample: Option<(u64, Instant)> = None;
        let mut last_sample: Option<(u64, Instant)> = None;

        for idx in 0..samples {
            if let Some(cmd) = latch_cmd {
                self.nodemap
                    .exec_command(cmd, &self.transport)
                    .map_err(GenicamError::from)?;
            }

            let raw_ticks = self
                .nodemap
                .get_integer(value_node, &self.transport)
                .map_err(GenicamError::from)?;
            let dev_ticks = u64::try_from(raw_ticks).map_err(|_| {
                GenicamError::transport("timestamp value is negative; unsupported camera")
            })?;
            let host = Instant::now();
            self.time_sync.update(dev_ticks, host);
            if idx == 0 {
                first_sample = Some((dev_ticks, host));
            }
            last_sample = Some((dev_ticks, host));
            if let Some(origin) = self.time_sync.origin_instant() {
                let ns = host.duration_since(origin).as_nanos();
                debug!(
                    sample = idx,
                    ticks = dev_ticks,
                    host_ns = ns,
                    "timestamp sample"
                );
            } else {
                debug!(sample = idx, ticks = dev_ticks, "timestamp sample");
            }

            if interval_ms > 0 && idx + 1 < samples {
                sleep(Duration::from_millis(interval_ms)).await;
            }
        }

        if freq_hz.is_none() {
            if let (Some((first_ticks, first_host)), Some((last_ticks, last_host))) =
                (first_sample, last_sample)
            {
                if last_ticks > first_ticks {
                    if let Some(delta) = last_host.checked_duration_since(first_host) {
                        let secs = delta.as_secs_f64();
                        if secs > 0.0 {
                            freq_hz = Some((last_ticks - first_ticks) as f64 / secs);
                        }
                    }
                }
            }
        }

        let (a, b) = self
            .time_sync
            .fit(freq_hz)
            .ok_or_else(|| GenicamError::transport("insufficient samples for timestamp fit"))?;

        if let Some(freq) = freq_hz {
            info!(freq_hz = freq, a, b, "time calibration complete");
        } else {
            info!(a, b, "time calibration complete");
        }

        Ok(())
    }

    /// Map device tick counters to host time using the fitted model.
    pub fn map_dev_ts(&self, dev_ticks: u64) -> SystemTime {
        self.time_sync.to_host_time(dev_ticks)
    }

    /// Inspect the timestamp synchroniser state.
    pub fn time_sync(&self) -> &TimeSync {
        &self.time_sync
    }

    /// Reset the device timestamp counter when supported by the camera.
    pub fn time_reset(&mut self) -> Result<(), GenicamError> {
        if let Some(cmd) = self.find_alias(sfnc::TS_RESET_CMDS) {
            self.nodemap
                .exec_command(cmd, &self.transport)
                .map_err(GenicamError::from)?;
            self.time_sync = TimeSyncModel::new(self.time_sync.capacity());
            info!(command = cmd, "timestamp counter reset");
        }
        Ok(())
    }

    /// Trigger acquisition start via the SFNC command feature.
    pub fn acquisition_start(&mut self) -> Result<(), GenicamError> {
        self.nodemap
            .exec_command("AcquisitionStart", &self.transport)
            .map_err(Into::into)
    }

    /// Trigger acquisition stop via the SFNC command feature.
    pub fn acquisition_stop(&mut self) -> Result<(), GenicamError> {
        self.nodemap
            .exec_command("AcquisitionStop", &self.transport)
            .map_err(Into::into)
    }

    /// Configure chunk mode and enable the requested selectors.
    pub fn configure_chunks(&mut self, cfg: &ChunkConfig) -> Result<(), GenicamError> {
        self.ensure_chunk_feature(sfnc::CHUNK_MODE_ACTIVE)?;
        self.ensure_chunk_feature(sfnc::CHUNK_SELECTOR)?;
        self.ensure_chunk_feature(sfnc::CHUNK_ENABLE)?;

        let transport = &self.transport as *const T;
        unsafe {
            // SAFETY: `transport` points to `self.transport`, which is only accessed immutably
            // while the nodemap borrow is active.
            self.nodemap_mut()
                .set_bool(sfnc::CHUNK_MODE_ACTIVE, cfg.active, &*transport)
                .map_err(GenicamError::from)?;
        }

        for selector in &cfg.selectors {
            let transport = &self.transport as *const T;
            unsafe {
                // SAFETY: see rationale above; the nodemap mutation does not alias with
                // the immutable transport access.
                self.nodemap_mut()
                    .set_enum(sfnc::CHUNK_SELECTOR, selector, &*transport)
                    .map_err(GenicamError::from)?;
                self.nodemap_mut()
                    .set_bool(sfnc::CHUNK_ENABLE, true, &*transport)
                    .map_err(GenicamError::from)?;
            }
        }

        Ok(())
    }

    fn ensure_chunk_feature(&self, name: &str) -> Result<(), GenicamError> {
        if self.nodemap.node(name).is_none() {
            return Err(GenicamError::MissingChunkFeature(name.to_string()));
        }
        Ok(())
    }

    fn find_alias<'a>(&'a self, names: &[&'static str]) -> Option<&'static str> {
        names
            .iter()
            .copied()
            .find(|name| self.nodemap.node(name).is_some())
    }
}

/// Configuration for enabling chunk data via SFNC features.
#[derive(Debug, Clone, Default)]
pub struct ChunkConfig {
    /// Names of chunk selectors that should be enabled on the device.
    pub selectors: Vec<String>,
    /// Whether chunk mode should be active after configuration.
    pub active: bool,
}

/// Blocking adapter turning an asynchronous [`GigeDevice`] into a [`RegisterIo`]
/// implementation.
///
/// The adapter uses a [`tokio::runtime::Handle`] to synchronously wait on GVCP
/// register transactions. All callers must ensure these methods are invoked
/// from outside of the runtime context to avoid nested `block_on` panics.
pub struct GigeRegisterIo {
    handle: tokio::runtime::Handle,
    device: Mutex<GigeDevice>,
}

impl GigeRegisterIo {
    /// Create a new adapter using the provided runtime handle and device.
    pub fn new(handle: tokio::runtime::Handle, device: GigeDevice) -> Self {
        Self {
            handle,
            device: Mutex::new(device),
        }
    }

    fn lock(&self) -> Result<MutexGuard<'_, GigeDevice>, GenApiError> {
        self.device
            .lock()
            .map_err(|_| GenApiError::Io("gige device mutex poisoned".into()))
    }
}

impl RegisterIo for GigeRegisterIo {
    fn read(&self, addr: u64, len: usize) -> Result<Vec<u8>, GenApiError> {
        let mut device = self.lock()?;
        self.handle
            .block_on(device.read_mem(addr, len))
            .map_err(|err| GenApiError::Io(err.to_string()))
    }

    fn write(&self, addr: u64, data: &[u8]) -> Result<(), GenApiError> {
        let mut device = self.lock()?;
        self.handle
            .block_on(device.write_mem(addr, data))
            .map_err(|err| GenApiError::Io(err.to_string()))
    }
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" => Some(true),
        "0" | "false" => Some(false),
        _ => None,
    }
}
