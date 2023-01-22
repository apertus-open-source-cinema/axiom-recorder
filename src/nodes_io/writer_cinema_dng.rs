use crate::pipeline_processing::{
    buffers::CpuBuffer,
    frame::Raw,
    node::{InputProcessingNode, NodeID, ProgressUpdate, SinkNode},
    parametrizable::prelude::*,
    processing_context::ProcessingContext,
    puller::pull_unordered,
};
use anyhow::{Context, Result};
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
    number_of_frames: u64,
    priority: u8,
    base_ifd: Ifd,
}

impl Parameterizable for CinemaDngWriter {
    const DESCRIPTION: Option<&'static str> = Some("writes Cinema DNG files into a directory");

    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new()
            .with("input", Mandatory(NodeInputParameter))
            .with("path", Mandatory(StringParameter))
            .with("priority", Optional(U8()))
            .with("number-of-frames", Optional(NaturalWithZero()))
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

        let path_string = parameters.take::<String>("dcp-yaml").unwrap_or("".to_string());
        let dcp_ifd = if path_string.is_empty() {
            IfdYamlParser::default().parse_from_str(include_str!("./default_dcp.yml"))?
        } else {
            let path = PathBuf::from_str(&path_string)?;
            let data = fs::read_to_string(path.clone()).context("couldnt read dcp-yaml file")?;
            IfdYamlParser::new(path).parse_from_str(&data).context("couldnt parse dcp-yaml file")?
        };

        base_ifd.insert_from_other(dcp_ifd);

        let filename = parameters.take("path")?;
        create_dir(&filename).context("Error while creating target directory")?;

        Ok(Self {
            dir_path: filename,
            input: parameters.take("input")?,
            number_of_frames: parameters.take("number-of-frames")?,
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
                    .ensure_cpu_buffer::<Raw>(&input)
                    .context("Wrong input format for CinemaDngWriter")?;


                let mut ifd = Ifd::new(IfdType::Ifd);
                ifd.insert_from_other(base_ifd.clone());

                ifd.insert(tags::ifd::ImageWidth, frame.interp.width as u32);
                ifd.insert(tags::ifd::ImageLength, frame.interp.height as u32);
                ifd.insert(tags::ifd::RowsPerStrip, frame.interp.height as u32);
                ifd.insert(
                    tags::ifd::FrameRate,
                    IfdValue::SRational((frame.interp.fps * 10000.0) as i32, 10000),
                );
                ifd.insert(tags::ifd::BitsPerSample, frame.interp.bit_depth as u32);
                ifd.insert(
                    tags::ifd::CFAPattern,
                    match (frame.interp.cfa.red_in_first_row, frame.interp.cfa.red_in_first_col) {
                        (true, true) => [0u8, 1, 1, 2],
                        (true, false) => [1, 0, 2, 1],
                        (false, true) => [1, 2, 0, 1],
                        (false, false) => [2, 1, 1, 0],
                    },
                );

                ifd.insert(
                    tags::ifd::StripOffsets,
                    IfdValue::Offsets(Arc::new(frame.storage.clone())),
                );
                ifd.insert(tags::ifd::StripByteCounts, frame.storage.len() as u32);

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
