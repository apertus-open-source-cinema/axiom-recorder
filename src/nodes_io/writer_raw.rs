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

use crate::pipeline_processing::{
    frame::{Raw, Rgb},
    payload::Payload,
    processing_context::ProcessingContext,
};


pub struct RawBlobWriter {
    file: Arc<Mutex<File>>,
    context: ProcessingContext,
}
impl Parameterizable for RawBlobWriter {
    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new().with("path", Mandatory(StringParameter))
    }
    fn from_parameters(parameters: &Parameters, context: ProcessingContext) -> Result<Self>
    where
        Self: Sized,
    {
        Ok(Self {
            file: Arc::new(Mutex::new(File::create(parameters.get::<String>("path")?)?)),
            context,
        })
    }
}
impl ProcessingNode for RawBlobWriter {
    fn process(
        &self,
        input: &mut Payload,
        _frame_lock: ProcessingStageLockWaiter,
    ) -> Result<Option<Payload>> {
        if let Ok(frame) = self.context.ensure_cpu_buffer::<Rgb>(input) {
            frame.storage.as_slice(|slice| self.file.lock().unwrap().write_all(slice))?;
        } else if let Ok(frame) = self.context.ensure_cpu_buffer::<Raw>(input) {
            frame.storage.as_slice(|slice| self.file.lock().unwrap().write_all(slice))?;
        } else {
            return Err(anyhow!("unknown input format {}", input.type_name));
        }
        Ok(Some(Payload::empty()))
    }
}

pub struct RawDirectoryWriter {
    dir_path: String,
    context: ProcessingContext,
}
impl Parameterizable for RawDirectoryWriter {
    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new().with("path", Mandatory(StringParameter))
    }

    fn from_parameters(parameters: &Parameters, context: ProcessingContext) -> Result<Self>
    where
        Self: Sized,
    {
        let filename = parameters.get("path")?;
        create_dir(&filename)?;
        Ok(Self { dir_path: filename, context })
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
        if let Ok(frame) = self.context.ensure_cpu_buffer::<Rgb>(input) {
            frame.storage.as_slice(|slice| file.write_all(slice))?;
        } else if let Ok(frame) = self.context.ensure_cpu_buffer::<Raw>(input) {
            frame.storage.as_slice(|slice| file.write_all(slice))?;
        } else {
            return Err(anyhow!("unknown input format {}", input.type_name));
        }
        Ok(Some(Payload::empty()))
    }
}
