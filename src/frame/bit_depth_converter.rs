use crate::{
    frame::raw_frame::RawFrame,
    pipeline_processing::{
        parametrizable::{
            ParameterType,
            ParameterTypeDescriptor::Optional,
            ParameterValue,
            Parameterizable,
            Parameters,
            ParametersDescriptor,
        },
        processing_node::{Payload, ProcessingNode},
    },
};
use anyhow::{Context, Result};

pub struct BitDepthConverter(u64);
impl Parameterizable for BitDepthConverter {
    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new().with(
            "target-bits",
            Optional(ParameterType::IntRange(8, 8), ParameterValue::IntRange(8)),
        )
    }

    fn from_parameters(parameters: &Parameters) -> Result<Self>
    where
        Self: Sized,
    {
        Ok(Self(parameters.get("target-bits")?))
    }
}
impl ProcessingNode for BitDepthConverter {
    fn process(&self, input: &mut Payload) -> Result<Option<Payload>> {
        let frame = input.downcast::<RawFrame>().context("Wrong input format")?;
        Ok(Some(Payload::from(frame.convert_to_8_bit())))
    }
}
