use crate::pipeline_processing::{
    parametrizable::{Parameterizable, Parameters, ParametersDescriptor},
    payload::Payload,
};
use anyhow::{Context, Result};
use std::sync::Arc;

use crate::{
    pipeline_processing::{
        frame::{CfaDescriptor, Frame, FrameInterpretation, Raw, Rgb},
        node::{Caps, ProcessingNode},
        parametrizable::{
            ParameterType,
            ParameterTypeDescriptor,
            ParameterTypeDescriptor::Optional,
            ParameterValue,
        },
        processing_context::ProcessingContext,
    },
    util::async_notifier::AsyncNotifier,
};
use async_trait::async_trait;
use futures::try_join;

pub struct DualFrameRawDecoder {
    input: Arc<dyn ProcessingNode + Send + Sync>,
    cfa_descriptor: CfaDescriptor,
    last_frame_info: AsyncNotifier<(u64, u64)>,
}
impl Parameterizable for DualFrameRawDecoder {
    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new()
            .with("input", ParameterTypeDescriptor::Mandatory(ParameterType::NodeInput))
            .with(
                "first-red-x",
                Optional(ParameterType::BoolParameter, ParameterValue::BoolParameter(true)),
            )
            .with(
                "first-red-y",
                Optional(ParameterType::BoolParameter, ParameterValue::BoolParameter(true)),
            )
    }

    fn from_parameters(parameters: &Parameters, _context: &ProcessingContext) -> Result<Self> {
        Ok(Self {
            input: parameters.get("input")?,
            cfa_descriptor: CfaDescriptor {
                first_is_red_x: parameters.get("first-red-x")?,
                first_is_red_y: parameters.get("first-red-y")?,
            },
            last_frame_info: Default::default(),
        })
    }
}

#[async_trait]
impl ProcessingNode for DualFrameRawDecoder {
    async fn pull(&self, frame_number: u64, context: &ProcessingContext) -> Result<Payload> {
        self.last_frame_info.wait(move |(next, _)| *next == frame_number).await;
        let (_, next_even) = self.last_frame_info.get();

        let frames = try_join!(
            self.input.pull(next_even, context),
            self.input.pull(next_even + 1, context)
        )?;
        let frames_array = [frames.0, frames.1];
        let mut frames_collected = frames_array
            .iter()
            .map(|x| context.ensure_cpu_buffer::<Rgb>(&x).context("Wrong input format"));

        let mut frame_a = frames_collected.next().unwrap()?;
        let mut frame_b = frames_collected.next().unwrap()?;

        let is_correct = frame_a.storage.as_slice(|frame_a| frame_a[0] % 2 == 1);

        let next_next_even = if !is_correct {
            // we slip one frame
            println!("frame slipped in DualFrameRawDecoder");
            frame_a = frame_b;
            frame_b = context
                .ensure_cpu_buffer::<Rgb>(&self.input.pull(next_even + 2, &context).await?)
                .context("Wrong input format")?;
            next_even + 3
        } else {
            next_even + 2
        };
        self.last_frame_info.update(move |(next, next_even)| {
            *next = frame_number + 1;
            *next_even = next_next_even;
        });

        let interp = Raw {
            width: frame_a.interp.width * 2,
            height: frame_a.interp.height * 2,
            bit_depth: 12,
            cfa: self.cfa_descriptor,
            fps: frame_a.interp.fps / 2.0,
        };

        let mut new_buffer = unsafe { context.get_uninit_cpu_buffer(interp.required_bytes()) };

        let line_bytes = frame_a.interp.width as usize * 3;
        frame_a.storage.as_slice(|frame_a| {
            frame_b.storage.as_slice(|frame_b| {
                new_buffer.as_mut_slice(|new_buffer| {
                    for ((frame_a_chunk, frame_b_chunk), output_chunk) in frame_a
                        .chunks_exact(line_bytes)
                        .zip(frame_b.chunks_exact(line_bytes))
                        .zip(new_buffer.chunks_exact_mut(line_bytes * 2))
                    {
                        let mut chunks = output_chunk.chunks_exact_mut(line_bytes);
                        chunks.next().unwrap().copy_from_slice(frame_b_chunk);
                        chunks.next().unwrap().copy_from_slice(frame_a_chunk);
                    }
                });
            })
        });

        Ok(Payload::from(Frame { interp, storage: new_buffer }))
    }

    fn get_caps(&self) -> Caps {
        let upstream = self.input.get_caps();
        Caps { frame_count: upstream.frame_count.map(|x| x / 2), ..upstream }
    }
}