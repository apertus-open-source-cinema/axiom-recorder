use crate::pipeline_processing::{
    frame::{Frame, FrameInterpretation, FrameInterpretations},
    node::{Caps, NodeID, ProcessingNode},
    parametrizable::{
        ParameterType::{BoolParameter, StringParameter},
        ParameterTypeDescriptor::{Mandatory, Optional},
        Parameterizable,
        Parameters,
        ParametersDescriptor,
        SerdeParameterValue,
    },
    payload::Payload,
    processing_context::ProcessingContext,
};
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use glob::glob;
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::PathBuf,
    sync::Mutex,
};


pub struct RawBlobReader {
    file: Mutex<File>,
    interp: FrameInterpretations,
    cache_frames: bool,
    cache: Mutex<Vec<Option<Payload>>>,
    frame_count: u64,
}
impl Parameterizable for RawBlobReader {
    const DESCRIPTION: Option<&'static str> =
        Some("read packed binary frames from a single file without headers or metadata");

    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new()
            .with_interpretation()
            .with("file", Mandatory(StringParameter))
            .with(
                "cache-frames",
                Optional(BoolParameter, SerdeParameterValue::BoolParameter(false)),
            )
    }
    fn from_parameters(
        mut options: Parameters,
        _is_input_to: &[NodeID],
        _context: &ProcessingContext,
    ) -> anyhow::Result<Self>
    where
        Self: Sized,
    {
        let path: String = options.take("file")?;
        let file = File::open(path)?;

        let interp = options.get_interpretation()?;
        let frame_count = file.metadata()?.len() / interp.required_bytes() as u64;
        Ok(Self {
            file: Mutex::new(file),
            interp,
            frame_count,
            cache_frames: options.take("cache-frames")?,
            cache: Mutex::new((0..frame_count).map(|_| None).collect()),
        })
    }
}
#[async_trait]
impl ProcessingNode for RawBlobReader {
    async fn pull(
        &self,
        frame_number: u64,
        _puller_id: NodeID,
        context: &ProcessingContext,
    ) -> Result<Payload> {
        if frame_number >= self.frame_count as u64 {
            return Err(anyhow!(
                "frame {} was requested but this stream only has a length of {}",
                frame_number,
                self.frame_count
            ));
        }

        let mut file = self.file.lock().unwrap();
        file.seek(SeekFrom::Start(frame_number * self.interp.required_bytes() as u64))?;

        let mut buffer = unsafe { context.get_uninit_cpu_buffer(self.interp.required_bytes()) };
        buffer
            .as_mut_slice(|buffer| file.read_exact(buffer).context("error while reading file"))?;

        if self.cache_frames {
            if let Some(cached) = self.cache.lock().unwrap()[frame_number as usize].clone() {
                return Ok(cached);
            }
        }

        let payload = match self.interp {
            FrameInterpretations::Raw(interp) => Payload::from(Frame { storage: buffer, interp }),
            FrameInterpretations::Rgb(interp) => Payload::from(Frame { storage: buffer, interp }),
            FrameInterpretations::Rgba(interp) => Payload::from(Frame { storage: buffer, interp }),
        };

        self.cache.lock().unwrap()[frame_number as usize] = Some(payload.clone());
        Ok(payload)
    }

    fn get_caps(&self) -> Caps {
        Caps { frame_count: Some(self.frame_count as u64), is_live: false }
    }
}


pub struct RawDirectoryReader {
    files: Vec<PathBuf>,
    interp: FrameInterpretations,
    cache_frames: bool,
    internal_loop: bool,
    cache: Mutex<Vec<Option<Payload>>>,
}
impl Parameterizable for RawDirectoryReader {
    const DESCRIPTION: Option<&'static str> =
        Some("read packed binary frames without headers or metadata from a directory");

    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new()
            .with_interpretation()
            .with("file-pattern", Mandatory(StringParameter))
            .with(
                "cache-frames",
                Optional(BoolParameter, SerdeParameterValue::BoolParameter(false)),
            )
            .with(
                "internal-loop",
                Optional(BoolParameter, SerdeParameterValue::BoolParameter(false)),
            )
    }
    fn from_parameters(
        mut options: Parameters,
        _is_input_to: &[NodeID],
        _context: &ProcessingContext,
    ) -> anyhow::Result<Self>
    where
        Self: Sized,
    {
        let file_pattern: String = options.take("file-pattern")?;
        let files = glob(&file_pattern)?.collect::<std::result::Result<Vec<_>, _>>()?;
        let frame_count = files.len();
        if frame_count == 0 {
            return Err(anyhow!("no files matched the pattern {}", file_pattern));
        }
        Ok(Self {
            files,
            interp: options.get_interpretation()?,
            cache_frames: options.take("cache-frames")?,
            internal_loop: options.take("internal-loop")?,
            cache: Mutex::new((0..frame_count).map(|_| None).collect()),
        })
    }
}

#[async_trait]
impl ProcessingNode for RawDirectoryReader {
    async fn pull(
        &self,
        mut frame_number: u64,
        _puller_id: NodeID,
        context: &ProcessingContext,
    ) -> Result<Payload> {
        if self.internal_loop {
            frame_number %= self.files.len() as u64;
        }
        if frame_number >= self.files.len() as u64 {
            return Err(anyhow!(
                "frame {} was requested but this stream only has a length of {}",
                frame_number,
                self.files.len()
            ));
        }

        if self.cache_frames {
            if let Some(cached) = &self.cache.lock().unwrap()[frame_number as usize] {
                return Ok(cached.clone());
            }
        }

        let path = &self.files[frame_number as usize];
        let mut file = File::open(path)?;
        let mut buffer = unsafe { context.get_uninit_cpu_buffer(self.interp.required_bytes()) };
        buffer
            .as_mut_slice(|buffer| file.read_exact(buffer).context("error while reading file"))?;


        let payload = match self.interp {
            FrameInterpretations::Raw(interp) => Payload::from(Frame { storage: buffer, interp }),
            FrameInterpretations::Rgb(interp) => Payload::from(Frame { storage: buffer, interp }),
            FrameInterpretations::Rgba(interp) => Payload::from(Frame { storage: buffer, interp }),
        };

        if self.cache_frames {
            self.cache.lock().unwrap()[frame_number as usize] = Some(payload.clone());
        }
        Ok(payload)
    }

    fn get_caps(&self) -> Caps {
        Caps {
            frame_count: if self.internal_loop { None } else { Some(self.files.len() as u64) },
            is_live: false,
        }
    }
}
