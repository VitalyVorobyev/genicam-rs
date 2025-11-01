//! Public API: Camera, Stream, Image, Feature access bridging TL <-> GenApi.

use genapi_core::{GenApiError, Node, NodeMap};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GenicamError {
    #[error(transparent)]
    GenApi(#[from] GenApiError),
    #[error("transport: {0}")]
    Transport(&'static str),
}

pub trait Transport {
    fn read_mem(&self, addr: u64, len: usize) -> Result<Vec<u8>, GenicamError>;
    fn write_mem(&self, addr: u64, data: &[u8]) -> Result<(), GenicamError>;
    // start_stream / stop_stream / events etc.
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
