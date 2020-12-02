use std::{
    fs::{create_dir, File},
    io::prelude::*,
    sync::{Arc, Mutex},
};

use crate::{
    graph_processing::{
        parametrizable::{
            ParameterType::StringParameter,
            ParameterTypeDescriptor::Mandatory,
            Parameterizable,
            Parameters,
            ParametersDescriptor,
        },
        processing_node::{Payload, ProcessingNode},
    },
    raw_video_io::raw_frame::RawFrame,
};
use anyhow::Result;

use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
    },
};


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
    fn process(&self, input: &mut Payload) -> Result<Option<Payload>> {
        let frame = input.downcast::<RawFrame>()?;
        self.file.lock().unwrap().write_all(&frame.buffer.packed_data)?;
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
    fn process(&self, input: &mut Payload) -> Result<Option<Payload>> {
        let current_frame_number = self.frame_number.fetch_add(1, Ordering::SeqCst);
        let frame = input.downcast::<RawFrame>()?;
        let mut file =
            File::create(format!("{}/{:06}.raw8", &self.dir_path, current_frame_number))?;
        file.write_all(&frame.buffer.packed_data)?;
        Ok(Some(Payload::empty()))
    }
}
