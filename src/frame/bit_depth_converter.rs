use crate::{
    frame::raw_frame::RawFrame,
    pipeline_processing::{
        parametrizable::{Parameterizable, Parameters, ParametersDescriptor},
        processing_node::{Payload, ProcessingNode},
    },
};
use anyhow::{Context, Result};
use std::sync::{Arc, MutexGuard};

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
    fn process(&self, input: &mut Payload, frame_lock: MutexGuard<u64>) -> Result<Option<Payload>> {
        drop(frame_lock);
        let frame = input.downcast::<RawFrame>().context("Wrong input format")?;

        let new_frame = if frame.buffer.bit_depth() == 8 {
            frame
        } else if frame.buffer.bit_depth() == 12 {
            let mut new_buffer =
                vec![0u8; frame.buffer.bytes().len() * 8 / frame.buffer.bit_depth() as usize];
            new_buffer.chunks_mut(20000).zip(frame.buffer.bytes().chunks(30000)).for_each(
                |(macro_output_chunk, macro_input_chunk)| {
                    macro_output_chunk.chunks_mut(2).zip(macro_input_chunk.chunks(3)).for_each(
                        |(output_chunk, input_chunk)| {
                            output_chunk[0] = ((((input_chunk[0] as u16) << 4) & 0xff0)
                                | (((input_chunk[1] as u16) >> 4) | 0xf))
                                .wrapping_shr(4)
                                as u8;
                            output_chunk[1] = ((((input_chunk[1] as u16) << 8) & 0xf00)
                                | ((input_chunk[2] as u16) | 0xff))
                                .wrapping_shr(4)
                                as u8;
                        },
                    );
                },
            );

            Arc::new(RawFrame::from_bytes(new_buffer, frame.width, frame.height, 8, frame.cfa)?)
        } else {
            let mut new_buffer = Vec::with_capacity(
                frame.buffer.bytes().len() * 8 / frame.buffer.bit_depth() as usize,
            );

            let mut rest_value: u32 = 0;
            let mut rest_bits: u32 = 0;
            for value in frame.buffer.bytes().iter() {
                let bits_more_than_bit_depth =
                    (rest_bits as i32 + 8) - frame.buffer.bit_depth() as i32;
                //println!("rest_bits: {}, rest_value: {:032b}, value: {:08b},
                // bits_more_than_bit_depth: {}", rest_bits, rest_value, value,
                // bits_more_than_bit_depth);
                if bits_more_than_bit_depth >= 0 {
                    let new_n_bit_value: u32 = rest_value
                        .wrapping_shl(frame.buffer.bit_depth() as u32 - rest_bits)
                        | value.wrapping_shr(8 - bits_more_than_bit_depth as u32) as u32;
                    //println!("new_n_bit_value: {:012b}", new_n_bit_value);
                    new_buffer.push(
                        if frame.buffer.bit_depth() > 8 {
                            new_n_bit_value.wrapping_shr(frame.buffer.bit_depth() as u32 - 8)
                        } else {
                            new_n_bit_value
                        } as u8,
                    );
                    rest_bits = bits_more_than_bit_depth as u32;
                    rest_value = (value & (2u32.pow(rest_bits as u32) - 1) as u8) as u32
                } else {
                    rest_bits += 8;
                    rest_value = (rest_value << 8) | *value as u32;
                };
            }
            Arc::new(RawFrame::from_bytes(new_buffer, frame.width, frame.height, 8, frame.cfa)?)
        };

        Ok(Some(Payload::from_arc(new_frame)))
    }
}
