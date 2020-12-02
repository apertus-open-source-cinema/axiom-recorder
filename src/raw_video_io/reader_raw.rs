use crate::{
    graph_processing::{
        parametrizable::{
            ParameterType::{IntRange, StringParameter},
            ParameterTypeDescriptor::Mandatory,
            Parameterizable,
            Parameters,
            ParametersDescriptor,
        },
        processing_node::{Payload, ProcessingNode},
    },
    raw_video_io::raw_frame::RawFrame,
};
use anyhow::{anyhow, Result};
use glob::glob;
use std::{
    fs::File,
    io::Read,
    path::{PathBuf},
    sync::Mutex,
    vec::IntoIter,
};

pub struct RawBlobReader {
    file: Mutex<File>,
    frame_count: u64,
    bit_depth: u64,
    width: u64,
    height: u64,
}
impl Parameterizable for RawBlobReader {
    const DESCRIPTION: Option<&'static str> =
        Some("read packed binary frames from a single file without headers or metadata");

    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new()
            .with("file", Mandatory(StringParameter))
            .with("bit_depth", Mandatory(IntRange(8, 16)))
            .with("width", Mandatory(IntRange(0, i64::max_value())))
            .with("height", Mandatory(IntRange(0, i64::max_value())))
    }
    fn from_parameters(options: &Parameters) -> anyhow::Result<Self>
    where
        Self: Sized,
    {
        let width = options.get("width")?;
        let height = options.get("height")?;
        let bit_depth = options.get("bit_depth")?;
        let path: String = options.get("path")?;

        let file = File::open(&path)?;
        let frame_count = file.metadata()?.len() / (width * height * bit_depth / 8);
        Ok(Self { file: Mutex::new(file), frame_count, bit_depth, width, height })
    }
}
impl ProcessingNode for RawBlobReader {
    fn process(&self, _input: &mut Payload) -> Result<Option<Payload>> {
        let mut bytes = vec![0u8; (self.width * self.height * self.bit_depth / 8) as usize];
        let read_count = self.file.lock().unwrap().read(&mut bytes)?;
        if read_count == 0 {
            Ok(None)
        } else if read_count == bytes.len() {
            Ok(Some(Payload::from(RawFrame::new(self.width, self.height, bytes, self.bit_depth))))
        } else {
            Err(anyhow!("File could not be fully consumed. is the resolution set right?"))
        }
    }
    fn size_hint(&self) -> Option<u64> { Some(self.frame_count) }
}

pub struct RawDirectoryReader {
    files_iterator: Mutex<IntoIter<PathBuf>>,
    frame_count: u64,
    bit_depth: u64,
    width: u64,
    height: u64,
}
impl Parameterizable for RawDirectoryReader {
    const DESCRIPTION: Option<&'static str> =
        Some("read packed binary frames without headers or metadata from a directory");

    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new()
            .with("file_pattern", Mandatory(StringParameter))
            .with("bit_depth", Mandatory(IntRange(8, 16)))
            .with("width", Mandatory(IntRange(0, i64::max_value())))
            .with("height", Mandatory(IntRange(0, i64::max_value())))
    }
    fn from_parameters(options: &Parameters) -> anyhow::Result<Self>
    where
        Self: Sized,
    {
        let file_pattern: String = options.get("file_pattern")?;
        let entries = glob(&file_pattern)?.collect::<std::result::Result<Vec<_>, _>>()?;
        let frame_count = entries.len() as u64;
        let files_iterator = Mutex::new(entries.into_iter());
        Ok(Self {
            files_iterator,
            frame_count,
            bit_depth: options.get("bit_depth")?,
            width: options.get("width")?,
            height: options.get("height")?,
        })
    }
}
impl ProcessingNode for RawDirectoryReader {
    fn process(&self, _input: &mut Payload) -> Result<Option<Payload>> {
        let path = self.files_iterator.lock().unwrap().next();
        match path {
            None => Ok(None),
            Some(path) => {
                let mut file = File::open(path)?;
                let mut bytes = vec![0u8; (self.width * self.height * self.bit_depth / 8) as usize];
                file.read_exact(&mut bytes)?;
                Ok(Some(Payload::from(RawFrame::new(
                    self.width,
                    self.height,
                    bytes,
                    self.bit_depth,
                ))))
            }
        }
    }
    fn size_hint(&self) -> Option<u64> { Some(self.frame_count) }
}
