use crate::pipeline_processing::{
    node::NodeID,
    parametrizable::{ParameterValue, Parameterizable, Parameters, ParametersDescriptor},
    payload::Payload,
};
use anyhow::{Context, Result};
use std::sync::Arc;

use crate::{
    pipeline_processing::{
        buffers::ChunkedCpuBuffer,
        frame::{Frame, Raw},
        node::{Caps, InputProcessingNode, ProcessingNode},
        parametrizable::{ParameterType, ParameterTypeDescriptor},
        processing_context::ProcessingContext,
    },
    util::async_notifier::AsyncNotifier,
};
use async_trait::async_trait;
use futures::{stream::FuturesUnordered, FutureExt, StreamExt};

pub struct Average {
    input: InputProcessingNode,
    num_frames: usize,
    last_frame_info: AsyncNotifier<(u64, u64)>,
    produce_std: bool,
}
impl Parameterizable for Average {
    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new()
            .with("input", ParameterTypeDescriptor::Mandatory(ParameterType::NodeInput))
            .with("n", ParameterTypeDescriptor::Mandatory(ParameterType::IntRange(1, 1_000_000)))
            .with(
                "std",
                ParameterTypeDescriptor::Optional(
                    ParameterType::BoolParameter,
                    ParameterValue::BoolParameter(false),
                ),
            )
    }

    fn from_parameters(
        mut parameters: Parameters,
        _is_input_to: &[NodeID],
        _context: &ProcessingContext,
    ) -> Result<Self> {
        Ok(Self {
            input: parameters.take("input")?,
            num_frames: parameters.take::<i64>("n")? as usize,
            produce_std: parameters.take("std")?,
            last_frame_info: Default::default(),
        })
    }
}

#[async_trait]
impl ProcessingNode for Average {
    async fn pull(
        &self,
        frame_number: u64,
        _puller_id: NodeID,
        context: &ProcessingContext,
    ) -> Result<Payload> {
        let (_, total_offset) =
            self.last_frame_info.wait(move |(next, _)| *next == frame_number).await;
        let mut n = (self.num_frames as u64) * frame_number + total_offset;
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

        // println!("[{frame_number}] adding {n}");

        // f32 -> 4 bytes per pixel
        let out_buffer_avg =
            unsafe { context.get_uninit_cpu_buffer((interp.height * interp.width * 4) as usize) };

        let out_buffer_std =
            unsafe { context.get_uninit_cpu_buffer((interp.height * interp.width * 4) as usize) };

        let out = Arc::new(ChunkedCpuBuffer::<usize, 2>::new(
            [out_buffer_avg, out_buffer_std],
            context.num_threads(),
        ));

        frame
            .storage
            .as_slice_async(|frame: &[u8]| {
                {
                    let out = out.clone();
                    async move {
                        out.zip_with(frame, |[avg, std], frame, count| {
                            let avg: &mut [f32] = bytemuck::cast_slice_mut(avg);
                            let std: &mut [f32] = bytemuck::cast_slice_mut(std);

                            for ((avg, std), frame) in avg
                                .chunks_exact_mut(2)
                                .zip(std.chunks_exact_mut(2))
                                .zip(frame.chunks_exact(3))
                            {
                                avg[0] = (((frame[0] as u16) << 4) | (frame[1] as u16 >> 4)) as f32;
                                avg[1] =
                                    ((((frame[1] & 0xf) as u16) << 8) | (frame[2] as u16)) as f32;
                                std[0] = 0.;
                                std[1] = 0.;
                            }
                            *count = 1;
                        })
                        .await;
                    }
                }
                .boxed()
            })
            .await;

        macro_rules! spawn {
            ($i:expr) => {{
                let input = self.input.clone_for_same_puller();
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
                        out.zip_with(frame, |[avg, std], frame, count| {
                            let avg: &mut [f32] = bytemuck::cast_slice_mut(avg);
                            let std: &mut [f32] = bytemuck::cast_slice_mut(std);

                            *count += 1;
                            let count = *count as f32;

                            for ((avg, std), frame) in avg.chunks_exact_mut(2).zip(std.chunks_exact_mut(2)).zip(frame.chunks_exact(3)) {
                                let value = (((frame[0] as u16) << 4) | (frame[1] as u16 >> 4)) as f32;
                                let delta = value - avg[0];
                                avg[0] += delta / count;
                                std[0] += (value - avg[0]) * delta;


                                let value = ((((frame[1] & 0xf) as u16) << 8) | (frame[2] as u16)) as f32;
                                let delta = value - avg[1];
                                avg[1] += delta / count;
                                std[1] += (value - avg[1]) * delta;
                            }
                        }).await;
                    }.boxed()).await;
                    // println!("[{frame_number}] adding {}", $i);

                    anyhow::Result::<_, anyhow::Error>::Ok($i)
                })
            }};
        }

        let to_spawn = self.num_frames.min(num_cpus::get() + 1) as u64;
        let mut futs = (1u64..to_spawn)
            .into_iter()
            .map(|i| spawn!(n + i).boxed())
            .collect::<FuturesUnordered<_>>();
        let mut spawned = to_spawn;
        let mut limit = self.num_frames as u64;
        let mut next = n + to_spawn;

        while let Some(res) = futs.next().await {
            if res.is_err() {
                n += 1;
                limit += 1
            }

            if spawned < limit {
                futs.push(spawn!(next as u64).boxed());
                spawned += 1;
                next += 1;
            }
        }

        self.last_frame_info.update(move |(next, total_offset)| {
            *next = frame_number + 1;
            *total_offset = n - (self.num_frames as u64) * frame_number;
        });

        let [avg_buffer, mut std_buffer] = match Arc::try_unwrap(out) {
            Ok(buf) => buf.unchunk(),
            Err(_) => return Err(anyhow::anyhow!("could not get out_buffer back")),
        };

        std_buffer.as_mut_slice(move |buf| {
            let buf: &mut [f32] = bytemuck::cast_slice_mut(buf);
            for i in buf {
                *i /= self.num_frames as f32;
            }
        });

        let mut interp = interp;
        interp.bit_depth = 32;
        let avg_frame = Frame { storage: avg_buffer, interp };
        let std_frame = Frame { storage: std_buffer, interp };

        if self.produce_std {
            Ok(Payload::from(vec![Payload::from(avg_frame), Payload::from(std_frame)]))
        } else {
            Ok(Payload::from(avg_frame))
        }
    }

    fn get_caps(&self) -> Caps {
        let caps = self.input.get_caps();
        Caps {
            frame_count: caps.frame_count.map(|v| v / self.num_frames as u64),
            is_live: caps.is_live,
        }
    }
}
