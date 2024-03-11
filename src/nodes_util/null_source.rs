use crate::pipeline_processing::{
    frame::{Frame, FrameInterpretation},
    node::{Caps, NodeID, ProcessingNode, Request},
    parametrizable::{Parameterizable, Parameters, ParametersDescriptor},
    payload::Payload,
    processing_context::ProcessingContext,
};
use async_trait::async_trait;

#[derive(Clone)]
pub struct NullFrameSource {
    pub context: ProcessingContext,
    pub interpretation: FrameInterpretation,
}

impl Parameterizable for NullFrameSource {
    const DESCRIPTION: Option<&'static str> =
        Some("returns frames with the specified interpretation where all bytes are zero");

    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new().with_interpretation()
    }
    fn from_parameters(
        mut parameters: Parameters,
        _is_input_to: &[NodeID],
        context: &ProcessingContext,
    ) -> anyhow::Result<Self>
    where
        Self: Sized,
    {
        Ok(Self { context: context.clone(), interpretation: parameters.get_interpretation()? })
    }
}

#[async_trait]
impl ProcessingNode for NullFrameSource {
    async fn pull(&self, _request: Request) -> anyhow::Result<Payload> {
        let buffer = unsafe {
            let mut buffer =
                self.context.get_uninit_cpu_buffer(self.interpretation.required_bytes());
            buffer.as_mut_slice(|buffer| {
                buffer.as_mut_ptr().write_bytes(0, buffer.len());
            });
            buffer
        };

        let payload =
            Payload::from(Frame { storage: buffer, interpretation: self.interpretation.clone() });
        Ok(payload)
    }
    fn get_caps(&self) -> Caps { Caps { frame_count: None, random_access: true } }
}
