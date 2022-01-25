use crate::pipeline_processing::{
    node::{ProcessingNode, ProgressUpdate},
    payload::Payload,
    processing_context::ProcessingContext,
};
use anyhow::Result;
use futures::{stream::FuturesUnordered, StreamExt};
use std::{
    collections::VecDeque,
    ops::Deref,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc::{sync_channel, Receiver, SendError},
        Arc,
    },
    thread,
    thread::JoinHandle,
};


pub async fn pull_unordered(
    context: &ProcessingContext,
    progress_callback: Arc<dyn Fn(ProgressUpdate) + Send + Sync>,
    input: Arc<dyn ProcessingNode + Send + Sync>,
    number_of_frames: u64,
    on_payload: impl Fn(Payload, u64) -> Result<()> + Send + Sync + Clone + 'static,
) -> Result<()> {
    let mut range = match (number_of_frames, input.get_caps().frame_count) {
        (0, None) => 0..u64::MAX,
        (0, Some(n)) => 0..n,
        (n, None) => 0..n,
        (n, Some(m)) => 0..(n.min(m)),
    };

    let total_frames = if range.end == u64::MAX { None } else { Some(range.end) };

    let latest_frame = Arc::new(AtomicU64::new(0));
    let mut futures_unordered = FuturesUnordered::new();

    loop {
        if futures_unordered.len() >= context.num_threads() {
            if let Some(result) = futures_unordered.next().await {
                result?;
            } else {
                break;
            }
        }
        if let Some(frame) = range.next() {
            let context_clone = context.clone();
            let input = input.clone();
            let on_payload = on_payload.clone();
            let progress_callback = progress_callback.clone();
            let latest_frame = latest_frame.clone();
            futures_unordered.push(context.spawn(async move {
                match input.pull(frame, &context_clone).await {
                    Ok(pulled) => on_payload(pulled, frame)?,
                    Err(e) => println!("frame {} dropped. error:\n{:?}", frame, e),
                }

                let latest_frame = latest_frame.fetch_max(frame, Ordering::Relaxed);
                progress_callback(ProgressUpdate { latest_frame, total_frames });

                Ok::<(), anyhow::Error>(())
            }))
        }
    }

    Ok(())
}

pub struct OrderedPuller {
    rx: Option<Receiver<Payload>>,
    join_handle: Option<JoinHandle<Result<()>>>,
}

impl OrderedPuller {
    pub fn new(
        context: &ProcessingContext,
        input: Arc<dyn ProcessingNode + Send + Sync>,
        do_loop: bool,
        number_of_frames: u64,
    ) -> Self {
        let (tx, rx) = sync_channel::<Payload>(context.num_threads());
        let context = context.clone();
        let join_handle = thread::spawn(move || {
            let range = match (number_of_frames, input.get_caps().frame_count) {
                (0, None) => 0..u64::MAX,
                (0, Some(n)) => 0..n,
                (n, None) => 0..n,
                (n, Some(m)) => 0..(n.min(m)),
            };

            let mut range: Box<dyn Iterator<Item = u64>> =
                if do_loop { Box::new(range.cycle()) } else { Box::new(range) };

            let mut todo = VecDeque::with_capacity(context.num_threads());
            loop {
                while todo.len() < 10 {
                    if let Some(frame) = range.next() {
                        let context = context.for_frame(frame);
                        let input = input.clone();
                        todo.push_front(
                            context.clone().spawn(async move { input.pull(frame, &context).await }),
                        );
                    } else {
                        break;
                    }
                }
                if let Some(payload) = todo.pop_back() {
                    match pollster::block_on(payload) {
                        Ok(pulled) => match tx.send(pulled) {
                            Ok(()) => {}
                            Err(SendError(_)) => break,
                        },
                        Err(e) => println!("frame dropped. error:\n\t{:?}", e),
                    }
                } else {
                    break;
                }
            }

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
