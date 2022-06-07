use crate::pipeline_processing::{
    frame::Rgb,
    node::{InputProcessingNode, NodeID, ProgressUpdate, SinkNode},
    parametrizable::{
        ParameterType::{FloatRange, NodeInput, StringParameter},
        ParameterTypeDescriptor::{Mandatory, Optional},
        ParameterValue,
        Parameterizable,
        Parameters,
        ParametersDescriptor,
    },
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
    fps: f64,
    input: InputProcessingNode,
}
impl Parameterizable for FfmpegWriter {
    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new()
            .with("fps", Mandatory(FloatRange(0., f64::MAX)))
            .with("output", Mandatory(StringParameter))
            .with(
                "input-options",
                Optional(StringParameter, ParameterValue::StringParameter("".to_string())),
            )
            .with("input", Mandatory(NodeInput))
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
            fps: parameters.take("fps")?,
            input: parameters.take("input")?,
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
        let rx = pull_ordered(context, progress_callback, self.input.clone_for_same_puller(), 0);
        let mut frame = context
            .ensure_cpu_buffer::<Rgb>(&rx.recv_async().await.unwrap())
            .context("Wrong input format")?;

        let mut child = Command::new("ffmpeg")
            .args(
                shlex::split(&format!(
                    "{} -f rawvideo -framerate {} -video_size {}x{} -pixel_format rgb24 -i - {}",
                    self.input_options,
                    self.fps,
                    frame.interp.width,
                    frame.interp.height,
                    self.output
                ))
                .unwrap(),
            )
            .stdin(Stdio::piped())
            .spawn()?;

        loop {
            frame.storage.as_slice(|slice| child.stdin.as_mut().unwrap().write_all(slice))?;

            if let Ok(payload) = rx.recv_async().await {
                frame = context.ensure_cpu_buffer::<Rgb>(&payload).context("Wrong input format")?;
            } else {
                break;
            }
        }
        Ok(())
    }
}
