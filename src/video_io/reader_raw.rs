use crate::{
    frame::raw_frame::{CfaDescriptor, RawFrame},
    pipeline_processing::{
        execute::ProcessingStageLockWaiter,
        parametrizable::{
            ParameterType::{BoolParameter, FloatRange, IntRange, StringParameter},
            ParameterTypeDescriptor::{Mandatory, Optional},
            ParameterValue,
            Parameterizable,
            Parameters,
            ParametersDescriptor,
        },
        payload::Payload,
        processing_node::ProcessingNode,
    },
};
use anyhow::{anyhow, Result};
use glob::glob;
use std::{
    fs::File,
    io::Read,
    path::PathBuf,
    sync::Mutex,
    thread::sleep,
    time::Duration,
    vec::IntoIter,
};

pub struct RawBlobReader {
    file: Mutex<File>,
    frame_count: u64,
    bit_depth: u64,
    width: u64,
    height: u64,
    cfa: CfaDescriptor,
    sleep: f64,
}
impl Parameterizable for RawBlobReader {
    const DESCRIPTION: Option<&'static str> =
        Some("read packed binary frames from a single file without headers or metadata");

    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new()
            .with("file", Mandatory(StringParameter))
            .with("bit-depth", Mandatory(IntRange(8, 16)))
            .with("width", Mandatory(IntRange(0, i64::max_value())))
            .with("height", Mandatory(IntRange(0, i64::max_value())))
            .with("first-red-x", Optional(BoolParameter, ParameterValue::BoolParameter(true)))
            .with("first-red-y", Optional(BoolParameter, ParameterValue::BoolParameter(true)))
            .with("sleep", Optional(FloatRange(0., f64::MAX), ParameterValue::FloatRange(0.0)))
    }
    fn from_parameters(options: &Parameters) -> anyhow::Result<Self>
    where
        Self: Sized,
    {
        let width = options.get("width")?;
        let height = options.get("height")?;
        let bit_depth = options.get("bit-depth")?;
        let path: String = options.get("file")?;
        let cfa =
            CfaDescriptor::from_first_red(options.get("first-red-x")?, options.get("first-red-y")?);

        let file = File::open(&path)?;
        let frame_count = file.metadata()?.len() / (width * height * bit_depth / 8);
        Ok(Self {
            file: Mutex::new(file),
            frame_count,
            bit_depth,
            width,
            height,
            cfa,
            sleep: options.get("sleep")?,
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
        let mut bytes = vec![0u8; (self.width * self.height * self.bit_depth / 8) as usize];
        let read_count = self.file.lock().unwrap().read(&mut bytes)?;
        if read_count == 0 {
            Ok(None)
        } else if read_count == bytes.len() {
            Ok(Some(Payload::from(RawFrame::from_bytes(
                bytes,
                self.width,
                self.height,
                self.bit_depth,
                self.cfa,
            )?)))
        } else {
            Err(anyhow!("File could not be fully consumed. is the resolution set right?"))
        }
    }
    fn size_hint(&self) -> Option<u64> { Some(self.frame_count) }
}

pub struct RawDirectoryReader {
    files: Vec<PathBuf>,
    frame_count: usize,
    bit_depth: u64,
    width: u64,
    height: u64,
    cfa: CfaDescriptor,
    do_loop: bool,
    payload_vec: Mutex<Vec<Option<Payload>>>,
    sleep: f64,
}
impl Parameterizable for RawDirectoryReader {
    const DESCRIPTION: Option<&'static str> =
        Some("read packed binary frames without headers or metadata from a directory");

    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new()
            .with("file-pattern", Mandatory(StringParameter))
            .with("bit-depth", Mandatory(IntRange(8, 16)))
            .with("width", Mandatory(IntRange(0, i64::max_value())))
            .with("height", Mandatory(IntRange(0, i64::max_value())))
            .with("first-red-x", Optional(BoolParameter, ParameterValue::BoolParameter(true)))
            .with("first-red-y", Optional(BoolParameter, ParameterValue::BoolParameter(true)))
            .with("loop", Optional(BoolParameter, ParameterValue::BoolParameter(false)))
            .with("sleep", Optional(FloatRange(0., f64::MAX), ParameterValue::FloatRange(0.0)))
    }
    fn from_parameters(options: &Parameters) -> anyhow::Result<Self>
    where
        Self: Sized,
    {
        let file_pattern: String = options.get("file-pattern")?;
        let files = glob(&file_pattern)?.collect::<std::result::Result<Vec<_>, _>>()?;
        let frame_count = files.len();
        let cfa =
            CfaDescriptor::from_first_red(options.get("first-red-x")?, options.get("first-red-y")?);
        Ok(Self {
            files,
            frame_count,
            bit_depth: options.get("bit-depth")?,
            width: options.get("width")?,
            height: options.get("height")?,
            cfa,
            do_loop: options.get("loop")?,
            payload_vec: Mutex::new((0..frame_count).map(|_| None).collect()),
            sleep: options.get("sleep")?,
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

        Ok(match self.payload_vec.lock().unwrap()[(frame_number - 1) as usize % self.frame_count] {
            Some(ref payload) => {
                if self.do_loop || frame_number <= self.frame_count {
                    Some(payload.clone())
                } else {
                    None
                }
            }
            ref mut none => {
                let path = &self.files[(frame_number - 1) as usize % self.frame_count];
                let mut file = File::open(path)?;
                let mut bytes = vec![0u8; (self.width * self.height * self.bit_depth / 8) as usize];
                file.read_exact(&mut bytes)?;
                let payload = Payload::from(RawFrame::from_bytes(
                    bytes,
                    self.width,
                    self.height,
                    self.bit_depth,
                    self.cfa,
                )?);

                if self.do_loop {
                    *none = Some(payload.clone());
                }

                Some(payload)
            }
        })
    }

    fn size_hint(&self) -> Option<u64> {
        if self.do_loop {
            None
        } else {
            Some(self.frame_count as _)
        }
    }
}
