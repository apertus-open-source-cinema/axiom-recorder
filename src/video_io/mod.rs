pub mod source;
pub mod writer;

/// The main data structure for transferring and representing single frames of
/// a video stream
#[derive(Debug, Clone)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub bit_depth: u8,

    pub data: Vec<u8>,
}
