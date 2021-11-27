use crate::pipeline_processing::{
    buffers::CpuBuffer,
    parametrizable::{Parameterizable, Parameters, ParametersDescriptor},
    payload::Payload,
};
use anyhow::{Context, Result};

use crate::pipeline_processing::{
    frame::{Frame, Raw},
    processing_context::ProcessingContext,
};
use crate::pipeline_processing_legacy::execute::ProcessingStageLockWaiter;
use crate::pipeline_processing_legacy::processing_node::ProcessingNode;

pub struct BitDepthConverter {
    context: ProcessingContext,
}
impl Parameterizable for BitDepthConverter {
    fn describe_parameters() -> ParametersDescriptor { ParametersDescriptor::new() }

    fn from_parameters(_parameters: &Parameters, context: ProcessingContext) -> Result<Self>
    where
        Self: Sized,
    {
        Ok(Self { context })
    }
}
impl ProcessingNode for BitDepthConverter {
    fn process(
        &self,
        input: &mut Payload,
        _frame_lock: ProcessingStageLockWaiter,
    ) -> Result<Option<Payload>> {
        let frame = input.downcast::<Frame<Raw, CpuBuffer>>().context("Wrong input format")?;
        let mut new_buffer = unsafe {
            self.context.get_uninit_cpu_buffer((frame.interp.width * frame.interp.height) as usize)
        };

        if frame.interp.bit_depth == 8 {
            return Ok(Some(input.clone()));
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

        let new_frame = Frame { storage: new_buffer, interp: Raw { bit_depth: 8, ..frame.interp } };

        Ok(Some(Payload::from(new_frame)))
    }
}
