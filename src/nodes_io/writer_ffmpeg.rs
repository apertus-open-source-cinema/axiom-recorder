use crate::pipeline_processing::{
    frame::Rgb,
    node::{InputProcessingNode, NodeID, ProgressUpdate, SinkNode},
    parametrizable::prelude::*,
    processing_context::ProcessingContext,
    puller::pull_ordered,
};
use anyhow::{Context, Result};
use async_trait::async_trait;
use std::{
    io::Write,
    process::{Command, Stdio},
    sync::Arc,
};

pub struct FfmpegWriter {
    output: String,
    input_options: String,
    input: InputProcessingNode,
    priority: u8,
}
impl Parameterizable for FfmpegWriter {
    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new()
            .with("input", Mandatory(NodeInputParameter))
            .with("output", Mandatory(StringParameter))
            .with("priority", Optional(U8()))
            .with("input-options", Optional(StringParameter))
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
            output: parameters.take("output")?,
            input_options: parameters.take("input-options")?,
            input: parameters.take("input")?,
            priority: parameters.take("priority")?,
        })
    }
}

#[async_trait]
impl SinkNode for FfmpegWriter {
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
            0,
        );
        let mut frame = context
            .ensure_cpu_buffer::<Rgb>(&rx.recv_async().await.unwrap())
            .context("Wrong input format for FfmpegWriter")?;

        let input_options = &self.input_options;
        let fps = frame.interp.fps;
        let width = frame.interp.width;
        let height = frame.interp.height;
        let output = &self.output;
        let args_string = format!("{input_options} -f rawvideo -framerate {fps} -video_size {width}x{height} -pixel_format rgb24 -i - {output}");

        let mut child = Command::new("ffmpeg")
            .args(shlex::split(&args_string).unwrap())
            .stdin(Stdio::piped())
            .spawn()?;

        loop {
            frame.storage.as_slice(|slice| child.stdin.as_mut().unwrap().write_all(slice))?;

            if let Ok(payload) = rx.recv_async().await {
                frame = context
                    .ensure_cpu_buffer::<Rgb>(&payload)
                    .context("Wrong input format for FfmpegWriter")?;
            } else {
                break;
            }
        }
        Ok(())
    }
}
