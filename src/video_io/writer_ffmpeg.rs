use crate::{
    frame::rgb_frame::RgbFrame,
    pipeline_processing::{
        parametrizable::{
            ParameterType::{FloatRange, StringParameter},
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
use std::{
    io::Write,
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex, MutexGuard},
};

pub struct FfmpegWriter {
    output: String,
    input_options: String,
    fps: f64,
    resolution: Arc<Mutex<Option<[u64; 2]>>>,
    child: Arc<Mutex<Option<Child>>>,
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
    }
    fn from_parameters(parameters: &Parameters) -> Result<Self>
    where
        Self: Sized,
    {
        Ok(Self {
            child: Arc::new(Mutex::new(None)),
            resolution: Arc::new(Mutex::new(None)),
            output: parameters.get("output")?,
            input_options: parameters.get("input-options")?,
            fps: parameters.get("fps")?,
        })
    }
}
impl ProcessingNode for FfmpegWriter {
    fn process(
        &self,
        input: &mut Payload,
        _frame_lock: MutexGuard<u64>,
    ) -> Result<Option<Payload>> {
        let frame = input.downcast::<RgbFrame>()?;

        {
            let mut resolution = self.resolution.lock().unwrap();
            if resolution.is_none() {
                let child = Command::new("ffmpeg")
                    .args(
                        shlex::split(&format!(
                        "{} -f rawvideo -framerate {} -video_size {}x{} -pixel_format rgb24 -i - {}",
                        self.input_options,
                        self.fps,
                        frame.width,
                        frame.height,
                        self.output.to_string()
                    ))
                        .unwrap(),
                    )
                    .stdin(Stdio::piped())
                    .spawn()?;
                *self.child.lock().unwrap() = Some(child);
                *resolution = Some([frame.width, frame.height])
            } else if resolution.is_some() {
                let [width, height] = resolution.unwrap();
                if width != frame.width || height != frame.height {
                    return Err(anyhow!(
                        "the resolution MAY NOT change during an ffmpeg encoding session"
                    ));
                }
            }
        }

        {
            self.child
                .clone()
                .lock()
                .unwrap()
                .as_mut()
                .unwrap()
                .stdin
                .as_mut()
                .unwrap()
                .write_all(&frame.buffer)?;
        }

        Ok(Some(Payload::empty()))
    }
}
impl Drop for FfmpegWriter {
    fn drop(&mut self) { self.child.lock().unwrap().as_mut().unwrap().wait().unwrap(); }
}
