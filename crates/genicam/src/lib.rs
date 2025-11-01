//! Public API: Camera, Stream, Image, Feature access bridging TL <-> GenApi.

pub mod chunks;
pub mod events;
pub mod time;

use async_trait::async_trait;
use genapi_core::{GenApiError, Node, NodeMap};
use thiserror::Error;

pub use tl_gige::action::{AckSummary, ActionParams};
pub use chunks::{parse_chunk_bytes, ChunkKind, ChunkMap, ChunkValue};
pub use events::{bind_event_socket, configure_message_channel, Event, EventStream};
pub use time::TimeMapper;

#[derive(Debug, Error)]
pub enum GenicamError {
    #[error(transparent)]
    GenApi(#[from] GenApiError),
    #[error("transport: {0}")]
    Transport(String),
}

impl GenicamError {
    pub fn transport<S: Into<String>>(msg: S) -> Self {
        GenicamError::Transport(msg.into())
    }
}

pub trait Transport {
    fn read_mem(&self, addr: u64, len: usize) -> Result<Vec<u8>, GenicamError>;
    fn write_mem(&self, addr: u64, data: &[u8]) -> Result<(), GenicamError>;
    // start_stream / stop_stream / events etc.
}

#[async_trait]
pub trait ActionTransport {
    async fn fire_action(&self, params: &ActionParams) -> Result<AckSummary, GenicamError>;
}

pub struct Camera<T: Transport> {
    transport: T,
    nodemap: NodeMap,
}

impl<T: Transport> Camera<T> {
    pub fn transport(&self) -> &T {
        &self.transport
    }
    pub fn transport_mut(&mut self) -> &mut T {
        &mut self.transport
    }
    pub fn nodemap(&self) -> &NodeMap {
        &self.nodemap
    }
    pub fn nodemap_mut(&mut self) -> &mut NodeMap {
        &mut self.nodemap
    }
    pub fn set_integer(&mut self, name: &str, v: i64) -> Result<(), GenicamError> {
        let n = self
            .nodemap_mut()
            .get_mut(name)
            .ok_or_else(|| GenApiError::NodeNotFound(name.into()))?;
        match n {
            Node::Integer(ref mut int) => {
                if v < int.min || v > int.max {
                    return Err(GenApiError::Range(name.into()).into());
                }
                int.value = v;
                // TODO: map to registers via GenApi model, then self.transport.write_mem(...)
                Ok(())
            }
            _ => Err(GenApiError::TypeMismatch(name.into()).into()),
        }
    }
}

impl<T> Camera<T>
where
    T: Transport + ActionTransport + Send + Sync,
{
    pub async fn fire_action(&self, params: &ActionParams) -> Result<AckSummary, GenicamError> {
        self.transport.fire_action(params).await
    }
}

#[async_trait]
impl ActionTransport for tl_gige::GigeDevice {
    async fn fire_action(&self, params: &ActionParams) -> Result<AckSummary, GenicamError> {
        let destination = self.remote_addr();
        tl_gige::action::send_action(destination, params)
            .await
            .map_err(|err| GenicamError::transport(err.to_string()))
    }
}
