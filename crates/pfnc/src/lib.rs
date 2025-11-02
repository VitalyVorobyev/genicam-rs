#![cfg_attr(docsrs, feature(doc_cfg))]
//! PFNC pixel format helpers (placeholder).

#![allow(dead_code)]

/// Placeholder type until PFNC parsing lands.
pub struct PixelFormatCode(pub u32);

impl PixelFormatCode {
    /// Return the raw PFNC code.
    pub fn raw(&self) -> u32 {
        self.0
    }
}
