pub trait ToAny: 'static {
    fn as_any(&self) -> &dyn std::any::Any;
}
impl<T: 'static> ToAny for T {
    fn as_any(&self) -> &dyn std::any::Any { self }
}

/// The main data structure for transferring and representing single raw frames
/// of a video stream
pub struct Frame<Storage> {
    pub interpretation: FrameInterpretation,
    pub storage: Storage,
}

#[derive(Copy, Clone, Debug)]
pub struct FrameInterpretation {
    pub width: u64,
    pub height: u64,
    pub fps: Option<f64>,

    pub color_interpretation: ColorInterpretation,
    pub sample_interpretation: SampleInterpretation,
    pub compression: Compression,
}
impl FrameInterpretation {
    pub fn required_bytes(&self) -> usize {
        match self.compression {
            Compression::Uncompressed => self.required_bytes_uncompressed(),
            Compression::SZ3Compressed { size } => size,
        }
    }
    fn required_bytes_uncompressed(&self) -> usize {
        (self.width
            * self.height
            * self.color_interpretation.samples_per_pixel()
            * self.sample_interpretation.bits_per_sample()) as usize
            / 8
    }
}

#[derive(Clone, Copy, Debug)]
pub enum ColorInterpretation {
    Bayer(CfaDescriptor),
    Rgb,
    Rgba,
}
impl ColorInterpretation {
    pub fn samples_per_pixel(&self) -> u64 {
        match self {
            ColorInterpretation::Bayer(_) => 1,
            ColorInterpretation::Rgb => 3,
            ColorInterpretation::Rgba => 4,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct CfaDescriptor {
    pub red_in_first_col: bool,
    pub red_in_first_row: bool,
}
impl CfaDescriptor {
    pub fn from_first_red(red_in_first_col: bool, red_in_first_row: bool) -> Self {
        CfaDescriptor { red_in_first_col, red_in_first_row }
    }
}

#[derive(Copy, Clone, Debug)]
pub enum SampleInterpretation {
    UInt(u8),
    FP16,
    FP32,
}
impl SampleInterpretation {
    pub fn bits_per_sample(&self) -> u64 {
        match self {
            SampleInterpretation::UInt(bits) => *bits as _,
            SampleInterpretation::FP16 => 16,
            SampleInterpretation::FP32 => 32,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub enum Compression {
    Uncompressed,
    SZ3Compressed { size: usize },
}
