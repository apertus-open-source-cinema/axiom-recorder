use std::{
    fs::{create_dir, File},
    io::prelude::*,
    sync::{Arc, Mutex},
};

use crate::pipeline_processing::{
    execute::ProcessingStageLockWaiter,
    parametrizable::{
        ParameterType::StringParameter,
        ParameterTypeDescriptor::Mandatory,
        Parameterizable,
        Parameters,
        ParametersDescriptor,
    },
    processing_node::ProcessingNode,
};
use anyhow::{anyhow, Result};

use crate::{
    frame::{raw_frame::RawFrame, rgb_frame::RgbFrame},
    pipeline_processing::payload::Payload,
};
use std::sync::atomic::{AtomicU64, Ordering};


pub struct RawBlobWriter {
    file: Arc<Mutex<File>>,
}
impl Parameterizable for RawBlobWriter {
    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new().with("path", Mandatory(StringParameter))
    }
    fn from_parameters(parameters: &Parameters) -> Result<Self>
    where
        Self: Sized,
    {
        Ok(Self { file: Arc::new(Mutex::new(File::create(parameters.get::<String>("path")?)?)) })
    }
}
impl ProcessingNode for RawBlobWriter {
    fn process(
        &self,
        input: &mut Payload,
        _frame_lock: ProcessingStageLockWaiter,
    ) -> Result<Option<Payload>> {
        if let Ok(frame) = input.downcast::<RawFrame>() {
            self.file.lock().unwrap().write_all(&frame.buffer)?;
        } else if let Ok(frame) = input.downcast::<RgbFrame>() {
            self.file.lock().unwrap().write_all(&frame.buffer)?;
        } else {
            return Err(anyhow!("unknown input format {}", input.type_name));
        }
        Ok(Some(Payload::empty()))
    }
}

pub struct RawDirectoryWriter {
    dir_path: String,
    frame_number: AtomicU64,
}
impl Parameterizable for RawDirectoryWriter {
    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new().with("path", Mandatory(StringParameter))
    }

    fn from_parameters(parameters: &Parameters) -> Result<Self>
    where
        Self: Sized,
    {
        let filename = parameters.get("path")?;
        create_dir(&filename)?;
        Ok(Self { dir_path: filename, frame_number: AtomicU64::new(0) })
    }
}
impl ProcessingNode for RawDirectoryWriter {
    fn process(
        &self,
        input: &mut Payload,
        frame_lock: ProcessingStageLockWaiter,
    ) -> Result<Option<Payload>> {
        let current_frame_number = frame_lock.frame();
        let mut file =
            File::create(format!("{}/{:06}.data", &self.dir_path, current_frame_number))?;
        if let Ok(frame) = input.downcast::<RawFrame>() {
            file.write_all(&frame.buffer)?;
        } else if let Ok(frame) = input.downcast::<RgbFrame>() {
            file.write_all(&frame.buffer)?;
        } else {
            return Err(anyhow!("unknown input format {}", input.type_name));
        }
        Ok(Some(Payload::empty()))
    }
}
