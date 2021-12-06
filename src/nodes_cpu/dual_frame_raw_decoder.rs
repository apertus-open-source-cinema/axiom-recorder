use crate::pipeline_processing::{
    parametrizable::{Parameterizable, Parameters, ParametersDescriptor},
    payload::Payload,
};
use anyhow::{Context, Result};
use std::sync::Arc;

use crate::pipeline_processing::{
    frame::{CfaDescriptor, Frame, FrameInterpretation, Raw, Rgb},
    node::{Caps, ProcessingNode},
    parametrizable::{
        ParameterType,
        ParameterTypeDescriptor,
        ParameterTypeDescriptor::Optional,
        ParameterValue,
    },
    processing_context::ProcessingContext,
};
use async_trait::async_trait;

pub struct DualFrameRawDecoder {
    input: Arc<dyn ProcessingNode + Send + Sync>,
    cfa_descriptor: CfaDescriptor,
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
        })
    }
}

#[async_trait]
impl ProcessingNode for DualFrameRawDecoder {
    async fn pull(&self, frame_number: u64, context: &ProcessingContext) -> Result<Payload> {
        let frame1 = context
            .ensure_cpu_buffer::<Rgb>(&self.input.pull(frame_number * 2, context).await?)
            .context("Wrong input format")?;
        let frame2 = context
            .ensure_cpu_buffer::<Rgb>(&self.input.pull(frame_number * 2 + 1, context).await?)
            .context("Wrong input format")?;
        let interp = Raw {
            width: frame1.interp.width,
            height: frame1.interp.height,
            bit_depth: 12,
            cfa: self.cfa_descriptor,
            fps: frame1.interp.fps / 2.0,
        };

        let mut new_buffer = unsafe { context.get_uninit_cpu_buffer(interp.required_bytes()) };

        let line_bytes = interp.width as usize * 3;
        frame1.storage.as_slice(|frame1| {
            frame2.storage.as_slice(|frame2| {
                new_buffer.as_mut_slice(|new_buffer| {
                    for ((frame1_chunk, frame2_chunk), output_chunk) in frame1
                        .chunks_exact(line_bytes)
                        .zip(frame2.chunks_exact(line_bytes))
                        .zip(new_buffer.chunks_exact_mut(line_bytes * 2))
                    {
                        let mut chunks = output_chunk.chunks_exact_mut(line_bytes);
                        chunks.next().unwrap().copy_from_slice(frame1_chunk);
                        chunks.next().unwrap().copy_from_slice(frame2_chunk);
                    }
                });
            })
        });

        Ok(Payload::from(Frame { interp, storage: new_buffer }))
    }

    fn get_caps(&self) -> Caps { self.input.get_caps() }
}
