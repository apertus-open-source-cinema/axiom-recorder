use crate::frame::typing_hacks::Buffer;
use anyhow::{anyhow, Result};
use lazy_static::lazy_static;
use owning_ref::OwningHandle;
use std::{ops::Deref, sync::Arc};
use vulkano::{
    buffer::{
        cpu_access::{ReadLock, ReadLockError},
        BufferUsage,
        CpuAccessibleBuffer,
    },
    device::{physical::PhysicalDevice, Device, DeviceExtensions, Queue},
    instance::Instance,
    memory::{
        pool::{PotentialDedicatedAllocation, StdMemoryPoolAlloc},
        Content,
    },
    Version,
};
use vulkano::buffer::{DeviceLocalBuffer, ImmutableBuffer};
use vulkano::sync::GpuFuture;
use crate::frame::{CpuStorage, Frame, GpuBuffer};
use crate::pipeline_processing::payload::Payload;

#[derive(Clone)]
pub struct VulkanContext {
    pub device: Arc<Device>,
    pub queues: Vec<Arc<Queue>>,
}
lazy_static! {
    pub static ref VULKAN_CONTEXT: VulkanContext = VulkanContext::create().unwrap();
}
impl VulkanContext {
    pub fn create() -> Result<Self> {
        let required_extensions = vulkano_win::required_extensions();
        let instance = Instance::new(None, Version::V1_2, &required_extensions, None)?;
        let physical = PhysicalDevice::enumerate(&instance)
            .next()
            .ok_or_else(|| anyhow!("No physical device found"))?;
        let queue_family = physical.queue_families().map(|qf| (qf, 0.5)); // All queues have the same priority
        let device_ext = DeviceExtensions {
            khr_swapchain: true,
            khr_storage_buffer_storage_class: true,
            khr_8bit_storage: true,
            ..DeviceExtensions::none()
        };
        let (device, queues) =
            Device::new(physical, physical.supported_features(), &device_ext, queue_family)?;
        Ok(Self { device, queues: queues.collect() })
    }
    pub fn get() -> Self { VULKAN_CONTEXT.clone() }
}

impl<Interpretation: Clone> Frame<Interpretation, CpuStorage> {
    fn to_immutable_buffer(&self) -> (Frame<Interpretation, GpuBuffer>, impl GpuFuture) {
        let queue = VulkanContext::get().queues.iter().next().unwrap().clone();
        let (buffer, fut) = ImmutableBuffer::from_iter(self.storage.clone().into_iter(), BufferUsage {
            storage_buffer: true,
            storage_texel_buffer: true,
            ..BufferUsage::none()
        }, queue).unwrap();

        (Frame {
            interp: self.interp.clone(),
            storage: buffer
        }, fut)
    }
}



pub fn ensure_gpu_buffer<Interpretation: Clone + Send + Sync + 'static>(payload: &mut Payload) -> anyhow::Result<(Arc<Frame<Interpretation, GpuBuffer>>, impl GpuFuture)> {
    if let Ok(frame) = payload.downcast::<Frame<Interpretation, CpuStorage>>() {
        let (buf, fut) = frame.to_immutable_buffer();
        Ok((Arc::new(buf), fut.boxed()))
    } else if let Ok(frame) = payload.downcast::<Frame<Interpretation, GpuBuffer>>() {
        Ok((frame, vulkano::sync::now(VulkanContext::get().device.clone()).boxed()))
    } else {
        Err(anyhow!("wanted a frame as interpretation {}, but the payload was of type {}", std::any::type_name::<Interpretation>(), payload.type_name))
    }
}
