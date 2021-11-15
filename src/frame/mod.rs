use vulkano::buffer::{BufferUsage, CpuAccessibleBuffer, DeviceLocalBuffer, ImmutableBuffer, TypedBufferAccess};
use std::sync::Arc;
use std::ops::Deref;
use crate::gpu::gpu_util::VulkanContext;

pub mod bit_depth_converter;
pub mod typing_hacks;

pub trait FrameInterpretation {
    fn required_bytes(&self) -> usize;
}

/// The main data structure for transferring and representing single raw frames
/// of a video stream
pub struct Frame<Interpretation, Storage> {
    pub interp: Interpretation,
    pub storage: Storage
}

pub struct CpuStorage {
    buf: Arc<CpuAccessibleBuffer<[u8]>>
}

impl CpuStorage {
    pub fn len(&self) -> usize {
        self.buf.len() as _
    }

    pub fn buffer(&self) -> Arc<CpuAccessibleBuffer<[u8]>> {
        self.buf.clone()
    }

    pub fn as_slice<FN: FnOnce(&[u8]) -> R, R>(&self, func: FN) -> R {
        func(&*self.buf.read().unwrap())
    }

    pub fn as_mut_slice<FN: FnOnce(&mut [u8]) -> R, R>(&mut self, func: FN) -> R {
        func(&mut *self.buf.write().unwrap())
    }

    pub unsafe fn uninit(len: usize) -> Self {
        Self {
            buf: CpuAccessibleBuffer::uninitialized_array(VulkanContext::get().device.clone(), len as _, BufferUsage {
                storage_buffer: true,
                storage_texel_buffer: true,
                transfer_source: true,
                ..BufferUsage::none()
            }, true).unwrap()
        }
    }
}

// pub type CpuStorage = Vec<u8>;
pub type GpuBuffer = Arc<dyn TypedBufferAccess<Content = [u8]> + Send + Sync>;

/*
impl<Interpretation: FrameInterpretation> Frame<Interpretation, CpuStorage> {
    pub fn from_bytes(
        bytes: impl Deref<Target = [u8]>,
        interpretation: Interpretation
    ) -> anyhow::Result<Frame<Interpretation, CpuStorage>> {
        if interpretation.required_bytes() > bytes.len() {
            return Err(anyhow::anyhow!(
                "buffer is to small for raw frame (expected {} bytes, found {} bytes)",
                interpretation.required_bytes(),
                bytes.len()
            ));
        }

        Ok(Frame { storage: bytes.to_vec(), interp: interpretation })
    }
}

impl<Interpretation> AsRef<[u8]> for Frame<Interpretation, CpuStorage> {
    fn as_ref(&self) -> &[u8] { &self.storage[..] }
}
 */

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

#[derive(Clone, Copy)]
pub struct Raw {
    pub width: u64,
    pub height: u64,
    pub bit_depth: u64,
    pub cfa: CfaDescriptor,
}

impl FrameInterpretation for Raw {
    fn required_bytes(&self) -> usize {
        self.width as usize * self.height as usize * self.bit_depth as usize / 8
    }
}

#[derive(Clone, Copy)]
pub struct Rgb {
    pub width: u64,
    pub height: u64,
}

impl FrameInterpretation for Rgb {
    fn required_bytes(&self) -> usize {
        self.width as usize * self.height as usize
    }
}

#[derive(Clone, Copy)]
pub struct Rgba {
    pub width: u64,
    pub height: u64,
}
impl FrameInterpretation for Rgba {
    fn required_bytes(&self) -> usize {
        self.width as usize * self.height as usize
    }
}
