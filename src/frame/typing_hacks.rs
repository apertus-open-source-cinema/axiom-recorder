use std::{any::Any, ops::Deref, sync::Arc};

pub trait IntoAny {
    fn into_any(self: Arc<Self>) -> Arc<dyn Any + Send + Sync + 'static>;
}
impl<T: Any + Send + Sync + 'static> IntoAny for T {
    fn into_any(self: Arc<Self>) -> Arc<dyn Any + Send + Sync + 'static> { self }
}

pub trait Buffer: Deref<Target = [u8]> + IntoAny + Send + Sync {}
impl<T: std::ops::Deref<Target = [u8]> + 'static + Send + Sync> Buffer for T {}
