use crate::frame::typing_hacks::Buffer;
use anyhow::{anyhow, Result};
use lazy_static::lazy_static;
use owning_ref::OwningHandle;
use std::{ops::Deref, sync::Arc};
use vulkano::{
    buffer::{
        cpu_access::{ReadLock, ReadLockError},
        BufferAccess,
        BufferUsage,
        CpuAccessibleBuffer,
    },
    device::{Device, DeviceExtensions, Queue},
    instance::{Instance, PhysicalDevice},
    memory::{
        pool::{PotentialDedicatedAllocation, StdMemoryPoolAlloc},
        Content,
    },
};

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
        let instance = Instance::new(None, &required_extensions, None)?;
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

pub struct CpuAccessibleBufferReadView<T: 'static + ?Sized>(
    OwningHandle<Arc<CpuAccessibleBuffer<T>>, ReadLock<'static, T>>,
);
impl<T: ?Sized + Content> CpuAccessibleBufferReadView<T> {
    pub fn from_buffer(buffer: Arc<dyn Buffer>) -> Result<Arc<CpuAccessibleBufferReadView<[u8]>>> {
        let any_buffer = buffer.clone().into_any();
        Ok(match any_buffer.downcast::<CpuAccessibleBufferReadView<[u8]>>() {
            Ok(cpu_accessible_buffer) => cpu_accessible_buffer,
            Err(any_buffer) => {
                drop(any_buffer);
                Arc::new(CpuAccessibleBufferReadView::from_cpu_accessible_buffer(unsafe {
                    let uninitialized: Arc<CpuAccessibleBuffer<[u8]>> =
                        CpuAccessibleBuffer::uninitialized_array(
                            VulkanContext::get().device,
                            buffer.len(),
                            BufferUsage::all(),
                            true,
                        )?;

                    uninitialized.write().unwrap().clone_from_slice(&**buffer);
                    uninitialized
                })?)
            }
        })
    }
    pub fn from_cpu_accessible_buffer(
        cpu_accessible_buffer: Arc<CpuAccessibleBuffer<T>>,
    ) -> Result<Self> {
        Ok(CpuAccessibleBufferReadView(OwningHandle::try_new::<_, ReadLockError>(
            cpu_accessible_buffer,
            |cpu_accessible_buffer| unsafe { (*cpu_accessible_buffer).read() },
        )?))
    }
    pub fn as_cpu_accessible_buffer(
        &self,
    ) -> Arc<CpuAccessibleBuffer<T, PotentialDedicatedAllocation<StdMemoryPoolAlloc>>> {
        self.0.as_owner().clone()
    }
}
impl<T: ?Sized> Deref for CpuAccessibleBufferReadView<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target { self.0.deref() }
}
