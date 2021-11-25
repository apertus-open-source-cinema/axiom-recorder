use crate::pipeline_processing::{
    execute::ProcessingStageLockWaiter,
    frame::Rgb,
    parametrizable::{
        ParameterType::StringParameter,
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
use anyhow::{anyhow, Context, Result};
use std::{
    io::Write,
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
};

pub struct FfmpegWriter {
    output: String,
    input_options: String,
    interp: Arc<Mutex<Option<Rgb>>>,
    child: Arc<Mutex<Option<Child>>>,
    context: ProcessingContext,
}
impl Parameterizable for FfmpegWriter {
    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new().with("output", Mandatory(StringParameter)).with(
            "input-options",
            Optional(StringParameter, ParameterValue::StringParameter("".to_string())),
        )
    }
    fn from_parameters(parameters: &Parameters, context: ProcessingContext) -> Result<Self>
    where
        Self: Sized,
    {
        Ok(Self {
            child: Arc::new(Mutex::new(None)),
            interp: Arc::new(Mutex::new(None)),
            output: parameters.get("output")?,
            input_options: parameters.get("input-options")?,
            context,
        })
    }
}
impl ProcessingNode for FfmpegWriter {
    fn process(
        &self,
        input: &mut Payload,
        frame_lock: ProcessingStageLockWaiter,
    ) -> Result<Option<Payload>> {
        frame_lock.wait();
        let frame = self.context.ensure_cpu_buffer::<Rgb>(input).context("Wrong input format")?;

        {
            let mut interp = self.interp.lock().unwrap();
            if interp.is_none() {
                let child = Command::new("ffmpeg")
                    .args(
                        shlex::split(&format!(
                        "{} -f rawvideo -framerate {} -video_size {}x{} -pixel_format rgb24 -i - {}",
                        self.input_options,
                        frame.interp.fps,
                        frame.interp.width,
                        frame.interp.height,
                        self.output
                    ))
                        .unwrap(),
                    )
                    .stdin(Stdio::piped())
                    .spawn()?;
                *self.child.lock().unwrap() = Some(child);
                *interp = Some(frame.interp)
            } else if interp.is_some() {
                let Rgb { width, height, fps } = interp.unwrap();
                if width != frame.interp.width
                    || height != frame.interp.height
                    || fps != frame.interp.fps
                {
                    return Err(anyhow!(
                        "the resolution or the framerate MAY NOT change during an ffmpeg encoding session"
                    ));
                }
            }
        }

        frame.storage.as_slice(|slice| {
            self.child
                .clone()
                .lock()
                .unwrap()
                .as_mut()
                .unwrap()
                .stdin
                .as_mut()
                .unwrap()
                .write_all(slice)
        })?;

        Ok(Some(Payload::empty()))
    }
}
impl Drop for FfmpegWriter {
    fn drop(&mut self) { self.child.lock().unwrap().as_mut().unwrap().wait().unwrap(); }
}
