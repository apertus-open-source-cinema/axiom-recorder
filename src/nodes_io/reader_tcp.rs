use crate::{
    pipeline_processing::{
        frame::{Frame, FrameInterpretation, FrameInterpretations},
        node::{Caps, EOFError, NodeID, ProcessingNode, Request},
        parametrizable::prelude::*,
        payload::Payload,
        processing_context::ProcessingContext,
    },
    util::async_notifier::AsyncNotifier,
};
use anyhow::{Context, Result};
use async_trait::async_trait;
use std::{io::Read, net::TcpStream, sync::Mutex};

pub struct TcpReader {
    tcp_connection: Mutex<TcpStream>,
    interp: FrameInterpretations,
    notifier: AsyncNotifier<u64>,
    context: ProcessingContext,
}
impl Parameterizable for TcpReader {
    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new()
            .with("address", Mandatory(StringParameter))
            .with_interpretation()
    }

    fn from_parameters(
        mut parameters: Parameters,
        _is_input_to: &[NodeID],
        context: &ProcessingContext,
    ) -> Result<Self>
    where
        Self: Sized,
    {
        Ok(Self {
            tcp_connection: Mutex::new(TcpStream::connect(parameters.take::<String>("address")?)?),
            interp: parameters.get_interpretation()?,
            notifier: Default::default(),
            context: context.clone(),
        })
    }
}

#[async_trait]
impl ProcessingNode for TcpReader {
    async fn pull(&self, request: Request) -> Result<Payload> {
        let frame_number = request.frame_number();

        self.notifier.wait(move |x| *x >= frame_number).await;

        let mut buffer =
            unsafe { self.context.get_uninit_cpu_buffer(self.interp.required_bytes()) };
        buffer
            .as_mut_slice(|slice| self.tcp_connection.lock().unwrap().read_exact(slice))
            .context(EOFError)?;

        self.notifier.update(|x| *x = frame_number + 1);

        let payload = match self.interp {
            FrameInterpretations::Raw(interp) => Payload::from(Frame { storage: buffer, interp }),
            FrameInterpretations::Rgb(interp) => Payload::from(Frame { storage: buffer, interp }),
            FrameInterpretations::Rgba(interp) => Payload::from(Frame { storage: buffer, interp }),
        };

        Ok(payload)
    }

    fn get_caps(&self) -> Caps { Caps { frame_count: None, random_access: false } }
}
