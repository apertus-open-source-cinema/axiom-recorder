use crate::pipeline_processing::processing_node::{ProcessingNode, Payload};
use crate::pipeline_processing::parametrizable::{Parameterizable, Parameters, ParametersDescriptor, ParameterType, ParameterValue};
use crate::pipeline_processing::parametrizable::ParameterTypeDescriptor::Optional;
use anyhow::{Result, Context};
use crate::frame::raw_frame::RawFrame;

pub struct BitDepthConverter(u64);
impl Parameterizable for BitDepthConverter {
    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new()
            .with("target-bits", Optional(ParameterType::IntRange(8, 8), ParameterValue::IntRange(8)))
    }

    fn from_parameters(parameters: &Parameters) -> Result<Self> where
        Self: Sized {
        Ok(Self(parameters.get("target-bits")?))
    }
}
impl ProcessingNode for BitDepthConverter {
    fn process(&self, input: &mut Payload) -> Result<Option<Payload>> {
        let frame = input.downcast::<RawFrame>().context("Wrong input format")?;
        Ok(Some(Payload::from(frame.convert_to_8_bit())))
    }
}