use crate::pipeline_processing::payload::Payload;
use anyhow::{anyhow, Result};
use std::{
    any::{type_name, Any},
    sync::{Arc, MutexGuard},
};

pub trait ProcessingNode: Send + Sync {
    fn process(
        &self,
        input: &mut Payload,
        frame_lock: MutexGuard<u64>,
    ) -> anyhow::Result<Option<Payload>>;
    fn size_hint(&self) -> Option<u64> { None }
}
