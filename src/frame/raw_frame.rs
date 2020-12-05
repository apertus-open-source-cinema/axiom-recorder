use crate::frame::buffer::Buffer;
use anyhow::{anyhow, Result};
use std::ops::Deref;

/// The main data structure for transferring and representing single raw frames
/// of a video stream
pub struct RawFrame {
    pub width: u64,
    pub height: u64,
    pub buffer: Buffer,
}
impl RawFrame {
    pub fn from_bytes(
        byte_vec: impl Deref<Target=[u8]> + Send + Sync + 'static,
        width: u64,
        height: u64,
        bit_depth: u64,
    ) -> Result<RawFrame> {
        if (width * height * bit_depth / 8) > (byte_vec.len() as u64) {
            return Err(anyhow!(
                "buffer is to small (expected {}, found {})",
                width * height * bit_depth / 8,
                byte_vec.len()
            ));
        }

        Ok(RawFrame { width, height, buffer: Buffer::new(byte_vec, bit_depth)? })
    }
    pub fn convert_to_8_bit(&self) -> Self {
        RawFrame { width: self.width, height: self.height, buffer: self.buffer.repack_to_8_bit() }
    }
}
