use crate::pipeline_processing::{
    node::ProcessingNode,
    payload::Payload,
    processing_context::ProcessingContext,
};
use anyhow::Result;
use std::sync::Arc;

pub async fn pull_unordered(
    context: &ProcessingContext,
    input: Arc<dyn ProcessingNode + Send + Sync>,
    on_payload: impl Fn(Payload, u64) -> Result<()>,
) -> Result<()> {
    let range = match input.get_caps().frame_count {
        Some(frame_count) => 0..frame_count,
        None => 0..u64::MAX,
    };
    let on_payload = &on_payload;
    futures::future::try_join_all(range.map(move |frame_number| {
        let input = input.clone();
        async move {
            let input = input.clone().pull(frame_number, context.for_frame(frame_number)).await?;
            on_payload(input, frame_number)?;
            Ok::<(), anyhow::Error>(())
        }
    }))
    .await?;
    Ok(())
}
