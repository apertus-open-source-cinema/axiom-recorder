use anyhow::anyhow;
use std::{
    any::{type_name, Any},
    sync::Arc,
};

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
    pub fn downcast<T: Send + Sync + 'static>(&self) -> anyhow::Result<Arc<T>> {
        let downcast_result = self.data.clone().downcast::<T>();
        downcast_result.map_err(|_| {
            anyhow!(
                "Payload containing {} cannot be made into {}. The nodes you connected have incompatible port types.",
                self.type_name,
                type_name::<T>()
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::pipeline_processing::payload::Payload;


    #[test]
    fn test_payload() {
        let payload: Payload = Payload::from(0u32);
        let value = payload.downcast::<u32>().unwrap();
        assert_eq!(*value, 0u32);
    }
}
