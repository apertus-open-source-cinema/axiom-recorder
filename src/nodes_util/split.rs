use crate::pipeline_processing::{
    node::InputProcessingNode,
    parametrizable::{Parameterizable, Parameters, ParametersDescriptor},
    payload::Payload,
};
use anyhow::Result;


use crate::pipeline_processing::{
    node::{Caps, NodeID, ProcessingNode},
    parametrizable::{ParameterType, ParameterTypeDescriptor},
    processing_context::ProcessingContext,
};
use async_trait::async_trait;


pub struct Split {
    input: InputProcessingNode,
    elem: i64,
}

impl Parameterizable for Split {
    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new()
            .with("input", ParameterTypeDescriptor::Mandatory(ParameterType::NodeInput))
            .with(
                "element",
                ParameterTypeDescriptor::Mandatory(ParameterType::IntRange(0, i64::MAX)),
            )
    }

    fn from_parameters(
        mut parameters: Parameters,
        _is_input_to: &[NodeID],
        _context: &ProcessingContext,
    ) -> Result<Self> {
        Ok(Self { input: parameters.get("input")?, elem: parameters.get("element")? })
    }
}

#[async_trait]
impl ProcessingNode for Split {
    async fn pull(
        &self,
        frame_number: u64,
        _puller_id: NodeID,
        context: &ProcessingContext,
    ) -> Result<Payload> {
        let frame = self.input.pull(frame_number, context).await?;
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
