use crate::pipeline_processing::{
    frame::{Frame, Raw, Rgb, SZ3Compressed},
    node::{Caps, InputProcessingNode, NodeID, ProcessingNode, Request},
    parametrizable::prelude::*,
    payload::Payload,
    processing_context::ProcessingContext,
};
use anyhow::{Context, Result};
use async_trait::async_trait;
use std::sync::Arc;

enum DataType {
    F32,
    F64,
    I64,
    I32,
}

pub struct SZ3Compress {
    input: InputProcessingNode,
    dims: Option<Vec<i64>>,
    error_bound: sz3::ErrorBound,
    data_type: DataType,
    context: ProcessingContext,
}
impl Parameterizable for SZ3Compress {
    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new()
            .with("input", Mandatory(NodeInputParameter))
            .with("tolerance", Mandatory(PositiveReal()))
            .with("error_control", Mandatory(StringParameter))
            .with("data_type", Mandatory(StringParameter))
            .with(
                "dims",
                WithDefault(ListParameter(Box::new(IntRange(-1, i64::MAX))), ListValue(vec![])),
            )
    }

    fn from_parameters(
        mut parameters: Parameters,
        _is_input_to: &[NodeID],
        context: &ProcessingContext,
    ) -> Result<Self> {
        let tolerance = parameters.take("tolerance")?;
        let error_bound = match &*parameters.take::<String>("error_control")?.to_lowercase() {
            "abs" => Ok(sz3::ErrorBound::Absolute(tolerance)),
            "rel" => Ok(sz3::ErrorBound::Relative(tolerance)),
            "l2norm" => Ok(sz3::ErrorBound::L2Norm(tolerance)),
            "psnr" => Ok(sz3::ErrorBound::PSNR(tolerance)),
            other => Err(anyhow::anyhow!("unknown error control {other}")),
        }?;

        let data_type = match &*parameters.take::<String>("data_type")?.to_lowercase() {
            "float" | "f32" => Ok(DataType::F32),
            "double" | "f64" => Ok(DataType::F64),
            "int" | "i32" => Ok(DataType::I32),
            "long" | "i64" => Ok(DataType::I64),
            other => Err(anyhow::anyhow!("unknown data type {other}")),
        }?;

        let dims = parameters.take_vec("dims")?;
        if let Some(pos) = dims.iter().position(|v| *v == -1) {
            if pos + 1 != dims.len() {
                return Err(anyhow::anyhow!(
                    "remaining dim (-1) can only be specified in the last position"
                ));
            }
        }

        let dims = if dims.is_empty() { None } else { Some(dims) };
        Ok(Self {
            input: parameters.take("input")?,
            dims,
            error_bound,
            data_type,
            context: context.clone(),
        })
    }
}

#[async_trait]
impl ProcessingNode for SZ3Compress {
    async fn pull(&self, request: Request) -> Result<Payload> {
        let input = self.input.pull(request).await?;
        let (bytes, frame_dims, interp) =
            if let Ok(frame) = self.context.ensure_cpu_buffer::<Raw>(&input) {
                (
                    frame.storage.clone(),
                    vec![frame.interp.width as _, frame.interp.height as _],
                    Arc::new(frame.interp) as Arc<_>,
                )
            } else {
                let frame =
                    self.context.ensure_cpu_buffer::<Rgb>(&input).context("Wrong input format")?;
                (
                    frame.storage.clone(),
                    vec![3, frame.interp.width as _, frame.interp.height as _],
                    Arc::new(frame.interp) as Arc<_>,
                )
            };

        let dims = self.dims.clone().unwrap_or(frame_dims);

        let compressed = bytes.as_slice(|data| {
            macro_rules! compress {
                ($ty:ty) => {{
                    let data: &[$ty] = bytemuck::cast_slice(data);
                    let mut builder = sz3::DimensionedData::build(&data);
                    let add_remainder_dim = *dims.last().unwrap() == -1;
                    for dim in dims {
                        if dim == -1 {
                            break;
                        } else {
                            builder = builder.dim(dim as _)?;
                        }
                    }
                    let data = if add_remainder_dim {
                        builder.remainder_dim()?
                    } else {
                        builder.finish()?
                    };

                    anyhow::Result::<_, anyhow::Error>::Ok(sz3::compress(&data, self.error_bound)?)
                }};
            }

            match self.data_type {
                DataType::F64 => compress!(f64),
                DataType::F32 => compress!(f32),
                DataType::I64 => compress!(i64),
                DataType::I32 => compress!(i32),
            }
        })?;

        let buffer = unsafe {
            let mut buffer = self.context.get_uninit_cpu_buffer(compressed.len());
            buffer.as_mut_slice(|data| {
                data.copy_from_slice(&*compressed);
            });
            buffer
        };

        let new_frame = Frame { interp: SZ3Compressed::new(interp, buffer.len()), storage: buffer };

        Ok(Payload::from(new_frame))
    }

    fn get_caps(&self) -> Caps { self.input.get_caps() }
}
