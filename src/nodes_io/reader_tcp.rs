use crate::{
    pipeline_processing::{
        frame::{Frame, FrameInterpretation},
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
    interpretation: FrameInterpretation,
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
            interpretation: parameters.get_interpretation()?,
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
            unsafe { self.context.get_uninit_cpu_buffer(self.interpretation.required_bytes()) };
        buffer
            .as_mut_slice(|slice| self.tcp_connection.lock().unwrap().read_exact(slice))
            .context(EOFError)?;

        self.notifier.update(|x| *x = frame_number + 1);

        let payload = Payload::from(Frame { storage: buffer, interpretation: self.interpretation });

        Ok(payload)
    }

    fn get_caps(&self) -> Caps { Caps { frame_count: None, random_access: false } }
}
