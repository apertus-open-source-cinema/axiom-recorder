use crate::{
    pipeline_processing::{
        execute::ProcessingStageLockWaiter,
        parametrizable::{Parameterizable, Parameters, ParametersDescriptor},
        payload::Payload,
        processing_node::ProcessingNode,
    },
};
use anyhow::{Context, Result};
use vulkano::buffer::CpuAccessibleBuffer;
use std::sync::Arc;
use crate::frame::{CpuStorage, Frame, Raw};

pub struct BitDepthConverter();
impl Parameterizable for BitDepthConverter {
    fn describe_parameters() -> ParametersDescriptor { ParametersDescriptor::new() }

    fn from_parameters(_parameters: &Parameters) -> Result<Self>
    where
        Self: Sized,
    {
        Ok(Self())
    }
}
impl ProcessingNode for BitDepthConverter {
    fn process(
        &self,
        input: &mut Payload,
        _frame_lock: ProcessingStageLockWaiter,
    ) -> Result<Option<Payload>> {
        let frame = input.downcast::<Frame<Raw, CpuStorage>>().context("Wrong input format")?;
        let mut new_buffer = unsafe { CpuStorage::uninit((frame.interp.width * frame.interp.height) as usize) };

        if frame.interp.bit_depth == 8 {
            return Ok(Some(input.clone()))
        } else if frame.interp.bit_depth == 12 {
            new_buffer.as_mut_slice(|new_buffer| {
                frame.storage.as_slice(|frame_storage| {
                    new_buffer.chunks_mut(200000).zip(frame_storage.chunks(300000)).for_each(
                        |(macro_output_chunk, macro_input_chunk)| {
                            macro_output_chunk.chunks_mut(2).zip(macro_input_chunk.chunks(3)).for_each(
                                |(output_chunk, input_chunk)| {
                                    output_chunk[0] = ((((input_chunk[0] as u16) << 4) & 0xff0)
                                        | (((input_chunk[1] as u16) >> 4) & 0xf))
                                        .wrapping_shr(4)
                                        as u8;
                                    output_chunk[1] = ((((input_chunk[1] as u16) << 8) & 0xf00)
                                        | ((input_chunk[2] as u16) & 0xff))
                                        .wrapping_shr(4)
                                        as u8;
                                },
                            );
                        },
                    );
                })
            });
        } else {
            let mut rest_value: u32 = 0;
            let mut rest_bits: u32 = 0;
            let mut pos = 0;
            new_buffer.as_mut_slice(|new_buffer| {
                frame.storage.as_slice(|frame_storage| {
                    for value in frame_storage.iter() {
                        let bits_more_than_bit_depth = (rest_bits as i32 + 8) - frame.interp.bit_depth as i32;
                        //println!("rest_bits: {}, rest_value: {:032b}, value: {:08b},
                        // bits_more_than_bit_depth: {}", rest_bits, rest_value, value,
                        // bits_more_than_bit_depth);
                        if bits_more_than_bit_depth >= 0 {
                            let new_n_bit_value: u32 = rest_value
                                .wrapping_shl(frame.interp.bit_depth as u32 - rest_bits)
                                | value.wrapping_shr(8 - bits_more_than_bit_depth as u32) as u32;
                            //println!("new_n_bit_value: {:012b}", new_n_bit_value);
                            new_buffer[pos] = (
                                if frame.interp.bit_depth > 8 {
                                    new_n_bit_value.wrapping_shr(frame.interp.bit_depth as u32 - 8)
                                } else {
                                    new_n_bit_value
                                } as u8
                            );
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

        let new_frame = Frame {
            storage: new_buffer,
            interp: Raw {
                bit_depth: 8,
                ..frame.interp
            }
        };

        Ok(Some(Payload::from(new_frame)))
    }
}
