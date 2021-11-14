use std::sync::Arc;

use vulkano::buffer::{BufferAccess, CpuAccessibleBuffer, TypedBufferAccess};

#[derive(Clone)]
pub struct CpuBuffer {
    buf: Arc<CpuAccessibleBuffer<[u8]>>,
}
impl From<Arc<CpuAccessibleBuffer<[u8]>>> for CpuBuffer {
    fn from(buf: Arc<CpuAccessibleBuffer<[u8]>>) -> Self { Self { buf } }
}
impl CpuBuffer {
    pub fn len(&self) -> usize { self.buf.len() as _ }

    pub fn cpu_accessible_buffer(&self) -> Arc<CpuAccessibleBuffer<[u8]>> { self.buf.clone() }

    pub fn as_slice<FN: FnOnce(&[u8]) -> R, R>(&self, func: FN) -> R {
        func(&*self.buf.read().unwrap())
    }

    pub fn as_mut_slice<FN: FnOnce(&mut [u8]) -> R, R>(&mut self, func: FN) -> R {
        func(&mut *self.buf.write().unwrap())
    }
}

#[derive(Clone)]
pub struct GpuBuffer {
    typed_buffer_access: Arc<dyn TypedBufferAccess<Content = [u8]> + Send + Sync>,
    buffer_access: Arc<(dyn BufferAccess)>,
}
impl<T: TypedBufferAccess<Content = [u8]> + Send + Sync + 'static> From<Arc<T>> for GpuBuffer {
    fn from(typed_buffer_acccess: Arc<T>) -> Self {
        Self {
            typed_buffer_access: typed_buffer_acccess.clone() as _,
            buffer_access: typed_buffer_acccess as _,
        }
    }
}
impl GpuBuffer {
    pub fn typed(&self) -> Arc<dyn TypedBufferAccess<Content = [u8]> + Send + Sync> {
        self.typed_buffer_access.clone()
    }
    pub fn untyped(&self) -> Arc<(dyn BufferAccess)> { self.buffer_access.clone() }
}
