use crate::pipeline_processing::{
    node::{ProcessingNode, ProgressUpdate},
    payload::Payload,
    processing_context::ProcessingContext,
};
use anyhow::Result;
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
    on_payload: impl Fn(Payload, u64) -> Result<()> + Send + Sync + Clone + 'static,
) -> Result<()> {
    let total_frames = input.get_caps().frame_count;
    let latest_frame = Arc::new(AtomicU64::new(0));

    let range = match total_frames {
        Some(frame_count) => 0..frame_count,
        None => 0..u64::MAX,
    };

    futures::future::try_join_all(range.map(move |frame_number| {
        let input = input.clone();
        let latest_frame = latest_frame.clone();
        let progress_callback = progress_callback.clone();
        let context = context.for_frame(frame_number);
        let context_clone = context.clone();
        let on_payload = on_payload.clone();
        context.spawn(async move {
            let input = input.clone().pull(frame_number, &context_clone).await?;
            on_payload(input, frame_number)?;

            let latest_frame = latest_frame.fetch_max(frame_number, Ordering::Relaxed);
            progress_callback(ProgressUpdate { latest_frame, total_frames });

            Ok::<(), anyhow::Error>(())
        })
    }))
    .await?;
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
    ) -> Self {
        let (tx, rx) = sync_channel::<Payload>(10);
        let context = context.clone();
        let join_handle = thread::spawn(move || {
            let mut range: Box<dyn Iterator<Item = u64>> = match input.get_caps().frame_count {
                Some(frame_count) => {
                    if do_loop {
                        Box::new((0..frame_count).cycle())
                    } else {
                        Box::new(0..frame_count)
                    }
                }
                None => Box::new(0..u64::MAX),
            };

            let mut todo = VecDeque::with_capacity(10);
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
                    if let Ok(payload) = pollster::block_on(payload) {
                        match tx.send(payload) {
                            Ok(()) => {}
                            Err(SendError(_)) => break,
                        }
                    } else {
                        break;
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
