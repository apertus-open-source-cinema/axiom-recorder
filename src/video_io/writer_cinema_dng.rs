use crate::pipeline_processing::{
    parametrizable::{
        ParameterType::{FloatRange, StringParameter},
        ParameterTypeDescriptor::Mandatory,
        Parameterizable,
        Parameters,
        ParametersDescriptor,
    },
    processing_node::{Payload, ProcessingNode},
};
use anyhow::{Context, Result};

use crate::frame::raw_frame::RawFrame;
use std::{
    fs::create_dir,
    sync::atomic::{AtomicU64, Ordering},
};
use tiff_encoder::{
    ifd::{tags, Ifd},
    write::ByteBlock,
    TiffFile,
    ASCII,
    BYTE,
    LONG,
    RATIONAL,
    SHORT,
    SRATIONAL,
};

/// A writer, that writes cinemaDNG (a folder with DNG files)
pub struct CinemaDngWriter {
    dir_path: String,
    fps: f64,
    frame_number: AtomicU64,
}

impl Parameterizable for CinemaDngWriter {
    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new()
            .with("path", Mandatory(StringParameter))
            .with("fps", Mandatory(FloatRange(0., f64::MAX)))
    }

    fn from_parameters(parameters: &Parameters) -> Result<Self>
    where
        Self: Sized,
    {
        let filename = parameters.get("path")?;
        create_dir(&filename).context("Error while creating target directory")?;
        Ok(Self {
            dir_path: filename,
            fps: parameters.get("fps")?,
            frame_number: AtomicU64::new(0),
        })
    }
}

impl ProcessingNode for CinemaDngWriter {
    fn process(&self, input: &mut Payload) -> Result<Option<Payload>> {
        let frame = input.downcast::<RawFrame>().context("Wrong input format")?;
        let current_frame_number = self.frame_number.fetch_add(1, Ordering::SeqCst);

        let cfa_pattern = match (frame.cfa.first_is_red_x, frame.cfa.first_is_red_y) {
            (true, true) => BYTE![0, 1, 1, 2],
            (true, false) => BYTE![1, 0, 2, 1],
            (false, true) => BYTE![1, 2, 0, 1],
            (false, false) => BYTE![2, 1, 1, 0],
        };

        TiffFile::new(
            Ifd::new()
                .with_entry(50706, BYTE![1, 4, 0, 0])  // DNG version
                .with_entry(tags::Compression, SHORT![1]) // No compression
                .with_entry(tags::SamplesPerPixel, SHORT![1])
                .with_entry(tags::NewSubfileType, LONG![0])
                .with_entry(tags::XResolution, RATIONAL![(1, 1)])
                .with_entry(tags::YResolution, RATIONAL![(1, 1)])
                .with_entry(tags::ResolutionUnit, SHORT!(1))
                .with_entry(tags::FillOrder, SHORT![1])
                .with_entry(tags::Orientation, SHORT![1])
                .with_entry(tags::PlanarConfiguration, SHORT![1])

                .with_entry(tags::Make, ASCII!["Apertus"])
                .with_entry(tags::Model, ASCII!["AXIOM"])
                .with_entry(50708, ASCII!("Apertus AXIOM")) // unique camera model
                .with_entry(tags::Software, ASCII!["axiom-recorder"])

                .with_entry(tags::PhotometricInterpretation, SHORT![32803]) // Black is zero
                .with_entry(33421, SHORT![2, 2]) // CFARepeatPatternDim
                .with_entry(33422, cfa_pattern) // CFAPattern (R=0, G=1, B=2)

                // color matrix from https://github.com/apertus-open-source-cinema/misc-tools-utilities/blob/8c8e9fca96b4b3fec50756fd7a72be6ea5c7b77c/raw2dng/raw2dng.c#L46-L49
                .with_entry(50721, SRATIONAL![  // ColorMatrix1
                        (11038, 10000), (3184, 10000), (1009, 10000),
                        (3284, 10000), (11499, 10000), (1737, 10000),
                        (1283, 10000), (3550, 10000), (5967, 10000)
               ])

                .with_entry(51044, SRATIONAL![((self.fps * 10000.0) as i32, 10000)])// FrameRate

                .with_entry(tags::ImageLength, LONG![frame.height as u32])
                .with_entry(tags::ImageWidth, LONG![frame.width as u32])
                .with_entry(tags::RowsPerStrip, LONG![frame.height as u32])
                .with_entry(tags::StripByteCounts, LONG![frame.buffer.bytes().len() as u32])
                .with_entry(tags::BitsPerSample, SHORT![frame.buffer.bit_depth() as u16])
                .with_entry(tags::StripOffsets, ByteBlock::single(frame.buffer.bytes().to_vec()))
                .single(),
        )
        .write_to(format!("{}/{:06}.dng", &self.dir_path, current_frame_number))?;
        Ok(Some(Payload::empty()))
    }
}
