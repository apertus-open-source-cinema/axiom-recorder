use crate::pipeline_processing::{
    frame::{Frame, FrameInterpretation, Raw},
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
use anyhow::Result;
use std::{io::Read, net::TcpStream, sync::Mutex};
use crate::pipeline_processing::node::{Caps, ProcessingNode};
use async_trait::async_trait;
use gstreamer::Format::Default;
use crate::pipeline_processing::frame::FrameInterpretations;
use crate::util::async_notifier::AsyncNotifier;

pub struct TcpReader {
    tcp_connection: Mutex<TcpStream>,
    interp: FrameInterpretations,
    notifier: AsyncNotifier<u64>,
}
impl Parameterizable for TcpReader {
    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new()
            .with("address", Mandatory(StringParameter))
            .with_raw_interpretation()
    }

    fn from_parameters(parameters: &Parameters, context: ProcessingContext) -> Result<Self>
    where
        Self: Sized,
    {
        Ok(Self {
            tcp_connection: Mutex::new(TcpStream::connect(parameters.get::<String>("address")?)?),
            interp: parameters.get_raw_interpretation()?,
            notifier: Default::default(),
        })
    }
}

#[async_trait]
impl ProcessingNode for TcpReader {
    async fn pull(&self, frame_number: u64, context: &ProcessingContext) -> Result<Payload> {
        self.notifier.wait(|x| *x >= frame_number).await;

        let mut buf = unsafe { self.context.get_uninit_cpu_buffer(self.interp.required_bytes()) };
        buf.as_mut_slice(|slice| self.tcp_connection.lock().unwrap().read_exact(slice))?;

        self.notifier.update(|x| {*x = frame_number + 1});
        Ok(Payload::from(Frame { storage: buf, interp: self.interp }))
    }

    fn get_caps(&self) -> Caps {
        Caps {
            frame_count: None,
            is_live: true
        }
    }
}
