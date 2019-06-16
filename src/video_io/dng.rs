pub trait Dng {
    fn format_dng(&self) -> vec<u8>;
}

impl Dng for Image {
    fn format_dng(&self) -> _ {
        unimplemented!()
    }
}