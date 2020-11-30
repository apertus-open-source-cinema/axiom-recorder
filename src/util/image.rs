use crate::{throw, util::error::Res};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

/// The main data structure for transferring and representing single frames of
/// a video stream
#[derive(Debug)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub buffer: PackedBuffer,
}

impl Image {
    pub fn new(width: u32, height: u32, buffer: Vec<u8>, bit_depth: u8) -> Res<Self> {
        if (width * height * bit_depth as u32 / 8) > (buffer.len() as u32) {
            throw!(
                "buffer is to small (expected {}, found {})",
                width * height * bit_depth as u32 / 8,
                buffer.len()
            )
        }

        Ok(Image { width, height, buffer: PackedBuffer::new(buffer, bit_depth)? })
    }
}

#[derive(Debug)]
pub struct PackedBuffer {
    pub packed_data: Arc<Vec<u8>>,
    pub bit_depth: u8,
    u8_data: Mutex<RefCell<Option<Arc<Vec<u8>>>>>,
    u16_data: Mutex<RefCell<Option<Arc<Vec<u16>>>>>,
}

impl PackedBuffer {
    fn new(buffer: Vec<u8>, bit_depth: u8) -> Res<Self> {
        if bit_depth > 32 || bit_depth < 8 {
            throw!("bit depth must be between 8 and 32, found {}", bit_depth)
        }
        Ok(Self {
            packed_data: Arc::new(buffer),
            bit_depth,
            u8_data: Mutex::new(RefCell::new(None)),
            u16_data: Mutex::new(RefCell::new(None)),
        })
    }

    pub fn u8_buffer(&self) -> Arc<Vec<u8>> {
        let locked_u8_data = self.u8_data.lock().unwrap();
        if locked_u8_data.borrow().is_none() {
            if self.bit_depth == 8 {
                locked_u8_data.replace(Some(self.packed_data.clone()))
            } else {
                let mut iterator = self.packed_data.iter();
                let mut new_buffer =
                    Vec::with_capacity(self.packed_data.len() * 8 / self.bit_depth as usize);

                let mut rest_value: u32 = 0;
                let mut rest_bits: u32 = 0;
                loop {
                    match iterator.next() {
                        Some(value) => {
                            let bits_more_than_bit_depth =
                                (rest_bits as i32 + 8) - self.bit_depth as i32;
                            println!("rest_bits: {}, rest_value: {:032b}, value: {:08b}, bits_more_than_bit_depth: {}", rest_bits, rest_value, value, bits_more_than_bit_depth);
                            if bits_more_than_bit_depth >= 0 {
                                let new_n_bit_value: u32 = rest_value
                                    .wrapping_shl(self.bit_depth as u32 - rest_bits)
                                    | value.wrapping_shr(8 - bits_more_than_bit_depth as u32)
                                        as u32;
                                println!("new_n_bit_value: {:012b}", new_n_bit_value);
                                new_buffer.push(
                                    if self.bit_depth > 8 {
                                        new_n_bit_value.wrapping_shr(self.bit_depth as u32 - 8)
                                    } else {
                                        new_n_bit_value
                                    } as u8,
                                );
                                rest_bits = bits_more_than_bit_depth as u32;
                                rest_value = (rest_value
                                    .wrapping_shl(bits_more_than_bit_depth as u32)
                                    | (value
                                        & (2u32.pow(bits_more_than_bit_depth as u32) - 1) as u8)
                                        as u32)
                                    & (2u32.pow(rest_bits as u32) - 1)
                            } else {
                                rest_bits += 8;
                                rest_value = (rest_value << 8) | *value as u32;
                            };
                        }
                        None => break,
                    };
                }
                locked_u8_data.replace(Some(Arc::new(new_buffer)))
            };
        }

        let to_return = locked_u8_data.borrow().as_ref().unwrap().clone();
        to_return
    }

    pub fn u16_buffer(&self) -> Arc<Vec<u16>> {
        let locked_u16_data = self.u16_data.lock().unwrap();
        if locked_u16_data.borrow().is_none() {
            let mut iterator = self.packed_data.iter();
            let mut new_buffer =
                Vec::with_capacity(self.packed_data.len() * 8 / self.bit_depth as usize);

            let mut rest_value: u32 = 0;
            let mut rest_bits: u32 = 0;
            loop {
                match iterator.next() {
                    Some(value) => {
                        let bits_more_than_bit_depth =
                            (rest_bits as i32 + 8) - self.bit_depth as i32;
                        if bits_more_than_bit_depth >= 0 {
                            let new_n_bit_value: u32 = rest_value
                                .wrapping_shl(self.bit_depth as u32 - rest_bits)
                                | value.wrapping_shr(8 - bits_more_than_bit_depth as u32) as u32;
                            new_buffer.push(
                                if self.bit_depth > 16 {
                                    new_n_bit_value.wrapping_shr(self.bit_depth as u32 - 16)
                                } else {
                                    new_n_bit_value
                                } as u16,
                            );
                            rest_bits = bits_more_than_bit_depth as u32;
                            rest_value = (rest_value.wrapping_shl(bits_more_than_bit_depth as u32)
                                | (value & (2u32.pow(bits_more_than_bit_depth as u32) - 1) as u8)
                                    as u32)
                                & (2u32.pow(rest_bits as u32) - 1)
                        } else {
                            rest_bits += 8;
                            rest_value = (rest_value << 8) | *value as u32;
                        };
                    }
                    None => break,
                };
            }
            locked_u16_data.replace(Some(Arc::new(new_buffer)));
        };

        let to_return = locked_u16_data.borrow().as_ref().unwrap().clone();
        to_return
    }
}


#[cfg(test)]
mod tests {
    use crate::util::image::PackedBuffer;

    #[test]
    fn test_packed_buffer_u8() {
        let packed_buffer =
            PackedBuffer::new(vec![0b11000000, 0b0011_0000, 0b11110000], 12).unwrap();
        assert_eq!(packed_buffer.u8_buffer().len(), 2);
        assert_eq!(packed_buffer.u8_buffer()[0], 0b11000000);
        assert_eq!(packed_buffer.u8_buffer()[1], 0b00001111);
    }

    #[test]
    fn test_packed_buffer_u8_tough() {
        let packed_buffer = PackedBuffer::new(
            vec![0b10110100, 0b0011_1101, 0b10010101, 0b10110100, 0b0011_1101, 0b10010101],
            12,
        )
        .unwrap();
        assert_eq!(packed_buffer.u8_buffer().len(), 4);
        assert_eq!(packed_buffer.u8_buffer()[0], 0b10110100);
        assert_eq!(packed_buffer.u8_buffer()[1], 0b11011001);
        assert_eq!(packed_buffer.u8_buffer()[2], 0b10110100);
        assert_eq!(packed_buffer.u8_buffer()[3], 0b11011001);
    }

    #[test]
    fn test_packed_buffer_u16() {
        let packed_buffer =
            PackedBuffer::new(vec![0b11000000, 0b0011_0000, 0b11110000], 12).unwrap();
        assert_eq!(packed_buffer.u16_buffer().len(), 2);
        assert_eq!(packed_buffer.u16_buffer()[0], 0b110000000011);
        assert_eq!(packed_buffer.u16_buffer()[1], 0b000011110000);
    }
}
