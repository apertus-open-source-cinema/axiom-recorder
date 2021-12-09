use crate::{
    pipeline_processing::{
        frame::{Frame, FrameInterpretation, Rgb},
        node::{Caps, ProcessingNode},
        parametrizable::{
            ParameterType::{IntRange},
            ParameterTypeDescriptor::{Optional},
            ParameterValue,
            Parameterizable,
            Parameters,
            ParametersDescriptor,
        },
        payload::Payload,
        processing_context::ProcessingContext,
    },
    util::async_notifier::AsyncNotifier,
};
use anyhow::{anyhow, Result};
use async_trait::async_trait;

use nokhwa::{Camera};
use std::{
    collections::VecDeque,
    sync::{
        Arc,
        Mutex,
    },
    thread,
};

pub struct WebcamInput {
    queue: Arc<Mutex<VecDeque<(u64, Payload)>>>,
    last_frame: AsyncNotifier<u64>,
}
impl Parameterizable for WebcamInput {
    const DESCRIPTION: Option<&'static str> =
        Some("read frames from a webcam (or webcam like source like a frame-grabber)");

    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new()
            .with("device", Optional(IntRange(0, i64::MAX), ParameterValue::IntRange(0)))
    }
    fn from_parameters(options: &Parameters, context: &ProcessingContext) -> anyhow::Result<Self> {
        let mut camera = Camera::new(options.get::<u64>("device")? as usize, None)?;
        let queue = Arc::new(Mutex::new(VecDeque::new()));
        let last_frame = AsyncNotifier::new(0);
        let interp = Rgb {
            width: camera.resolution().width() as u64,
            height: camera.resolution().height() as u64,
            fps: camera.frame_rate() as f64,
        };
        let context = context.clone();
        let queue_clone = queue.clone();
        let last_frame_clone = last_frame.clone();
        camera.open_stream()?;
        thread::spawn(move || loop {
            let mut buffer = unsafe { context.get_uninit_cpu_buffer(interp.required_bytes()) };
            buffer.as_mut_slice(|buffer| camera.frame_to_buffer(buffer, false)).unwrap();
            last_frame_clone.update(|old| old + 1);
            queue_clone.lock().unwrap().push_back((
                last_frame_clone.get(),
                Payload::from(Frame { storage: buffer, interp: interp.clone() }),
            ));
        });

        Ok(Self { queue, last_frame })
    }
}

#[async_trait]
impl ProcessingNode for WebcamInput {
    async fn pull(&self, frame_number: u64, _context: &ProcessingContext) -> Result<Payload> {
        let mut lock = self.queue.lock().unwrap();
        let base_index = lock[0].0;
        if frame_number < base_index {
            Err(anyhow!(
                "the frame {} is not available anymore. your access pattern is probably bad"
            ))?;
        } else {
            self.last_frame.wait(move |n| *n >= frame_number);
        }

        let payload = lock
            .iter()
            .find(|(n, _)| *n == frame_number)
            .ok_or(anyhow!("Frame {} is not present anymore in webcam input buffer", frame_number))?
            .1
            .clone();
        lock.retain(|(n, _)| *n < frame_number);

        Ok(payload)
    }

    fn get_caps(&self) -> Caps { Caps { frame_count: None, is_live: true } }
}
