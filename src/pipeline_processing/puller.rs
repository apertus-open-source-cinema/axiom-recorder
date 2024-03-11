// TODO(robin): make pullers pull the actual amount specified, requesting more
// if there are errors
use crate::pipeline_processing::{
    node::{EOFError, InputProcessingNode, ProgressUpdate, Request},
    payload::Payload,
    processing_context::{Priority, ProcessingContext},
};
use anyhow::Result;
use bytemuck::Contiguous;
use futures::{
    stream::{FuturesOrdered, FuturesUnordered},
    StreamExt,
};
use std::{
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    u64,
};

pub async fn pull_unordered(
    context: &ProcessingContext,
    output_priority: u8,
    progress_callback: Arc<dyn Fn(ProgressUpdate) + Send + Sync>,
    input: InputProcessingNode,
    number_of_frames: Option<u64>,
    on_payload: impl Fn(Payload, u64) -> Result<()> + Send + Sync + Clone + 'static,
) -> Result<()> {
    let mut range = match (number_of_frames, input.get_caps().frame_count) {
        (None, None) => 0..u64::MAX_VALUE,
        (None, Some(n)) => 0..n,
        (Some(n), None) => 0..n,
        (Some(n), Some(m)) => 0..n.min(m),
    };

    let total_frames = if range.end == u64::MAX_VALUE { None } else { Some(range.end as _) };

    let latest_frame = Arc::new(AtomicU64::new(0));
    let should_stop = Arc::new(AtomicBool::new(false));
    let mut futures_unordered = FuturesUnordered::new();

    loop {
        if range.is_empty() && futures_unordered.is_empty() {
            break;
        }
        if range.is_empty() || futures_unordered.len() >= context.num_threads() {
            if let Some(result) = futures_unordered.next().await {
                result?;
            }
        }
        if let Some(frame) = range.next() {
            let input = input.clone_for_same_puller();
            let on_payload = on_payload.clone();
            let progress_callback = progress_callback.clone();
            let latest_frame = latest_frame.clone();
            let should_stop_fut = should_stop.clone();
            let res = futures_unordered.push(context.spawn(
                Priority::new(output_priority, frame),
                async move {
                    match input.pull(Request::new(output_priority, frame)).await {
                        Ok(pulled) => on_payload(pulled, frame as _)?,
                        Err(e) => {
                            // TODO(robin): clean up into own trait?
                            eprintln!("error pulling frame {frame}: {e:#}");
                            if let Some(&EOFError) = e.downcast_ref::<EOFError>() {
                                eprintln!("end of file, exiting");
                                should_stop_fut.store(true, Ordering::Relaxed);
                            } else if let Some(e) = e.downcast_ref::<Arc<anyhow::Error>>() {
                                if let Some(&EOFError) = e.downcast_ref::<EOFError>() {
                                    eprintln!("end of file, exiting");
                                    should_stop_fut.store(true, Ordering::Relaxed);
                                }
                            }
                        }
                    }

                    let latest_frame = latest_frame.fetch_max(frame as _, Ordering::Relaxed);
                    progress_callback(ProgressUpdate { latest_frame, total_frames });

                    Ok::<(), anyhow::Error>(())
                },
            ));

            if should_stop.load(Ordering::Relaxed) {
                break;
            }

            res
        }
    }

    Ok(())
}

// TODO(robin): abort the thread when we want to stop
pub fn pull_ordered(
    context: &ProcessingContext,
    output_priority: u8,
    progress_callback: Arc<dyn Fn(ProgressUpdate) + Send + Sync>,
    input: InputProcessingNode,
    number_of_frames: Option<u64>,
) -> flume::Receiver<Payload> {
    let mut range = match (number_of_frames, input.get_caps().frame_count) {
        (None, None) => 0..u64::MAX_VALUE,
        (None, Some(n)) => 0..n,
        (Some(n), None) => 0..n,
        (Some(n), Some(m)) => 0..n.min(m),
    };

    let total_frames = if range.end == u64::MAX_VALUE { None } else { Some(range.end as _) };

    let latest_frame = Arc::new(AtomicU64::new(0));
    let mut futures_ordered = FuturesOrdered::new();

    let (tx, rx) = flume::bounded(context.num_threads());

    let context = context.clone();
    std::thread::spawn(move || {
        context.block_on(async {
            loop {
                if range.is_empty() && futures_ordered.is_empty() {
                    break;
                }
                if range.is_empty() || futures_ordered.len() >= context.num_threads() {
                    if let Some(input) = futures_ordered.next().await {
                        let input: (Result<_, anyhow::Error>, _) = input;
                        match input {
                            (Ok(input), _) => tx.send_async(input).await.unwrap(),
                            (Err(e), frame) => {
                                // TODO(robin): clean up into own trait?
                                eprintln!("error pulling frame {frame}: {e:#}");
                                if let Some(&EOFError) = e.downcast_ref::<EOFError>() {
                                    eprintln!("end of file, exiting");
                                    break;
                                } else if let Some(e) = e.downcast_ref::<Arc<anyhow::Error>>() {
                                    if let Some(&EOFError) = e.downcast_ref::<EOFError>() {
                                        eprintln!("end of file, exiting");
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
                if let Some(frame) = range.next() {
                    let input = input.clone_for_same_puller();
                    let progress_callback = progress_callback.clone();
                    let latest_frame = latest_frame.clone();
                    futures_ordered.push_back(context.spawn(
                        Priority::new(output_priority, frame),
                        async move {
                            let input = input.pull(Request::new(output_priority, frame)).await;
                            let latest_frame =
                                latest_frame.fetch_max(frame as _, Ordering::Relaxed);
                            progress_callback(ProgressUpdate { latest_frame, total_frames });

                            (input, frame as u64)
                        },
                    ));
                }
            }
        })
    });

    rx
}
