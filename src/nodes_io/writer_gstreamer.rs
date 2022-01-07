use crate::pipeline_processing::{
    frame::Rgb,
    parametrizable::{
        Parameterizable,
        Parameters,
        ParametersDescriptor,
        ParameterType::StringParameter,
        ParameterTypeDescriptor::Mandatory,
    },
    payload::Payload,
    processing_context::ProcessingContext,
};
use anyhow::{anyhow, Context, Result};
use gstreamer::{Buffer, Format, Fraction, ParseContext, Pipeline, prelude::*};
use gstreamer_app::AppSrc;
use gstreamer_video::{VideoFormat, VideoInfo};
use std::thread::{JoinHandle, spawn};

pub struct GstWriter {
    appsrc: AppSrc,
    thread_handle: Option<JoinHandle<()>>,
    context: ProcessingContext,
}
impl Parameterizable for GstWriter {
    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new().with("pipeline", Mandatory(StringParameter))
    }
    fn from_parameters(parameters: &Parameters, context: ProcessingContext) -> Result<Self>
    where
        Self: Sized,
    {
        gstreamer::init()?;
        let mut parse_context = ParseContext::new();
        let pipeline_string =
            format!("appsrc max-bytes=20000000 ! {}", parameters.get::<String>("pipeline")?);
        let pipeline = gstreamer::parse_launch_full(
            &pipeline_string,
            Some(&mut parse_context),
            gstreamer::ParseFlags::empty(),
        )?
        .dynamic_cast::<Pipeline>()
        .unwrap();

        let appsrc =
            pipeline.children().into_iter().last().unwrap().dynamic_cast::<AppSrc>().unwrap();

        let thread_handle = Some(spawn(move || {
            main_loop(pipeline).unwrap();
        }));

        Ok(Self { appsrc, thread_handle, context })
    }
}

impl ProcessingNode for GstWriter {
    fn process(
        &self,
        input: &mut Payload,
        _frame_lock: ProcessingStageLockWaiter,
    ) -> Result<Option<Payload>> {
        let frame = self.context.ensure_cpu_buffer::<Rgb>(input).context("Wrong input format")?;

        let video_info = VideoInfo::builder(
            VideoFormat::Rgb,
            frame.interp.width as u32,
            frame.interp.height as u32,
        )
        .fps(Fraction::new(2, 1))
        .build()
        .expect("Failed to create video info");
        self.appsrc.set_caps(Some(&video_info.to_caps().unwrap()));
        self.appsrc.set_property("format", Format::Time)?;
        // TODO: save the copy
        let vec = frame.storage.as_slice(|s| s.to_vec());
        let buffer = Buffer::from_slice(vec);
        self.appsrc.push_buffer(buffer)?;

        Ok(Some(Payload::empty()))
    }
}
impl Drop for GstWriter {
    fn drop(&mut self) {
        self.appsrc.end_of_stream().unwrap();
        self.thread_handle.take().unwrap().join().unwrap();
    }
}


fn main_loop(pipeline: gstreamer::Pipeline) -> Result<()> {
    pipeline.set_state(gstreamer::State::Playing)?;

    let bus = pipeline.bus().expect("Pipeline without bus. Shouldn't happen!");

    for msg in bus.iter_timed(None) {
        use gstreamer::MessageView;

        match msg.view() {
            MessageView::Eos(..) => break,
            MessageView::Error(err) => {
                pipeline.set_state(gstreamer::State::Null)?;
                return Err(anyhow!(
                    "{:?}{:?}{:?}{:?}",
                    msg.src()
                        .map(|s| String::from(s.path_string()))
                        .unwrap_or_else(|| String::from("None")),
                    err.error().to_string(),
                    err.debug(),
                    err.error()
                ));
            }
            _ => (),
        }
    }

    pipeline.set_state(gstreamer::State::Null)?;

    Ok(())
}
