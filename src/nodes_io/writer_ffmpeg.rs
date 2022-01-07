use crate::pipeline_processing::{
    frame::Rgb,
    node::{ProcessingNode, ProgressUpdate, SinkNode},
    parametrizable::{
        ParameterType::{FloatRange, NodeInput, StringParameter},
        ParameterTypeDescriptor::{Mandatory, Optional},
        ParameterValue,
        Parameterizable,
        Parameters,
        ParametersDescriptor,
    },
    processing_context::ProcessingContext,
    puller::OrderedPuller,
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
    input: Arc<dyn ProcessingNode + Send + Sync>,
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
    fn from_parameters(parameters: &Parameters, _context: &ProcessingContext) -> Result<Self>
    where
        Self: Sized,
    {
        Ok(Self {
            output: parameters.get("output")?,
            input_options: parameters.get("input-options")?,
            fps: parameters.get("fps")?,
            input: parameters.get("input")?,
        })
    }
}

#[async_trait]
impl SinkNode for FfmpegWriter {
    async fn run(
        &self,
        context: &ProcessingContext,
        _progress_callback: Arc<dyn Fn(ProgressUpdate) + Send + Sync>,
    ) -> Result<()> {
        let puller = OrderedPuller::new(context, self.input.clone(), false, 0);
        let mut frame = context
            .ensure_cpu_buffer::<Rgb>(&puller.recv().unwrap())
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

            if let Ok(payload) = puller.recv() {
                frame = context.ensure_cpu_buffer::<Rgb>(&payload).context("Wrong input format")?;
            } else {
                break;
            }
        }
        Ok(())
    }
}
