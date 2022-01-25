pub trait FrameInterpretation {
    fn required_bytes(&self) -> usize;
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
}

#[derive(Clone, Copy, Debug)]
pub struct Rgb {
    pub width: u64,
    pub height: u64,
    pub fps: f64,
}

impl FrameInterpretation for Rgb {
    fn required_bytes(&self) -> usize { self.width as usize * self.height as usize * 3 }
}

#[derive(Clone, Copy, Debug)]
pub struct Rgba {
    pub width: u64,
    pub height: u64,
    pub fps: f64,
}

impl FrameInterpretation for Rgba {
    fn required_bytes(&self) -> usize { self.width as usize * self.height as usize * 3 }
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
}
