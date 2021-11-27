use crate::pipeline_processing::payload::Payload;

use crate::pipeline_processing_legacy::execute::ProcessingStageLockWaiter;

pub trait ProcessingNode: Send + Sync {
    fn process(
        &self,
        input: &mut Payload,
        frame_lock: ProcessingStageLockWaiter,
    ) -> anyhow::Result<Option<Payload>>;
    fn size_hint(&self) -> Option<u64> { None }
}