use crate::{
    pipeline_processing::{
        frame::{Frame, FrameInterpretation, FrameInterpretations},
        node::{Caps, ProcessingNode},
        parametrizable::{
            ParameterType::StringParameter,
            ParameterTypeDescriptor::Mandatory,
            Parameterizable,
            Parameters,
            ParametersDescriptor,
        },
        payload::Payload,
        processing_context::ProcessingContext,
    },
    util::async_notifier::AsyncNotifier,
};
use anyhow::Result;
use async_trait::async_trait;
use std::{io::Read, net::TcpStream, sync::Mutex};

pub struct TcpReader {
    tcp_connection: Mutex<TcpStream>,
    interp: FrameInterpretations,
    notifier: AsyncNotifier<u64>,
}
impl Parameterizable for TcpReader {
    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new()
            .with("address", Mandatory(StringParameter))
            .with_interpretation()
    }

    fn from_parameters(parameters: &Parameters, _context: &ProcessingContext) -> Result<Self>
    where
        Self: Sized,
    {
        Ok(Self {
            tcp_connection: Mutex::new(TcpStream::connect(parameters.get::<String>("address")?)?),
            interp: parameters.get_interpretation()?,
            notifier: Default::default(),
        })
    }
}

#[async_trait]
impl ProcessingNode for TcpReader {
    async fn pull(&self, frame_number: u64, context: &ProcessingContext) -> Result<Payload> {
        self.notifier.wait(move |x| *x >= frame_number).await;

        let mut buffer = unsafe { context.get_uninit_cpu_buffer(self.interp.required_bytes()) };
        buffer.as_mut_slice(|slice| self.tcp_connection.lock().unwrap().read_exact(slice))?;

        self.notifier.update(|x| *x = frame_number + 1);

        let payload = match self.interp {
            FrameInterpretations::Raw(interp) => Payload::from(Frame { storage: buffer, interp }),
            FrameInterpretations::Rgb(interp) => Payload::from(Frame { storage: buffer, interp }),
            FrameInterpretations::Rgba(interp) => Payload::from(Frame { storage: buffer, interp }),
        };

        Ok(payload)
    }

    fn get_caps(&self) -> Caps { Caps { frame_count: None, is_live: true } }
}
