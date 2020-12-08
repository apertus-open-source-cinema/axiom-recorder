use crate::frame::typing_hacks::Buffer;
use anyhow::{anyhow, Result};
use std::{ops::Deref, sync::Arc};

pub struct RgbFrame {
    pub width: u64,
    pub height: u64,
    pub buffer: Arc<dyn Buffer>,
}
impl RgbFrame {
    pub fn from_bytes(
        bytes: impl Deref<Target = [u8]> + Send + Sync + 'static,
        width: u64,
        height: u64,
    ) -> Result<RgbFrame> {
        if (width * height * 3) > (bytes.len() as u64) {
            return Err(anyhow!(
                "buffer is to small (expected {}, found {})",
                width * height * 3,
                bytes.len()
            ));
        }

        Ok(RgbFrame { width, height, buffer: Arc::new(bytes) })
    }
}


impl AsRef<[u8]> for RgbFrame {
    fn as_ref(&self) -> &[u8] { &self.buffer }
}
