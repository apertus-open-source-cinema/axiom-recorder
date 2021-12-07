use crate::pipeline_processing::{
    frame::{Frame, FrameInterpretation, FrameInterpretations},
    node::{Caps, ProcessingNode},
    parametrizable::{
        ParameterType::{BoolParameter, StringParameter},
        ParameterTypeDescriptor::{Mandatory, Optional},
        ParameterValue,
        Parameterizable,
        Parameters,
        ParametersDescriptor,
    },
    payload::Payload,
    processing_context::ProcessingContext,
};
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use glob::glob;
use std::{fs::File, io::Read, path::PathBuf, sync::Mutex};

/*
pub struct RawBlobReader {
    file: Mutex<File>,
    interp: Raw,
    frame_count: u64,
    sleep: f64,
    context: ProcessingContext,
}
impl Parameterizable for RawBlobReader {
    const DESCRIPTION: Option<&'static str> =
        Some("read packed binary frames from a single file without headers or metadata");

    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new()
            .with("file", Mandatory(StringParameter))
            .with_raw_interpretation()
            .with("sleep", Optional(FloatRange(0., f64::MAX), ParameterValue::FloatRange(0.0)))
    }
    fn from_parameters(options: &Parameters, context: ProcessingContext) -> anyhow::Result<Self>
    where
        Self: Sized,
    {
        let interp = options.get_raw_interpretation()?;
        let path: String = options.get("file")?;

        let file = File::open(&path)?;
        let frame_count = file.metadata()?.len() / interp.required_bytes() as u64;
        Ok(Self {
            file: Mutex::new(file),
            frame_count,
            interp,
            sleep: options.get("sleep")?,
            context,
        })
    }
}
impl ProcessingNode for RawBlobReader {
    fn process(
        &self,
        _input: &mut Payload,
        _frame_lock: ProcessingStageLockWaiter,
    ) -> Result<Option<Payload>> {
        sleep(Duration::from_secs_f64(self.sleep));

        let mut buffer =
            unsafe { self.context.get_uninit_cpu_buffer(self.interp.required_bytes()) };
        let read_count =
            buffer.as_mut_slice(|buffer| self.file.lock().unwrap().read(buffer).unwrap());

        if read_count == 0 {
            Ok(None)
        } else if read_count == buffer.len() {
            Ok(Some(Payload::from(Frame { storage: buffer, interp: self.interp })))
        } else {
            Err(anyhow!("File could not be fully consumed. is the resolution set right?"))
        }
    }
    fn size_hint(&self) -> Option<u64> { Some(self.frame_count) }
}
 */

pub struct RawDirectoryReader {
    files: Vec<PathBuf>,
    interp: FrameInterpretations,
    cache_frames: bool,
    cache: Mutex<Vec<Option<Payload>>>,
}
impl Parameterizable for RawDirectoryReader {
    const DESCRIPTION: Option<&'static str> =
        Some("read packed binary frames without headers or metadata from a directory");

    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new()
            .with_interpretation()
            .with("file-pattern", Mandatory(StringParameter))
            .with("cache-frames", Optional(BoolParameter, ParameterValue::BoolParameter(false)))
    }
    fn from_parameters(options: &Parameters, _context: &ProcessingContext) -> anyhow::Result<Self>
    where
        Self: Sized,
    {
        let file_pattern: String = options.get("file-pattern")?;
        let files = glob(&file_pattern)?.collect::<std::result::Result<Vec<_>, _>>()?;
        let frame_count = files.len();
        Ok(Self {
            files,
            interp: options.get_interpretation()?,
            cache_frames: options.get("cache-frames")?,
            cache: Mutex::new((0..frame_count).map(|_| None).collect()),
        })
    }
}

#[async_trait]
impl ProcessingNode for RawDirectoryReader {
    async fn pull(&self, frame_number: u64, context: &ProcessingContext) -> Result<Payload> {
        if frame_number >= self.files.len() as u64 {
            return Err(anyhow!(
                "frame {} was requested but this stream only has a length of {}",
                frame_number,
                self.files.len()
            ));
        }

        let path = &self.files[frame_number as usize];
        let mut file = File::open(path)?;

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
        Caps { frame_count: Some(self.files.len() as u64), is_live: false }
    }
}
