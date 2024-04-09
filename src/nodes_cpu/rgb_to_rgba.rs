use crate::pipeline_processing::{
    node::InputProcessingNode,
    parametrizable::{Parameterizable, Parameters, ParametersDescriptor},
    payload::Payload,
};
use anyhow::{Context, Result};


use crate::pipeline_processing::{
    frame::{Frame, FrameInterpretation, Raw},
    node::{Caps, NodeID, ProcessingNode, Request},
    parametrizable::prelude::*,
    processing_context::ProcessingContext,
};
use async_trait::async_trait;

pub struct RgbToRgbaToFlutter {
    input: InputProcessingNode,
    context: ProcessingContext,
    sink: Stream
}
impl Parameterizable for BitDepthConverter {
    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new().with("input", Mandatory(NodeInputParameter))
    }

    fn from_parameters(
        mut parameters: Parameters,
        _is_input_to: &[NodeID],
        context: &ProcessingContext,
    ) -> Result<Self> {
        Ok(Self { input: parameters.take("input")?, context: context.clone() })
    }
}

#[async_trait]
impl ProcessingNode for BitDepthConverter {
    async fn pull(&self, request: Request) -> Result<Payload> {
        let frame = self.input.pull(request).await?;
        let frame = processing_context.ensure_cpu_buffer::<Rgb>(&frame).unwrap();
        let mut rgba_buffer = vec![0u8; (frame.interp.width * frame.interp.height * 4) as usize];

        let interp = Rgba { width: frame.interp.width, height: frame.interp.height, fps: frame.interp.fps };
        let mut new_buffer = unsafe { self.context.get_uninit_cpu_buffer(interp.required_bytes()) };

        frame.storage.as_slice(|frame| {
            new_buffer.storage.as_slice_mut(|frame| {
            for (src, dest) in frame.chunks_exact(3).zip(rgba_buffer.chunks_exact_mut(4)) {
                dest[0] = src[0];
                dest[1] = src[1];
                dest[2] = src[2];
                dest[3] = 255;

            }
        });


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

    fn get_caps(&self) -> Caps { self.input.get_caps() }
}
