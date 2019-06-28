use crate::util::image::{Image};

pub trait Dng {
    fn format_dng(&self) -> Vec<u8>;
}

impl Dng for Image {
    fn format_dng(&self) -> Vec<u8> { unimplemented!() }
}
