use crate::pipeline_processing::{
    node::{ProcessingNode, ProgressUpdate},
    payload::Payload,
    processing_context::ProcessingContext,
};
use anyhow::Result;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

pub async fn pull_unordered(
    context: &ProcessingContext,
    progress_callback: Arc<dyn Fn(ProgressUpdate) + Send + Sync>,
    input: Arc<dyn ProcessingNode + Send + Sync>,
    on_payload: impl Fn(Payload, u64) -> Result<()>,
) -> Result<()> {
    let total_frames = input.get_caps().frame_count;
    let latest_frame = Arc::new(AtomicU64::new(0));

    let range = match total_frames {
        Some(frame_count) => 0..frame_count,
        None => 0..u64::MAX,
    };

    let on_payload = &on_payload;
    futures::future::try_join_all(range.map(move |frame_number| {
        let input = input.clone();
        let latest_frame = latest_frame.clone();
        let progress_callback = progress_callback.clone();
        async move {
            let input = input.clone().pull(frame_number, &context.for_frame(frame_number)).await?;
            on_payload(input, frame_number)?;

            let latest_frame = latest_frame.fetch_max(frame_number, Ordering::Relaxed);
            progress_callback(ProgressUpdate { latest_frame, total_frames });

            Ok::<(), anyhow::Error>(())
        }
    }))
    .await?;
    Ok(())
}
