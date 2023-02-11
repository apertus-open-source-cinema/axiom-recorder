use crate::{
    nodes_gpu::base_gpu_node::{GpuNode, PushConstantValue},
    pipeline_processing::{
        frame::{FrameInterpretation},
        node::{NodeID},
        parametrizable::prelude::*,
        processing_context::ProcessingContext,
    },
};
use anyhow::{Result};

use std::{collections::HashMap};


pub struct ColorVoodoo {
    pedestal: u8,
    s_gamma: f64,
    v_gamma: f64,
}

impl Parameterizable for ColorVoodoo {
    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new()
            .with("pedestal", WithDefault(U8(), IntRangeValue(8)))
            .with("s_gamma", WithDefault(FloatRange(0.0, 100.0), FloatRangeValue(1.0)))
            .with("v_gamma", WithDefault(FloatRange(0.0, 100.0), FloatRangeValue(1.0)))
    }
    fn from_parameters(
        mut parameters: Parameters,
        _is_input_to: &[NodeID],
        _context: &ProcessingContext,
    ) -> Result<Self>
    where
        Self: Sized,
    {
        Ok(ColorVoodoo {
            pedestal: parameters.take::<u64>("pedestal")? as u8,
            s_gamma: parameters.take("s_gamma")?,
            v_gamma: parameters.take("v_gamma")?,
        })
    }
}

impl GpuNode for ColorVoodoo {
    fn get_glsl(&self) -> String { include_str!("./debayer.glsl").to_string() }

    fn get_binding(
        &self,
        _frame_interpretation: &FrameInterpretation,
    ) -> Result<HashMap<String, PushConstantValue>> {
        todo!()
    }
}
