use anyhow::{anyhow, Result};
use crate::frame::buffer::{Buffer};

/// The main data structure for transferring and representing single raw frames
/// of a video stream
#[derive(Debug)]
pub struct RawFrame {
    pub width: u64,
    pub height: u64,
    pub buffer: Buffer,
}
impl RawFrame where {
    pub fn from_byte_vec(byte_vec: Vec<u8>, width: u64, height: u64, bit_depth: u64) -> Result<RawFrame> {
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
        RawFrame {
            width: self.width,
            height: self.height,
            buffer: self.buffer.unpack_u8()
        }
    }
}
