use crate::pipeline_processing::{
    node::{Caps, InputProcessingNode, NodeID, ProcessingNode, Request},
    parametrizable::{prelude::*, Parameterizable, Parameters, ParametersDescriptor},
    payload::Payload,
    processing_context::ProcessingContext,
};
use anyhow::Result;
use async_trait::async_trait;


pub struct Split {
    input: InputProcessingNode,
    elem: i64,
}

impl Parameterizable for Split {
    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new()
            .with("input", Mandatory(NodeInputParameter))
            .with("element", Mandatory(IntRange(0, i64::MAX)))
    }

    fn from_parameters(
        mut parameters: Parameters,
        _is_input_to: &[NodeID],
        _context: &ProcessingContext,
    ) -> Result<Self> {
        Ok(Self { input: parameters.take("input")?, elem: parameters.take("element")? })
    }
}

#[async_trait]
impl ProcessingNode for Split {
    async fn pull(&self, request: Request) -> Result<Payload> {
        let frame = self.input.pull(request).await?;
        let payloads = frame.downcast::<Vec<Payload>>()?;
        Ok(payloads
            .get(self.elem as usize)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "tried to get element {} of {payloads:?} but it did not exists",
                    self.elem
                )
            })?
            .clone())
    }

    fn get_caps(&self) -> Caps { self.input.get_caps() }
}
