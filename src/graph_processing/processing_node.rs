use anyhow::{anyhow, Result};
use std::{any::Any, sync::Arc};

pub trait ProcessingNode {
    fn process(&self, input: &mut Payload) -> anyhow::Result<Option<Payload>>;
    fn size_hint(&self) -> Option<u64> { None }
}

#[derive(Clone, Debug)]
pub struct Payload(Arc<Box<dyn Any + Send>>);
impl Payload {
    pub fn empty() -> Self { Payload::from(()) }
    pub fn from<T: Send + 'static>(payload: T) -> Self { Payload(Arc::new(Box::new(payload))) }
    pub fn downcast<T: 'static>(&self) -> Result<&T> {
        self.0.downcast_ref().ok_or_else(|| anyhow!("couldnt downcast."))
    }
}

#[cfg(test)]
mod tests {
    use crate::graph_processing::processing_node::Payload;
    

    #[test]
    fn test_payload() {
        let payload: Payload = Payload::from(0u32);
        let value: &u32 = payload.downcast().unwrap();
        assert_eq!(*value, 0u32);
    }
}
