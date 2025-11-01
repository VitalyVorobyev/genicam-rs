//! Public API: Camera, Stream, Image, Feature access bridging TL <-> GenApi.

pub mod chunks;
pub mod events;
pub mod stream;
pub mod time;

use std::sync::{Mutex, MutexGuard};

use genapi_core::{GenApiError, Node, NodeMap, RegisterIo};
use thiserror::Error;
use tl_gige::GigeDevice;

pub use chunks::{parse_chunk_bytes, ChunkKind, ChunkMap, ChunkValue};
pub use events::{bind_event_socket, configure_message_channel, Event, EventStream};
pub use stream::{Stream, StreamBuilder};
pub use time::TimeMapper;
pub use tl_gige::action::{AckSummary, ActionParams};

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
}

impl<T: RegisterIo> Camera<T> {
    /// Create a new camera wrapper from a transport and a nodemap.
    pub fn new(transport: T, nodemap: NodeMap) -> Self {
        Self { transport, nodemap }
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
