use crate::{
    nodes_gpu::base_gpu_node::{BindingValue, GpuNode},
    pipeline_processing::{
        frame::{ColorInterpretation, FrameInterpretation},
        node::NodeID,
        parametrizable::prelude::*,
        processing_context::ProcessingContext,
    },
};
use anyhow::{bail, Result};
use std::collections::HashMap;


pub struct ColorVoodoo {
    pedestal: f32,
    s_gamma: f32,
    v_gamma: f32,
}

impl Parameterizable for ColorVoodoo {
    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new()
            .with("pedestal", WithDefault(FloatRange(0.0, 1.0), FloatRangeValue(0.0)))
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
            pedestal: parameters.take("pedestal")?,
            s_gamma: parameters.take("s_gamma")?,
            v_gamma: parameters.take("v_gamma")?,
        })
    }
}

impl GpuNode for ColorVoodoo {
    fn get_glsl(&self) -> String { include_str!("./color_voodoo.glsl").to_string() }

    fn get_binding(
        &self,
        frame_interpretation: &FrameInterpretation,
    ) -> Result<HashMap<String, BindingValue>> {
        if frame_interpretation.color_interpretation != ColorInterpretation::Rgb {
            bail!("color_voodo node only supports rgb images")
        }

        Ok(HashMap::from([
            ("pedestal".to_string(), BindingValue::F32(self.pedestal)),
            ("s_gamma".to_string(), BindingValue::F32(self.s_gamma)),
            ("v_gamma".to_string(), BindingValue::F32(self.v_gamma)),
        ]))
    }
}


#[cfg(test)]
mod tests {
    use super::ColorVoodoo;
    use crate::{
        nodes_gpu::base_gpu_node::GpuNodeImpl,
        nodes_util::null_source::NullFrameSource,
        pipeline_processing::{
            frame::{ColorInterpretation, Compression, FrameInterpretation, SampleInterpretation},
            node::{InputProcessingNode, NodeID, ProcessingNode, Request},
            parametrizable::{prelude::NodeInputValue, Parameterizable, Parameters},
            processing_context::ProcessingContext,
        },
    };
    use std::{collections::HashMap, sync::Arc};

    #[test]
    fn test_basic_functionality_color_voodo() {
        let context = ProcessingContext::default();

        let source = NodeInputValue(InputProcessingNode::new(
            NodeID::default(),
            Arc::new(NullFrameSource {
                context: context.clone(),
                interpretation: FrameInterpretation {
                    width: 1920,
                    height: 1080,
                    fps: Some(24.0),
                    color_interpretation: ColorInterpretation::Rgb,
                    sample_interpretation: SampleInterpretation::FP16,
                    compression: Compression::Uncompressed,
                },
            }),
        ));
        let parameters = Parameters::new(HashMap::from([("input".to_string(), source)]))
            .add_defaults(GpuNodeImpl::<ColorVoodoo>::describe_parameters());
        let dut = GpuNodeImpl::<ColorVoodoo>::from_parameters(parameters, &[], &context).unwrap();

        for _ in 0..10 {
            let _payload = pollster::block_on(dut.pull(Request::new(0, 0))).unwrap();
        }
    }
}
