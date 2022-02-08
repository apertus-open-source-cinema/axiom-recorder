

use std::sync::Arc;
use futures::{StreamExt};
use futures::future::BoxFuture;
use futures::stream::FuturesUnordered;
use owning_ref::{OwningHandle};

use vulkano::buffer::{BufferAccess, CpuAccessibleBuffer, TypedBufferAccess};
use vulkano::buffer::cpu_access::WriteLock;

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

    pub async fn as_slice_async<FN: for<'a> FnOnce(&'a [u8]) -> BoxFuture<'a, R>, R>(&self, func: FN) -> R {
        func(&*self.buf.read().unwrap()).await
    }

    pub fn as_mut_slice<FN: FnOnce(&mut [u8]) -> R, R>(&mut self, func: FN) -> R {
        func(&mut *self.buf.write().unwrap())
    }

    pub async fn as_mut_slice_async<FN: for<'a> FnOnce(&'a mut [u8]) -> BoxFuture<'a, R>, R>(&mut self, func: FN) -> R {
        func(&mut *self.buf.write().unwrap()).await
    }
}

pub struct ChunkedCpuBuffer<'a> {
    buf_holder: OwningHandle<Arc<CpuAccessibleBuffer<[u8]>>, WriteLock<'a, [u8]>>,
    locks: Vec<futures::lock::Mutex<usize>>,
    n: usize,
    chunk_size: usize,
    ptr: *mut u8
}

unsafe impl<'a> Send for ChunkedCpuBuffer<'a> {}
unsafe impl<'a> Sync for ChunkedCpuBuffer<'a> {}

impl<'a> ChunkedCpuBuffer<'a> {
    pub fn new(cpu_buffer: CpuBuffer, n: usize) -> Self {
        let chunk_size = cpu_buffer.len() / n;
        let mut buf_holder = OwningHandle::new_with_fn(cpu_buffer.buf, |buf| unsafe {
            (*buf).write().unwrap()
        });

        let ptr = buf_holder.as_mut_ptr();

        let locks = (0..n).into_iter().map(futures::lock::Mutex::new).collect();

        Self {
            buf_holder,
            n,
            chunk_size,
            locks,
            ptr
        }
    }

    pub async fn zip_with<O, F: for<'b> Fn(&'b mut [u8], &'b [O]) + Clone>(&self, other: &[O], fun: F) {
        let mut futs = self.locks.iter().map(futures::lock::Mutex::lock).collect::<FuturesUnordered<_>>();
        let chunks = other.chunks(other.len() / self.n).collect::<Vec<_>>();
        while let Some(i) = futs.next().await {
            unsafe {
                fun(std::slice::from_raw_parts_mut(self.ptr.add(*i * self.chunk_size), self.chunk_size), chunks[*i])
            }
        }
    }

    pub fn unchunk(self) -> CpuBuffer {
        self.buf_holder.into_owner().into()
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
