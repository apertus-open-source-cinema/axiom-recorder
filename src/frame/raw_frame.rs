use crate::frame::typing_hacks::Buffer;
use anyhow::{anyhow, Result};
use std::{ops::Deref, sync::Arc};

/// The main data structure for transferring and representing single raw frames
/// of a video stream
pub struct RawFrame {
    pub width: u64,
    pub height: u64,
    pub bit_depth: u64,
    pub buffer: Arc<dyn Buffer>,
    pub cfa: CfaDescriptor,
}
impl RawFrame {
    pub fn from_bytes(
        bytes: impl Deref<Target = [u8]> + Send + Sync + 'static,
        width: u64,
        height: u64,
        bit_depth: u64,
        cfa: CfaDescriptor,
    ) -> Result<RawFrame> {
        if (width * height * bit_depth / 8) > (bytes.len() as u64) {
            return Err(anyhow!(
                "buffer is to small for raw frame (expected {}, found {})",
                width * height * bit_depth / 8,
                bytes.len()
            ));
        }

        Ok(RawFrame { width, height, buffer: Arc::new(bytes), bit_depth, cfa })
    }
}
#[derive(Debug, Copy, Clone)]
pub struct CfaDescriptor {
    pub first_is_red_x: bool,
    pub first_is_red_y: bool,
}
impl CfaDescriptor {
    pub fn from_first_red(first_is_red_x: bool, first_is_red_y: bool) -> Self {
        CfaDescriptor { first_is_red_x, first_is_red_y }
    }
}
