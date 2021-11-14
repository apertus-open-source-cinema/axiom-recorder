use crate::pipeline_processing::{
    execute::ProcessingStageLockWaiter,
    frame::{Frame, FrameInterpretation, Raw},
    parametrizable::{
        ParameterType::{BoolParameter, FloatRange, StringParameter},
        ParameterTypeDescriptor::{Mandatory, Optional},
        ParameterValue,
        Parameterizable,
        Parameters,
        ParametersDescriptor,
    },
    payload::Payload,
    processing_context::ProcessingContext,
    processing_node::ProcessingNode,
};
use anyhow::{anyhow, Result};
use glob::glob;
use std::{fs::File, io::Read, path::PathBuf, sync::Mutex, thread::sleep, time::Duration};

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

pub struct RawDirectoryReader {
    files: Vec<PathBuf>,
    payload_vec: Mutex<Vec<Option<Payload>>>,
    do_loop: bool,
    sleep: f64,
    interp: Raw,
    context: ProcessingContext,
}
impl Parameterizable for RawDirectoryReader {
    const DESCRIPTION: Option<&'static str> =
        Some("read packed binary frames without headers or metadata from a directory");

    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new()
            .with("file-pattern", Mandatory(StringParameter))
            .with_raw_interpretation()
            .with("loop", Optional(BoolParameter, ParameterValue::BoolParameter(false)))
            .with("sleep", Optional(FloatRange(0., f64::MAX), ParameterValue::FloatRange(0.0)))
    }
    fn from_parameters(options: &Parameters, context: ProcessingContext) -> anyhow::Result<Self>
    where
        Self: Sized,
    {
        let file_pattern: String = options.get("file-pattern")?;
        let files = glob(&file_pattern)?.collect::<std::result::Result<Vec<_>, _>>()?;
        let frame_count = files.len();
        Ok(Self {
            files,
            interp: options.get_raw_interpretation()?,
            do_loop: options.get("loop")?,
            payload_vec: Mutex::new((0..frame_count).map(|_| None).collect()),
            sleep: options.get("sleep")?,
            context,
        })
    }
}
impl ProcessingNode for RawDirectoryReader {
    fn process(
        &self,
        _input: &mut Payload,
        frame_lock: ProcessingStageLockWaiter,
    ) -> Result<Option<Payload>> {
        let frame_number = frame_lock.frame() as usize;
        sleep(Duration::from_secs_f64(self.sleep));

        Ok(match self.payload_vec.lock().unwrap()[(frame_number - 1) as usize % self.files.len()] {
            Some(ref payload) => {
                if self.do_loop || frame_number <= self.files.len() {
                    Some(payload.clone())
                } else {
                    None
                }
            }
            ref mut none => {
                if self.do_loop || frame_number <= self.files.len() {
                    let path = &self.files[(frame_number - 1) as usize % self.files.len()];
                    let mut file = File::open(path)?;

                    let mut buffer =
                        unsafe { self.context.get_uninit_cpu_buffer(self.interp.required_bytes()) };
                    buffer.as_mut_slice(|buffer| {
                        file.read_exact(buffer).unwrap();
                    });
                    let payload = Payload::from(Frame { storage: buffer, interp: self.interp });

                    if self.do_loop {
                        *none = Some(payload.clone());
                    }
                    Some(payload)
                } else {
                    None
                }
            }
        })
    }

    fn size_hint(&self) -> Option<u64> {
        if self.do_loop {
            None
        } else {
            Some(self.files.len() as _)
        }
    }
}
