use crate::pipeline_processing::{
    frame::{ColorInterpretation, SampleInterpretation},
    node::{InputProcessingNode, NodeID, ProgressUpdate, SinkNode},
    parametrizable::prelude::*,
    processing_context::ProcessingContext,
    puller::pull_ordered,
};
use anyhow::{anyhow, bail, Context, Result};
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
            .with("priority", WithDefault(U8(), ParameterValue::IntRangeValue(0)))
            .with("input-options", WithDefault(StringParameter, StringValue("".to_string())))
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
            None,
        );
        let mut frame = context
            .ensure_cpu_buffer_frame(&rx.recv_async().await.unwrap())
            .context("Wrong input format for FfmpegWriter")?;

        let input_options = &self.input_options;
        let fps = frame.interpretation.fps.ok_or(anyhow!("need to know fps to write video"))?;
        let width = frame.interpretation.width;
        let height = frame.interpretation.height;
        let output = &self.output;
        if !matches!(frame.interpretation.sample_interpretation, SampleInterpretation::UInt(8)) {
            bail!("A frame with bit_depth=8 is required. Convert the bit depth of the frame!")
        }
        let pixel_format = match frame.interpretation.color_interpretation {
            ColorInterpretation::Bayer(_) => bail!("cant write bayer video with ffmpeg!"),
            ColorInterpretation::Rgb => "rgb24",
            ColorInterpretation::Rgba => "rgba",
        };

        let args_string = format!("{input_options} -f rawvideo -framerate {fps} -video_size {width}x{height} -pixel_format {pixel_format} -i - {output}");

        let mut child = Command::new("ffmpeg")
            .args(shlex::split(&args_string).unwrap())
            .stdin(Stdio::piped())
            .spawn()?;

        loop {
            frame.storage.as_slice(|slice| child.stdin.as_mut().unwrap().write_all(slice))?;

            if let Ok(payload) = rx.recv_async().await {
                frame = context
                    .ensure_cpu_buffer_frame(&payload)
                    .context("Wrong input format for FfmpegWriter")?;
            } else {
                break;
            }
        }
        Ok(())
    }
}
