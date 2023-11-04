use crate::pipeline_processing::{
    frame::Raw, node::InputProcessingNode, parametrizable::{Parameterizable, Parameters, ParametersDescriptor}, payload::Payload
};
use anyhow::{Context, Result};


use crate::pipeline_processing::{
    frame::{Frame, FrameInterpretation},
    node::{Caps, NodeID, ProcessingNode, Request},
    parametrizable::prelude::*,
    processing_context::ProcessingContext,
};
use async_trait::async_trait;

pub struct Fp32ToUInt16 {
    input: InputProcessingNode,
    context: ProcessingContext,
    multiplier: f64,
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
            .ensure_cpu_buffer::<Raw>(&input)
            .context("Wrong input format for FPotUint16")?;
        let interp = Raw { bit_depth: 16, ..frame.interp };

        let mut new_buffer =
            unsafe { self.context.get_uninit_cpu_buffer(interp.required_bytes()) };

        new_buffer.as_mut_slice(|new_buffer| {
            frame.storage.as_slice(|frame_storage| {
                let fp_storage: &[f32] = bytemuck::cast_slice(frame_storage);
                let uint_storage: &mut [u16] = bytemuck::cast_slice_mut(new_buffer);

                for (input, output) in fp_storage.iter().zip(uint_storage.iter_mut()) {
                    *output = (*input * self.multiplier as f32).round() as u16;
                }
            })
        });
        
        let new_frame = Frame { storage: new_buffer, interp };

        Ok(Payload::from(new_frame))
    }

    fn get_caps(&self) -> Caps { self.input.get_caps() }
}
