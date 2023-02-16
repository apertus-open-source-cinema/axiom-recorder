use crate::{
    nodes_gpu::base_gpu_node::{BindingValue, GpuNode},
    pipeline_processing::frame::{ColorInterpretation, FrameInterpretation, SampleInterpretation},
};
use anyhow::{bail, Result};

use std::collections::HashMap;


#[derive(Default)]
pub struct Debayer {}
impl GpuNode for Debayer {
    fn get_glsl(&self) -> String { include_str!("./debayer.glsl").to_string() }

    fn get_binding(
        &self,
        frame_interpretation: &FrameInterpretation,
    ) -> Result<HashMap<String, BindingValue>> {
        match frame_interpretation.color_interpretation {
            ColorInterpretation::Bayer(cfa) => Ok(HashMap::from([
                ("cfa.red_in_first_col".to_string(), BindingValue::U32(cfa.red_in_first_col as _)),
                ("cfa.red_in_first_row".to_string(), BindingValue::U32(cfa.red_in_first_row as _)),
            ])),
            unsupported => bail!("expected bayer input found {unsupported:?}"),
        }
    }

    fn get_interpretation(&self, frame_interpretation: FrameInterpretation) -> FrameInterpretation {
        FrameInterpretation {
            color_interpretation: ColorInterpretation::Rgb,
            sample_interpretation: SampleInterpretation::UInt(8),
            ..frame_interpretation
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Debayer;
    use crate::{
        nodes_gpu::base_gpu_node::GpuNodeImpl,
        nodes_util::null_source::NullFrameSource,
        pipeline_processing::{
            frame::{
                CfaDescriptor,
                ColorInterpretation,
                Compression,
                FrameInterpretation,
                SampleInterpretation,
            },
            node::{InputProcessingNode, NodeID, ProcessingNode, Request},
            parametrizable::{prelude::NodeInputValue, Parameterizable, Parameters},
            processing_context::ProcessingContext,
        },
    };
    use std::{collections::HashMap, sync::Arc};

    #[test]
    fn test_basic_functionality_debayer() {
        let context = ProcessingContext::default();

        let source = NodeInputValue(InputProcessingNode::new(
            NodeID::default(),
            Arc::new(NullFrameSource {
                context: context.clone(),
                interpretation: FrameInterpretation {
                    width: 1920,
                    height: 1080,
                    fps: Some(24.0),
                    color_interpretation: ColorInterpretation::Bayer(CfaDescriptor {
                        red_in_first_col: false,
                        red_in_first_row: false,
                    }),
                    sample_interpretation: SampleInterpretation::FP16,
                    compression: Compression::Uncompressed,
                },
            }),
        ));
        let parameters = Parameters::new(HashMap::from([("input".to_string(), source)]));
        let dut = GpuNodeImpl::<Debayer>::from_parameters(parameters, &[], &context).unwrap();

        for _ in 0..10 {
            let _payload = pollster::block_on(dut.pull(Request::new(0, 0))).unwrap();
        }
    }
}
