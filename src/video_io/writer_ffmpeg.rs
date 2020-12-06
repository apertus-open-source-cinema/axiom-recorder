use std::sync::{Arc, Mutex, MutexGuard};
use crate::pipeline_processing::parametrizable::{ParametersDescriptor, Parameterizable, Parameters};
use std::fs::File;
use crate::pipeline_processing::parametrizable::ParameterTypeDescriptor::Mandatory;
use crate::pipeline_processing::parametrizable::ParameterType::{StringParameter, FloatRange};
use anyhow::{Result, anyhow};
use crate::pipeline_processing::processing_node::{ProcessingNode, Payload};
use crate::frame::raw_frame::RawFrame;
use std::io::Write;
use std::process::{Child, Command, Stdio};
use crate::frame::rgb_frame::RgbFrame;

pub struct FfmpegWriter {
    output: String,
    fps: f64,
    resolution: Arc<Mutex<Option<[u64; 2]>>>,
    child: Arc<Mutex<Option<Child>>>,
}
impl Parameterizable for FfmpegWriter {
    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new()
            .with("fps", Mandatory(FloatRange(0., f64::MAX)))
            .with("output", Mandatory(StringParameter))
    }
    fn from_parameters(parameters: &Parameters) -> Result<Self>
        where
            Self: Sized,
    {
        Ok(Self {
            child: Arc::new(Mutex::new(None)),
            resolution: Arc::new(Mutex::new(None)),
            output: parameters.get("output")?,
            fps: parameters.get("fps")?,
        })
    }
}
impl ProcessingNode for FfmpegWriter {
    fn process(&self, input: &mut Payload) -> Result<Option<Payload>> {
        let frame = input.downcast::<RgbFrame>()?;

        {
            let mut resolution = self.resolution.lock().unwrap();
            if resolution.is_none() {
                let child = Command::new("ffmpeg")
                    .args(format!(
                        "-f rawvideo -framerate {} -video_size {}x{} -pixel_format rgb24 -i - {}",
                        self.fps,
                        frame.width,
                        frame.height,
                        self.output.to_string()
                    ).split(' '))
                    .stdin(Stdio::piped())
                    .spawn()?;
                *self.child.lock().unwrap() = Some(child);
                *resolution = Some([frame.width, frame.height])
            } else if resolution.is_some() {
                let [width, height] = resolution.unwrap();
                if width != frame.width || height != frame.height {
                    return Err(anyhow!("the resolution MAY NOT change during an ffmpeg encoding session"))
                }
            }
        }

        {
            self.child.clone().lock().unwrap().as_mut().unwrap().stdin.as_mut().unwrap().write_all(&frame.buffer.clone())?;
        }

        Ok(Some(Payload::empty()))
    }
}
impl Drop for FfmpegWriter {
    fn drop(&mut self) {
        self.child.lock().unwrap().as_mut().unwrap().wait().unwrap();
    }
}
