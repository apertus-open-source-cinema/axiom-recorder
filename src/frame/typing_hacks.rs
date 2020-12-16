use std::{
    any::Any,
    ops::{Deref, Range},
    sync::Arc,
};

pub trait IntoAny {
    fn into_any(self: Arc<Self>) -> Arc<dyn Any + Send + Sync + 'static>;
}
impl<T: Any + Send + Sync + 'static> IntoAny for T {
    fn into_any(self: Arc<Self>) -> Arc<dyn Any + Send + Sync + 'static> { self }
}

pub trait Buffer: Deref<Target = [u8]> + IntoAny + Send + Sync {}
impl<T: std::ops::Deref<Target = [u8]> + 'static + Send + Sync> Buffer for T {}

pub struct SubBuffer {
    bytes: Box<dyn Buffer>,
    range: Range<usize>,
}
impl SubBuffer {
    pub fn from_buffer(buffer: impl Buffer + 'static, range: Range<usize>) -> SubBuffer {
        SubBuffer { bytes: Box::new(buffer), range }
    }
}
impl Deref for SubBuffer {
    type Target = [u8];

    fn deref(&self) -> &Self::Target { &self.bytes[self.range.clone()] }
}
