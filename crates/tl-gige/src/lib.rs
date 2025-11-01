//! GigE Vision TL: discovery (GVCP), control (GenCP/GVCP), streaming (GVSP).

pub mod gvcp;
pub mod gvsp;
pub mod nic;
pub mod stats;

pub use gvcp::{discover, discover_on_interface, DeviceInfo, GigeDevice, GigeError, GVCP_PORT};
