use anyhow::{anyhow, Result};
use bytemuck::cast_slice;
use rayon::{
    iter::{IndexedParallelIterator, ParallelIterator},
    slice::{ParallelSlice, ParallelSliceMut},
};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct Buffer {
    bytes: Arc<Vec<u8>>,
    bit_depth: u64,
}
impl Buffer {
    pub fn new(buffer: Vec<u8>, bit_depth: u64) -> Result<Self> {
        match bit_depth {
            8..=32 => Ok(Self { bytes: Arc::new(buffer), bit_depth }),
            _ => Err(anyhow!("bit depth must be between 8 and 32, found {}", bit_depth)),
        }
    }
    pub fn bytes(&self) -> Arc<&[u8]> { Arc::new(cast_slice(&self.bytes)) }
    pub fn bit_depth(&self) -> u64 { self.bit_depth }

    pub fn unpacked_u8(&self) -> Result<Arc<&[u8]>> {
        match self.bit_depth {
            8 => Ok(Arc::new(cast_slice(&self.bytes))),
            _ => Err(anyhow!("A Buffer with bit_depth=8 is required! Try to repack the data.")),
        }
    }
    pub fn unpacked_u16(&self) -> Result<Arc<&[u16]>> {
        match self.bit_depth {
            16 => Ok(Arc::new(cast_slice(&self.bytes))),
            _ => Err(anyhow!("A Buffer with bit_depth=8 is required! Try to repack the data.")),
        }
    }

    pub fn repack_to_8_bit(&self) -> Buffer {
        if self.bit_depth() == 8 {
            self.clone()
        } else if self.bit_depth() == 12 {
            let mut new_buffer = vec![0u8; self.bytes().len() * 8 / self.bit_depth() as usize];
            new_buffer.par_chunks_mut(20000).zip(self.bytes().par_chunks(30000)).for_each(
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

            Self::new(new_buffer, 8).unwrap()
        } else {
            let mut new_buffer =
                Vec::with_capacity(self.bytes().len() * 8 / self.bit_depth() as usize);

            let mut rest_value: u32 = 0;
            let mut rest_bits: u32 = 0;
            for value in self.bytes().iter() {
                let bits_more_than_bit_depth = (rest_bits as i32 + 8) - self.bit_depth() as i32;
                //println!("rest_bits: {}, rest_value: {:032b}, value: {:08b},
                // bits_more_than_bit_depth: {}", rest_bits, rest_value, value,
                // bits_more_than_bit_depth);
                if bits_more_than_bit_depth >= 0 {
                    let new_n_bit_value: u32 = rest_value
                        .wrapping_shl(self.bit_depth() as u32 - rest_bits)
                        | value.wrapping_shr(8 - bits_more_than_bit_depth as u32) as u32;
                    //println!("new_n_bit_value: {:012b}", new_n_bit_value);
                    new_buffer.push(
                        if self.bit_depth() > 8 {
                            new_n_bit_value.wrapping_shr(self.bit_depth() as u32 - 8)
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
            Self::new(new_buffer, 8).unwrap()
        }
    }
}
