use std::sync::Arc;

pub trait ToAny: 'static {
    fn as_any(&self) -> &dyn std::any::Any;
}
impl<T: 'static> ToAny for T {
    fn as_any(&self) -> &dyn std::any::Any { self }
}


pub trait FrameInterpretation: ToAny {
    fn required_bytes(&self) -> usize;
    fn width(&self) -> u64;
    fn height(&self) -> u64;
    fn fps(&self) -> Option<f64>;
}

/// The main data structure for transferring and representing single raw frames
/// of a video stream
pub struct Frame<Interpretation, Storage> {
    pub interp: Interpretation,
    pub storage: Storage,
}

#[derive(Debug, Copy, Clone)]
pub struct CfaDescriptor {
    pub red_in_first_col: bool,
    pub red_in_first_row: bool,
}

impl CfaDescriptor {
    pub fn from_first_red(red_in_first_col: bool, red_in_first_row: bool) -> Self {
        CfaDescriptor { red_in_first_col, red_in_first_row }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Raw {
    pub width: u64,
    pub height: u64,
    pub bit_depth: u64,
    pub cfa: CfaDescriptor,
    pub fps: f64,
}

impl FrameInterpretation for Raw {
    fn required_bytes(&self) -> usize {
        self.width as usize * self.height as usize * self.bit_depth as usize / 8
    }
    fn width(&self) -> u64 { self.width }
    fn height(&self) -> u64 { self.height }
    fn fps(&self) -> Option<f64> { Some(self.fps) }
}

#[derive(Clone, Copy, Debug)]
pub struct Rgb {
    pub width: u64,
    pub height: u64,
    pub fps: f64,
}

impl FrameInterpretation for Rgb {
    fn required_bytes(&self) -> usize { self.width as usize * self.height as usize * 3 }
    fn width(&self) -> u64 { self.width }
    fn height(&self) -> u64 { self.height }
    fn fps(&self) -> Option<f64> { Some(self.fps) }
}

#[derive(Clone, Copy, Debug)]
pub struct Rgba {
    pub width: u64,
    pub height: u64,
    pub fps: f64,
}

impl FrameInterpretation for Rgba {
    fn required_bytes(&self) -> usize { self.width as usize * self.height as usize * 4 }
    fn width(&self) -> u64 { self.width }
    fn height(&self) -> u64 { self.height }
    fn fps(&self) -> Option<f64> { Some(self.fps) }
}


#[derive(Clone, Copy, Debug)]
pub enum FrameInterpretations {
    Raw(Raw),
    Rgb(Rgb),
    Rgba(Rgba),
}
impl FrameInterpretation for FrameInterpretations {
    fn required_bytes(&self) -> usize {
        match self {
            FrameInterpretations::Raw(interp) => interp.required_bytes(),
            FrameInterpretations::Rgb(interp) => interp.required_bytes(),
            FrameInterpretations::Rgba(interp) => interp.required_bytes(),
        }
    }
    fn width(&self) -> u64 {
        match self {
            FrameInterpretations::Raw(interp) => interp.width(),
            FrameInterpretations::Rgb(interp) => interp.width(),
            FrameInterpretations::Rgba(interp) => interp.width(),
        }
    }
    fn height(&self) -> u64 {
        match self {
            FrameInterpretations::Raw(interp) => interp.height(),
            FrameInterpretations::Rgb(interp) => interp.height(),
            FrameInterpretations::Rgba(interp) => interp.height(),
        }
    }
    fn fps(&self) -> Option<f64> {
        match self {
            FrameInterpretations::Raw(interp) => interp.fps(),
            FrameInterpretations::Rgb(interp) => interp.fps(),
            FrameInterpretations::Rgba(interp) => interp.fps(),
        }
    }
}
