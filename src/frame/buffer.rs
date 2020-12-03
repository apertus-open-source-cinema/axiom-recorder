use std::sync::{Arc};
use rayon::slice::{ParallelSliceMut, ParallelSlice};
use rayon::iter::{IndexedParallelIterator, ParallelIterator};
use anyhow::{Result, anyhow};
use crate::frame::buffer::Buffer::U8Buffer;

#[derive(Debug, Clone)]
pub enum Buffer {
    PackedBuffer{ bytes: Arc<Vec<u8>>, bit_depth: u64},
    U8Buffer { bytes: Arc<Vec<u8>> },
    U16Buffer { bytes: Arc<Vec<u8>>, words: Arc<Vec<u16>> }
}
impl Buffer {
    pub fn new(buffer: Vec<u8>, bit_depth: u64) -> Result<Self> {
        match bit_depth {
            8 => Ok(Self::U8Buffer { bytes: Arc::new(buffer) }),
            16 => Ok({
                let words: Vec<u16> = (&buffer)
                    .chunks_exact(2)
                    .into_iter()
                    .map(|a| u16::from_ne_bytes([a[0], a[1]]))
                    .collect();
                Self::U16Buffer { bytes: Arc::new(buffer), words: Arc::new(words) }
            }),
            8..=32 => Ok(Self::PackedBuffer { bytes: Arc::new(buffer), bit_depth }),
            _ => Err(anyhow!("bit depth must be between 8 and 32, found {}", bit_depth)),
        }
    }
    pub fn bytes(&self) -> Arc<Vec<u8>> {
        match self {
            Self::PackedBuffer { bytes, bit_depth } => bytes.clone(),
            Self::U8Buffer { bytes } => bytes.clone(),
            Self::U16Buffer { bytes, words } => bytes.clone(),
        }
    }
    pub fn bit_depth(&self) -> u64 {
        match self {
            Self::PackedBuffer { bytes, bit_depth } => *bit_depth,
            Self::U8Buffer { bytes } => 8,
            Self::U16Buffer { bytes, words } => 16,
        }
    }

    pub fn unpacked_u8(&self) -> Result<Arc<Vec<u8>>> {
        match self {
            Self::U8Buffer {bytes} => Ok(bytes.clone()),
            _ => Err(anyhow!("A U8Buffer is needed! Try to convert explicitly to U8Buffer"))
        }
    }
    pub fn unpack_u8(&self) -> Buffer {
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

            U8Buffer {bytes: Arc::new(new_buffer)}
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
            U8Buffer {bytes: Arc::new(new_buffer)}
        }
    }
}


#[cfg(test)]
mod tests {
    use crate::frame::buffer::{PackedBuffer, U8Buffer};

    #[test]
    fn test_packed_buffer_u8() {
        let packed_buffer = PackedBuffer::new(vec![0b11000000, 0b0011_0000, 0b11110000], 12).unwrap();
        let u8_buffer = U8Buffer::from_packed_buffer(packed_buffer);
        assert_eq!(u8_buffer.u8_vec().len(), 2);
        assert_eq!(u8_buffer.u8_vec()[0], 0b11000000);
        assert_eq!(u8_buffer.u8_vec()[1], 0b00001111);
    }

    #[test]
    fn test_packed_buffer_u8_tough() {
        let packed_buffer = PackedBuffer::new(
            vec![0b10110100, 0b0011_1101, 0b10010101, 0b10110100, 0b0011_1101, 0b10010101],
            12,
        ).unwrap();
        let u8_buffer = U8Buffer::from_packed_buffer(packed_buffer);
        assert_eq!(u8_buffer.u8_vec().len(), 4);
        assert_eq!(u8_buffer.u8_vec()[0], 0b10110100);
        assert_eq!(u8_buffer.u8_vec()[1], 0b11011001);
        assert_eq!(u8_buffer.u8_vec()[2], 0b10110100);
        assert_eq!(u8_buffer.u8_vec()[3], 0b11011001);
    }
}
