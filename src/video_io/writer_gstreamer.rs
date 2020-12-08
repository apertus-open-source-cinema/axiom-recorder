use crate::{
    frame::rgba_frame::RgbaFrame,
    pipeline_processing::{
        parametrizable::{
            ParameterType::StringParameter,
            ParameterTypeDescriptor::Mandatory,
            Parameterizable,
            Parameters,
            ParametersDescriptor,
        },
        payload::Payload,
        processing_node::ProcessingNode,
    },
};
use anyhow::{anyhow, Result};
use core::mem;
use gstreamer::{prelude::*, Buffer, BufferMap, Format, Fraction, Memory, ParseContext, Pipeline};
use gstreamer_app::AppSrc;
use gstreamer_video::{VideoFormat, VideoFrameRef, VideoInfo};
use std::{
    io::Write,
    sync::MutexGuard,
    thread::{spawn, JoinHandle},
};

pub struct GstWriter {
    appsrc: AppSrc,
    thread_handle: Option<JoinHandle<()>>,
}
impl Parameterizable for GstWriter {
    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new().with("pipeline", Mandatory(StringParameter))
    }
    fn from_parameters(parameters: &Parameters) -> Result<Self>
    where
        Self: Sized,
    {
        gstreamer::init()?;
        let mut context = ParseContext::new();
        let pipeline_string =
            format!("appsrc max-bytes=20000000 ! {}", parameters.get::<String>("pipeline")?);
        let pipeline = gstreamer::parse_launch_full(
            &pipeline_string,
            Some(&mut context),
            gstreamer::ParseFlags::empty(),
        )?
        .dynamic_cast::<Pipeline>()
        .unwrap();

        let appsrc =
            pipeline.get_children().into_iter().last().unwrap().dynamic_cast::<AppSrc>().unwrap();

        let thread_handle = Some(spawn(move || {
            main_loop(pipeline).unwrap();
        }));

        Ok(Self { appsrc, thread_handle })
    }
}
impl ProcessingNode for GstWriter {
    fn process(
        &self,
        input: &mut Payload,
        _frame_lock: MutexGuard<u64>,
    ) -> Result<Option<Payload>> {
        let frame = input.downcast::<RgbaFrame>()?;

        let video_info =
            VideoInfo::builder(VideoFormat::Rgbx, frame.width as u32, frame.height as u32)
                .fps(Fraction::new(2, 1))
                .build()
                .expect("Failed to create video info");
        self.appsrc.set_caps(Some(&video_info.to_caps().unwrap()));
        self.appsrc.set_property_format(Format::Time);
        let buffer = Buffer::from_slice(&**Box::leak(Box::new(frame.clone())).buffer);
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

    let bus = pipeline.get_bus().expect("Pipeline without bus. Shouldn't happen!");

    for msg in bus.iter_timed(gstreamer::CLOCK_TIME_NONE) {
        use gstreamer::MessageView;

        match msg.view() {
            MessageView::Eos(..) => break,
            MessageView::Error(err) => {
                pipeline.set_state(gstreamer::State::Null)?;
                return Err(anyhow!(
                    "{:?}{:?}{:?}{:?}",
                    msg.get_src()
                        .map(|s| String::from(s.get_path_string()))
                        .unwrap_or_else(|| String::from("None")),
                    err.get_error().to_string(),
                    err.get_debug(),
                    err.get_error()
                ));
            }
            _ => (),
        }
    }

    pipeline.set_state(gstreamer::State::Null)?;

    Ok(())
}
