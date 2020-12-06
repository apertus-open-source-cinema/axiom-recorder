use anyhow::Result;
use owning_ref::OwningHandle;
use std::{ops::Deref, sync::Arc};
use vulkano::{
    buffer::{
        cpu_access::{ReadLock, ReadLockError},
        CpuAccessibleBuffer,
    },
    memory::Content,
};

pub struct CpuAccessibleBufferReadView<T: 'static + ?Sized>(
    OwningHandle<Arc<CpuAccessibleBuffer<T>>, ReadLock<'static, T>>,
);
impl<T: ?Sized + Content> CpuAccessibleBufferReadView<T> {
    pub fn new(cpu_accessible_buffer: Arc<CpuAccessibleBuffer<T>>) -> Result<Self> {
        Ok(CpuAccessibleBufferReadView(OwningHandle::try_new::<_, ReadLockError>(
            cpu_accessible_buffer,
            |cpu_accessible_buffer| unsafe { (*cpu_accessible_buffer).read() },
        )?))
    }
}
impl<T: ?Sized> Deref for CpuAccessibleBufferReadView<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target { self.0.deref() }
}
