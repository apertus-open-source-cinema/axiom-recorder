use anyhow::{anyhow, Result};
use std::{
    any::{type_name, Any},
    sync::Arc,
};

pub trait ProcessingNode: Send + Sync {
    fn process(&self, input: &mut Payload) -> anyhow::Result<Option<Payload>>;
    fn size_hint(&self) -> Option<u64> { None }
}

#[derive(Clone, Debug)]
pub struct Payload {
    data: Arc<dyn Any + Send + Sync>,
    pub type_name: String,
}
impl Payload {
    pub fn empty() -> Self { Payload::from(()) }
    pub fn from<T: Send + Sync + 'static>(payload: T) -> Self {
        Payload { data: Arc::new(payload), type_name: type_name::<T>().to_string() }
    }
    pub fn from_arc<T: Send + Sync + 'static>(payload: Arc<T>) -> Self {
        Payload { data: payload, type_name: type_name::<T>().to_string() }
    }
    pub fn downcast<T: Send + Sync + 'static>(&self) -> Result<Arc<T>> {
        let downcast_result = self.data.clone().downcast::<T>();
        downcast_result.map_err(|_| {
            anyhow!(
                "Payload containing {} cannot be made into {}",
                self.type_name,
                type_name::<T>()
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        frame::raw_frame::{CfaDescriptor, RawFrame},
        pipeline_processing::processing_node::Payload,
    };


    #[test]
    fn test_payload() {
        let payload: Payload = Payload::from(0u32);
        let value = payload.downcast::<u32>().unwrap();
        assert_eq!(*value, 0u32);
    }

    #[test]
    fn test_payload_raw_frame() {
        let payload = Payload::from(
            RawFrame::from_bytes(vec![1u8], 1, 1, 8, CfaDescriptor::from_first_red(true, true))
                .unwrap(),
        );
        let _value = payload.downcast::<RawFrame>().unwrap();
    }
}
