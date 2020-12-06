use anyhow::{anyhow, Result};
use bytemuck::cast_slice;
use rayon::{
    iter::{IndexedParallelIterator, ParallelIterator},
    slice::{ParallelSlice, ParallelSliceMut},
};
use std::{fmt::Debug, ops::Deref, sync::Arc};

#[derive(Clone)]
pub struct Buffer {
    bytes: Arc<dyn Deref<Target = [u8]> + Send + Sync>,
    bit_depth: u64,
}
impl Buffer {
    pub fn new(
        buffer: impl Deref<Target = [u8]> + Send + Sync + 'static,
        bit_depth: u64,
    ) -> Result<Self> {
        match bit_depth {
            8..=32 => Ok(Self { bytes: Arc::new(buffer), bit_depth }),
            _ => Err(anyhow!("bit depth must be between 8 and 32, found {}", bit_depth)),
        }
    }
    pub fn bytes(&self) -> Arc<dyn Deref<Target = [u8]> + Send + Sync> { self.bytes.clone() }
    pub fn bit_depth(&self) -> u64 { self.bit_depth }
}
