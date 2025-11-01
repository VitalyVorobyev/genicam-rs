//! GenCP: generic control protocol encode/decode (transport-agnostic).

use bytes::{BufMut, Bytes, BytesMut};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GenCpError {
    #[error("invalid packet: {0}")]
    InvalidPacket(&'static str),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, Copy)]
pub enum OpCode {
    // Keep generic; fill actual codes as you implement
    ReadMem,
    WriteMem,
    // Add more as needed (events, pending ack, etc.)
}

#[derive(Debug, Clone)]
pub struct GenCpCmd {
    pub id: u16,          // command id / request id
    pub opcode: OpCode,
    pub payload: Bytes,   // already formatted per opcode
}

#[derive(Debug, Clone)]
pub struct GenCpAck {
    pub id: u16,
    pub status: u16,      // transport-specific status codes
    pub payload: Bytes,
}

pub fn encode_cmd(cmd: &GenCpCmd) -> Bytes {
    // placeholder header + payload; fill per spec
    let mut b = BytesMut::with_capacity(64 + cmd.payload.len());
    // header...
    // b.put_u16(cmd.id); ...
    b.put_slice(&cmd.payload);
    b.freeze()
}

pub fn decode_ack(buf: &[u8]) -> Result<GenCpAck, GenCpError> {
    // parse header, status, payload; validate lengths
    if buf.len() < 4 { return Err(GenCpError::InvalidPacket("too short")); }
    // placeholder
    Ok(GenCpAck { id: 0, status: 0, payload: Bytes::copy_from_slice(buf) })
}
