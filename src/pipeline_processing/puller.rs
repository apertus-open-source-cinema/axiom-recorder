// TODO(robin): make pullers pull the actual amount specified, requesting more
// if there are errors
use crate::pipeline_processing::{
    node::{EOFError, InputProcessingNode, ProgressUpdate},
    payload::Payload,
    processing_context::ProcessingContext,
};
use anyhow::Result;
use futures::{
    stream::{FuturesOrdered, FuturesUnordered},
    StreamExt,
};
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};

pub async fn pull_unordered(
    context: &ProcessingContext,
    progress_callback: Arc<dyn Fn(ProgressUpdate) + Send + Sync>,
    input: InputProcessingNode,
    number_of_frames: u64,
    on_payload: impl Fn(Payload, u64) -> Result<()> + Send + Sync + Clone + 'static,
) -> Result<()> {
    let mut range = match (number_of_frames, input.get_caps().frame_count) {
        (0, None) => 0..u32::MAX,
        (0, Some(n)) => 0..(n as u32),
        (n, None) => 0..(n as u32),
        (n, Some(m)) => 0..((n as u32).min(m as u32)),
    };

    let total_frames = if range.end == u32::MAX { None } else { Some(range.end as _) };

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
            let context_clone = context.clone();
            let input = input.clone_for_same_puller();
            let on_payload = on_payload.clone();
            let progress_callback = progress_callback.clone();
            let latest_frame = latest_frame.clone();
            let should_stop_fut = should_stop.clone();
            let res = futures_unordered.push(context.spawn(async move {
                match input.pull(frame as _, &context_clone).await {
                    Ok(pulled) => on_payload(pulled, frame as _)?,
                    Err(e) => {
                        // TODO(robin): clean up into own trait?
                        eprintln!("error pulling frame {frame}: {e:#}");
                        if let Some(&EOFError) = e.downcast_ref::<EOFError>() {
                            eprintln!("end of file, exiting");
                            should_stop_fut.store(true, Ordering::SeqCst);
                        } else if let Some(e) = e.downcast_ref::<Arc<anyhow::Error>>() {
                            if let Some(&EOFError) = e.downcast_ref::<EOFError>() {
                                eprintln!("end of file, exiting");
                                should_stop_fut.store(true, Ordering::SeqCst);
                            }
                        }
                    }
                }

                let latest_frame = latest_frame.fetch_max(frame as _, Ordering::Relaxed);
                progress_callback(ProgressUpdate { latest_frame, total_frames });

                Ok::<(), anyhow::Error>(())
            }));

            if should_stop.load(Ordering::SeqCst) {
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
    progress_callback: Arc<dyn Fn(ProgressUpdate) + Send + Sync>,
    input: InputProcessingNode,
    number_of_frames: u64,
) -> flume::Receiver<Payload> {
    let mut range = match (number_of_frames, input.get_caps().frame_count) {
        (0, None) => 0..u32::MAX,
        (0, Some(n)) => 0..(n as u32),
        (n, None) => 0..(n as u32),
        (n, Some(m)) => 0..((n as u32).min(m as u32)),
    };

    let total_frames = if range.end == u32::MAX { None } else { Some(range.end as _) };

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
                    let context_clone = context.clone();
                    let input = input.clone_for_same_puller();
                    let progress_callback = progress_callback.clone();
                    let latest_frame = latest_frame.clone();
                    futures_ordered.push(context.spawn(async move {
                        let input = input.pull(frame as _, &context_clone).await;
                        let latest_frame = latest_frame.fetch_max(frame as _, Ordering::Relaxed);
                        progress_callback(ProgressUpdate { latest_frame, total_frames });

                        (input, frame as u64)
                    }));
                }
            }
        })
    });

    rx
}

/*
pub struct OrderedPuller {
    rx: Option<Receiver<Payload>>,
    join_handle: Option<JoinHandle<Result<()>>>,
}

/*
 *
    context: &ProcessingContext,
    progress_callback: Arc<dyn Fn(ProgressUpdate) + Send + Sync>,
    input: InputProcessingNode,
    number_of_frames: u64,
    on_payload: impl Fn(Payload, u64) -> Result<()> + Send + Sync + Clone + 'static,
 * */

impl OrderedPuller {
    pub fn new(
        context: &ProcessingContext,
        input: InputProcessingNode,
        do_loop: bool,
        number_of_frames: u64,
    ) -> Self {
        let (tx, rx) = sync_channel::<Payload>(context.num_threads());
        let context = context.clone();
        let join_handle = thread::spawn(move || {
            pollster::block_on(pull_ordered(&context, Arc::new(|_| {}), input, number_of_frames, move |input, frame_number| {
                dbg!(frame_number);
                Ok(tx.send(input)?)
            }));

            Ok(())
        });

        Self { rx: Some(rx), join_handle: Some(join_handle) }
    }
}

impl Drop for OrderedPuller {
    fn drop(&mut self) {
        drop(self.rx.take());
        self.join_handle.take().unwrap().join().unwrap().unwrap();
    }
}

impl Deref for OrderedPuller {
    type Target = Receiver<Payload>;

    fn deref(&self) -> &Self::Target { self.rx.as_ref().unwrap() }
}
*/
