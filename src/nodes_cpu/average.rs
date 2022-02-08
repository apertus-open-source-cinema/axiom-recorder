use crate::pipeline_processing::{
    parametrizable::{Parameterizable, Parameters, ParametersDescriptor},
    payload::Payload,
};
use anyhow::{Context, Result};
use std::sync::Arc;

use crate::pipeline_processing::{
    frame::{Frame, Raw},
    node::{Caps, ProcessingNode},
    parametrizable::{ParameterType, ParameterTypeDescriptor},
    processing_context::ProcessingContext,
};
use async_trait::async_trait;
use futures::stream::FuturesUnordered;
use futures::{StreamExt, FutureExt};
use crate::pipeline_processing::buffers::ChunkedCpuBuffer;
use crate::util::async_notifier::AsyncNotifier;

pub struct Average {
    input: Arc<dyn ProcessingNode + Send + Sync>,
    n: usize,
    last_frame_info: AsyncNotifier<(u64, u64)>,
}
impl Parameterizable for Average {
    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new()
            .with("input", ParameterTypeDescriptor::Mandatory(ParameterType::NodeInput))
            .with("n", ParameterTypeDescriptor::Mandatory(ParameterType::IntRange(1, 1_000_000)))
    }

    fn from_parameters(parameters: &Parameters, _context: &ProcessingContext) -> Result<Self> {
        Ok(Self {
            input: parameters.get("input")?,
            n: parameters.get::<i64>("n")? as usize,
            last_frame_info: Default::default(),
        })
    }
}

#[async_trait]
impl ProcessingNode for Average {
    async fn pull(&self, frame_number: u64, context: &ProcessingContext) -> Result<Payload> {
        self.last_frame_info.wait(move |(next, _)| *next == frame_number).await;
        let total_offset = self.last_frame_info.get().1;
        let mut n = (self.n as u64) * frame_number + total_offset;
        let input = loop {
            let input = self.input.pull(n, context).await;
            match input {
                Ok(input) => break input,
                Err(e) => {
                    println!("An error occured for {n}: {e}");
                    n += 1
                }
            }
        };
        let frame = context.ensure_cpu_buffer::<Raw>(&input).context("Wrong input format")?;
        let interp = frame.interp;
        assert_eq!(frame.interp.bit_depth, 12);

        // u32 -> 4 bytes per pixel
        let out_buffer = unsafe { context.get_uninit_cpu_buffer((interp.height * interp.width * 4) as usize) };

        let out = Arc::new(ChunkedCpuBuffer::new(out_buffer, 8));

        frame.storage.as_slice_async(|frame: &[u8]| {
            let out = out.clone();
            async move {
                out.zip_with(frame, |out, frame| {
                    let out: &mut [u32] = bytemuck::cast_slice_mut(out);
                    for (out, frame) in out.chunks_exact_mut(2).zip(frame.chunks_exact(3)) {
                        out[0] = (((frame[0] as u16) << 4) | (frame[1] as u16 >> 4)) as u32;
                        out[1] = ((((frame[1] & 0xf) as u16) << 8) | (frame[2] as u16)) as u32;
                    }
                }).await;
            }
        }.boxed()).await;

        macro_rules! spawn {
            ($i:expr) => {{
                let input = self.input.clone();
                let context_copy = context.clone();
                let out = out.clone();
                context.spawn(async move {
                    let input = input.pull($i, &context_copy).await;
                    match &input {
                        Err(e) => {
                            println!("An error occured for {}: {e}", $i);
                        },
                        _ => {}
                    }
                    let input = input?;
                    let frame = context_copy.ensure_cpu_buffer::<Raw>(&input).context("Wrong input format")?;
                    assert_eq!(interp.bit_depth, 12);

                    frame.storage.as_slice_async(|frame: &[u8]| async move {
                        out.zip_with(frame, |out, frame| {
                            let out: &mut [u32] = bytemuck::cast_slice_mut(out);
                            for (out, frame) in out.chunks_exact_mut(2).zip(frame.chunks_exact(3)) {
                                out[0] += (((frame[0] as u16) << 4) | (frame[1] as u16 >> 4)) as u32;
                                out[1] += ((((frame[1] & 0xf) as u16) << 8) | (frame[2] as u16)) as u32;
                            }
                        }).await;
                    }.boxed()).await;

                    anyhow::Result::<_, anyhow::Error>::Ok($i)
                })
            }};
        }

        let chunk_size = self.n;
        let mut futs = (1u64..self.n as u64).into_iter().map(|i| spawn!(n + i).boxed()).collect::<FuturesUnordered<_>>();

        while let Some(res) = futs.next().await {
            match res {
                Err(e) => {
                    futs.push(spawn!(n + chunk_size as u64).boxed());
                    n += 1;
                }
                _ => {}
            }
        }

        self.last_frame_info.update(move |(next, total_offset)| {
            *next = frame_number + 1;
            *total_offset = n - (self.n as u64) * frame_number;
        });

        let mut out_buffer = match Arc::try_unwrap(out) {
            Ok(buf) => buf.unchunk(),
            Err(_) => return Err(anyhow::anyhow!("could not get out_buffer back"))
        };

        out_buffer.as_mut_slice(|out| {
            let out: &mut [u32] = bytemuck::cast_slice_mut(out);
            for val in out {
                *val = bytemuck::cast((*val as f32) / (self.n as f32));
            }
        });

        let mut interp = interp;
        interp.bit_depth = 32;
        let new_frame = Frame { storage: out_buffer, interp };

        Ok(Payload::from(new_frame))
    }

    fn get_caps(&self) -> Caps {
        let caps = self.input.get_caps();
        Caps {
            frame_count: caps.frame_count.map(|v| v / self.n as u64),
            is_live: caps.is_live
        }
    }
}
