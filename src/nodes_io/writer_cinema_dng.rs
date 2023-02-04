use crate::pipeline_processing::{
    buffers::CpuBuffer,
    frame::{ColorInterpretation, Frame, SampleInterpretation},
    node::{InputProcessingNode, NodeID, ProgressUpdate, SinkNode},
    parametrizable::prelude::*,
    processing_context::ProcessingContext,
    puller::pull_unordered,
};
use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use dng::{
    ifd::{Ifd, IfdValue, Offsets},
    tags,
    tags::IfdType,
    yaml::IfdYamlParser,
    DngWriter,
    FileType,
};
use std::{
    fs,
    fs::{create_dir, File},
    io::Write,
    path::PathBuf,
    str::FromStr,
    sync::Arc,
};


/// A writer, that writes cinemaDNG (a folder with DNG files)
pub struct CinemaDngWriter {
    dir_path: String,
    input: InputProcessingNode,
    number_of_frames: Option<u64>,
    priority: u8,
    base_ifd: Ifd,
}

impl Parameterizable for CinemaDngWriter {
    const DESCRIPTION: Option<&'static str> = Some("writes Cinema DNG files into a directory");

    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new()
            .with("input", Mandatory(NodeInputParameter))
            .with("path", Mandatory(StringParameter))
            .with("priority", WithDefault(U8(), IntRangeValue(0)))
            .with("number-of-frames", Optional(NaturalGreaterZero()))
            .with("dcp-yaml", Optional(StringParameter))
    }

    fn from_parameters(
        mut parameters: Parameters,
        _is_input_to: &[NodeID],
        _context: &ProcessingContext,
    ) -> Result<Self>
    where
        Self: Sized,
    {
        let mut base_ifd =
            IfdYamlParser::default().parse_from_str(include_str!("./base_ifd.yml"))?;

        let dcp_ifd = if let Some(path) = parameters.take_option::<String>("dcp-yaml")? {
            let path = PathBuf::from_str(&path)?;
            let data = fs::read_to_string(path.clone()).context("couldnt read dcp-yaml file")?;
            IfdYamlParser::new(path).parse_from_str(&data).context("couldnt parse dcp-yaml file")?
        } else {
            IfdYamlParser::default().parse_from_str(include_str!("./default_dcp.yml"))?
        };

        base_ifd.insert_from_other(dcp_ifd);

        let filename = parameters.take("path")?;
        create_dir(&filename).context("Error while creating target directory")?;

        Ok(Self {
            dir_path: filename,
            input: parameters.take("input")?,
            number_of_frames: parameters.take_option("number-of-frames")?,
            priority: parameters.take("priority")?,
            base_ifd,
        })
    }
}

#[async_trait]
impl SinkNode for CinemaDngWriter {
    async fn run(
        &self,
        context: &ProcessingContext,
        progress_callback: Arc<dyn Fn(ProgressUpdate) + Send + Sync>,
    ) -> Result<()> {
        let context = context.clone();
        let dir_path = self.dir_path.clone();
        let base_ifd = self.base_ifd.clone();

        pull_unordered(
            &context.clone(),
            self.priority,
            progress_callback,
            self.input.clone_for_same_puller(),
            self.number_of_frames,
            move |input, frame_number| {
                let frame = context
                    .ensure_cpu_buffer_frame(&input)
                    .context("Wrong input format for CinemaDngWriter")?;

                let ifd = frame_to_dng_ifd(frame, base_ifd.clone())?;

                let file = File::create(format!("{}/{:06}.dng", &dir_path, frame_number))?;
                DngWriter::write_dng(file, true, FileType::Dng, vec![ifd])?;

                Ok::<(), anyhow::Error>(())
            },
        )
        .await
    }
}

impl Offsets for CpuBuffer {
    fn size(&self) -> u32 { self.len() as u32 }
    fn write(&self, writer: &mut dyn Write) -> std::io::Result<()> {
        self.as_slice(|slice| writer.write_all(slice))
    }
}


pub fn frame_to_dng_ifd(frame: Arc<Frame<CpuBuffer>>, base_ifd: Ifd) -> Result<Ifd> {
    let mut ifd = Ifd::new(IfdType::Ifd);
    ifd.insert_from_other(base_ifd);

    if let ColorInterpretation::Bayer(cfa) = frame.interpretation.color_interpretation {
        ifd.insert(
            tags::ifd::CFAPattern,
            match (cfa.red_in_first_row, cfa.red_in_first_col) {
                (true, true) => [0u8, 1, 1, 2],
                (true, false) => [1, 0, 2, 1],
                (false, true) => [1, 2, 0, 1],
                (false, false) => [2, 1, 1, 0],
            },
        );
    } else {
        bail!("cant write non-bayer image as DNG")
    }

    match frame.interpretation.sample_interpretation {
        SampleInterpretation::UInt(bits) => {
            ifd.insert(tags::ifd::BitsPerSample, bits as u32);
            ifd.insert(tags::ifd::SampleFormat, 1);
        }
        SampleInterpretation::FP16 => {
            ifd.insert(tags::ifd::BitsPerSample, 16);
            ifd.insert(tags::ifd::SampleFormat, 3);
        }
        SampleInterpretation::FP32 => {
            ifd.insert(tags::ifd::BitsPerSample, 32);
            ifd.insert(tags::ifd::SampleFormat, 3);
        }
    }

    ifd.insert(tags::ifd::ImageWidth, frame.interpretation.width as u32);
    ifd.insert(tags::ifd::ImageLength, frame.interpretation.height as u32);
    ifd.insert(tags::ifd::RowsPerStrip, frame.interpretation.height as u32);
    if let Some(fps) = frame.interpretation.fps {
        ifd.insert(tags::ifd::FrameRate, IfdValue::SRational((fps * 10000.0) as i32, 10000));
    }

    ifd.insert(tags::ifd::StripOffsets, IfdValue::Offsets(Arc::new(frame.storage.clone())));
    ifd.insert(tags::ifd::StripByteCounts, frame.storage.len() as u32);

    Ok(ifd)
}
