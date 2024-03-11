use crate::pipeline_processing::{
    node::{InputProcessingNode, NodeID, ProgressUpdate, SinkNode},
    parametrizable::prelude::*,
    processing_context::ProcessingContext,
    puller::{pull_ordered, pull_unordered},
};
use anyhow::Result;
use async_trait::async_trait;
use std::{
    fs::{create_dir, File},
    io::prelude::*,
    sync::{Arc, Mutex},
};


pub struct RawBlobWriter {
    file: Arc<Mutex<File>>,
    input: InputProcessingNode,
    number_of_frames: u64,
    priority: u8,
}
impl Parameterizable for RawBlobWriter {
    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new()
            .with("path", Mandatory(StringParameter))
            .with("input", Mandatory(NodeInputParameter))
            .with("priority", Optional(U8()))
            .with("number-of-frames", Optional(NaturalWithZero()))
    }
    fn from_parameters(
        mut parameters: Parameters,
        _is_input_to: &[NodeID],
        _context: &ProcessingContext,
    ) -> Result<Self>
    where
        Self: Sized,
    {
        Ok(Self {
            file: Arc::new(Mutex::new(File::create(parameters.take::<String>("path")?)?)),
            input: parameters.take("input")?,
            number_of_frames: parameters.take("number-of-frames")?,
            priority: parameters.take("priority")?,
        })
    }
}

#[async_trait]
impl SinkNode for RawBlobWriter {
    async fn run(
        &self,
        context: &ProcessingContext,
        progress_callback: Arc<dyn Fn(ProgressUpdate) + Send + Sync>,
    ) -> Result<()> {
        let rx = pull_ordered(
            context,
            self.priority,
            progress_callback,
            self.input.clone_for_same_puller(),
            self.number_of_frames,
        );
        while let Ok(payload) = rx.recv_async().await {
            let buffer = context.ensure_any_cpu_buffer(&payload)?;
            buffer.as_slice(|slice| self.file.lock().unwrap().write_all(slice))?;
        }

        Ok(())
    }
}

pub struct RawDirectoryWriter {
    dir_path: String,
    input: InputProcessingNode,
    number_of_frames: u64,
    priority: u8,
}
impl Parameterizable for RawDirectoryWriter {
    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new()
            .with("path", Mandatory(StringParameter))
            .with("input", Mandatory(NodeInputParameter))
            .with("priority", Optional(U8()))
            .with("number-of-frames", Optional(NaturalWithZero()))
    }

    fn from_parameters(
        mut parameters: Parameters,
        _is_input_to: &[NodeID],
        _context: &ProcessingContext,
    ) -> Result<Self>
    where
        Self: Sized,
    {
        let filename = parameters.take("path")?;
        create_dir(&filename)?;
        Ok(Self {
            dir_path: filename,
            input: parameters.take("input")?,
            number_of_frames: parameters.take("number-of-frames")?,
            priority: parameters.take("priority")?,
        })
    }
}
#[async_trait]
impl SinkNode for RawDirectoryWriter {
    async fn run(
        &self,
        context: &ProcessingContext,
        progress_callback: Arc<dyn Fn(ProgressUpdate) + Send + Sync>,
    ) -> Result<()> {
        let dir_path = self.dir_path.clone();
        let context_clone = context.clone();
        pull_unordered(
            context,
            self.priority,
            progress_callback,
            self.input.clone_for_same_puller(),
            self.number_of_frames,
            move |payload, frame_number| {
                let buffer = context_clone.ensure_any_cpu_buffer(&payload)?;
                let mut file = File::create(format!("{}/{:06}.data", &dir_path, frame_number))?;
                buffer.as_slice(|slice| file.write_all(slice))?;
                Ok(())
            },
        )
        .await
    }
}
