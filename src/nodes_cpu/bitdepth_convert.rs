use crate::pipeline_processing::{
    parametrizable::{Parameterizable, Parameters, ParametersDescriptor},
    payload::Payload,
};
use anyhow::{Context, Result};
use std::sync::Arc;

use crate::pipeline_processing::{
    frame::{Frame, FrameInterpretation, Raw},
    node::{Caps, ProcessingNode},
    parametrizable::{ParameterType, ParameterTypeDescriptor},
    processing_context::ProcessingContext,
};
use async_trait::async_trait;

pub struct BitDepthConverter {
    input: Arc<dyn ProcessingNode + Send + Sync>,
}
impl Parameterizable for BitDepthConverter {
    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new()
            .with("input", ParameterTypeDescriptor::Mandatory(ParameterType::NodeInput))
    }

    fn from_parameters(parameters: &Parameters, _context: &ProcessingContext) -> Result<Self> {
        Ok(Self { input: parameters.get("input")? })
    }
}

#[async_trait]
impl ProcessingNode for BitDepthConverter {
    async fn pull(&self, frame_number: u64, context: &ProcessingContext) -> Result<Payload> {
        let input = self.input.pull(frame_number, context).await?;
        let frame = context.ensure_cpu_buffer::<Raw>(&input).context("Wrong input format")?;
        let interp = Raw { bit_depth: 8, ..frame.interp };
        let mut new_buffer = unsafe { context.get_uninit_cpu_buffer(interp.required_bytes()) };

        if frame.interp.bit_depth == 8 {
            return Ok(input);
        } else if frame.interp.bit_depth == 12 {
            new_buffer.as_mut_slice(|new_buffer| {
                frame.storage.as_slice(|frame_storage| {
                    for (input, output) in
                        frame_storage.chunks_exact(3).zip(new_buffer.chunks_exact_mut(2))
                    {
                        output[0] = input[0];
                        output[1] = (input[1] << 4) | (input[2] >> 4);
                    }
                })
            });
        } else {
            let mut rest_value: u32 = 0;
            let mut rest_bits: u32 = 0;
            let mut pos = 0;
            new_buffer.as_mut_slice(|new_buffer| {
                frame.storage.as_slice(|frame_storage| {
                    for value in frame_storage.iter() {
                        let bits_more_than_bit_depth =
                            (rest_bits as i32 + 8) - frame.interp.bit_depth as i32;
                        if bits_more_than_bit_depth >= 0 {
                            let new_n_bit_value: u32 = rest_value
                                .wrapping_shl(frame.interp.bit_depth as u32 - rest_bits)
                                | value.wrapping_shr(8 - bits_more_than_bit_depth as u32) as u32;
                            new_buffer[pos] = (if frame.interp.bit_depth > 8 {
                                new_n_bit_value.wrapping_shr(frame.interp.bit_depth as u32 - 8)
                            } else {
                                new_n_bit_value
                            } as u8);
                            pos += 1;

                            rest_bits = bits_more_than_bit_depth as u32;
                            rest_value = (value & (2u32.pow(rest_bits as u32) - 1) as u8) as u32
                        } else {
                            rest_bits += 8;
                            rest_value = (rest_value << 8) | *value as u32;
                        };
                    }
                })
            });
        }

        let new_frame = Frame { storage: new_buffer, interp };

        Ok(Payload::from(new_frame))
    }

    fn get_caps(&self) -> Caps {
        self.input.get_caps()
    }
}
