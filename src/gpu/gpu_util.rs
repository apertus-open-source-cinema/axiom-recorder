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

pub use narui::VulkanContext;

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
