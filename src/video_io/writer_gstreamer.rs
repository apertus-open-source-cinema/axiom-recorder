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
        processing_node::{Payload, ProcessingNode},
    },
};
use anyhow::{anyhow, Result};
use gstreamer::{prelude::*, Buffer, Format, Fraction, ParseContext, Pipeline, BufferMap, Memory};
use gstreamer_app::AppSrc;
use gstreamer_video::{VideoFormat, VideoFrameRef, VideoInfo};
use std::{
    any::Any,
    io::Write,
    sync::{MutexGuard, Arc},
    thread::{spawn, JoinHandle},
};
use core::mem;

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
        let pipeline_string = format!("appsrc max-bytes=20000000 ! {}", parameters.get::<String>("pipeline")?);
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

struct ArcAsRef<T: ?Sized> {
    inner: Arc<T>
}

impl<T: ?Sized> ArcAsRef<T> {
    fn new(t: Arc<T>) -> Self {
        ArcAsRef {
            inner: t
        }
    }
}

impl<G: ?Sized, T: ?Sized> AsRef<G> for ArcAsRef<T>
    where T: AsRef<G>
{
    fn as_ref(&self) -> &G {
        (&*self.inner).as_ref()
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
        let buffer = Buffer::from_slice(ArcAsRef::new(frame));
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
