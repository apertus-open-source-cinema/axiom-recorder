use crate::pipeline_processing::{
    node::InputProcessingNode,
    parametrizable::{Parameterizable, Parameters, ParametersDescriptor},
    payload::Payload,
};
use anyhow::{bail, Context, Result};


use crate::pipeline_processing::{
    frame::{Frame, FrameInterpretation, SampleInterpretation},
    node::{Caps, NodeID, ProcessingNode, Request},
    parametrizable::prelude::*,
    processing_context::ProcessingContext,
};
use async_trait::async_trait;

pub struct Fp32ToUInt16 {
    input: InputProcessingNode,
    context: ProcessingContext,
    multiplier: f32,
}
impl Parameterizable for Fp32ToUInt16 {
    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new()
            .with("input", Mandatory(NodeInputParameter))
            .with("multiplier", WithDefault(PositiveReal(), FloatRangeValue(1.0)))
    }

    fn from_parameters(
        mut parameters: Parameters,
        _is_input_to: &[NodeID],
        context: &ProcessingContext,
    ) -> Result<Self> {
        Ok(Self {
            input: parameters.take("input")?,
            multiplier: parameters.take("multiplier")?,
            context: context.clone(),
        })
    }
}

#[async_trait]
impl ProcessingNode for Fp32ToUInt16 {
    async fn pull(&self, request: Request) -> Result<Payload> {
        let input = self.input.pull(request).await?;
        let frame = self
            .context
            .ensure_cpu_buffer_frame(&input)
            .context("Wrong input format for FPotUint16")?;
        let interpretation = FrameInterpretation {
            sample_interpretation: SampleInterpretation::UInt(16),
            ..frame.interpretation.clone()
        };
        let mut new_buffer =
            unsafe { self.context.get_uninit_cpu_buffer(interpretation.required_bytes()) };

        if let SampleInterpretation::FP32 = frame.interpretation.sample_interpretation {
            new_buffer.as_mut_slice(|new_buffer| {
                frame.storage.as_slice(|frame_storage| {
                    let fp_storage: &[f32] = bytemuck::cast_slice(frame_storage);
                    let uint_storage: &mut [u16] = bytemuck::cast_slice_mut(new_buffer);

                    for (input, output) in fp_storage.iter().zip(uint_storage.iter_mut()) {
                        *output = (*input * self.multiplier).round() as u16;
                    }
                })
            });
        } else {
            bail!("only fp32 is a legal input to the Fp32ToUInt16 Node!")
        }

        let new_frame = Frame { storage: new_buffer, interpretation };

        Ok(Payload::from(new_frame))
    }

    fn get_caps(&self) -> Caps { self.input.get_caps() }
}
