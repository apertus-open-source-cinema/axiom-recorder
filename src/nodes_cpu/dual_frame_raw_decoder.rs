use crate::pipeline_processing::{
    node::InputProcessingNode,
    parametrizable::{Parameterizable, Parameters, ParametersDescriptor},
    payload::Payload,
};
use anyhow::{anyhow, Context, Result};
use std::sync::Arc;

use crate::{
    pipeline_processing::{
        buffers::CpuBuffer,
        frame::{CfaDescriptor, Frame, FrameInterpretation, Raw, Rgb},
        node::{Caps, NodeID, ProcessingNode},
        parametrizable::{
            ParameterType,
            ParameterTypeDescriptor,
            ParameterTypeDescriptor::Optional,
            SerdeParameterValue,
        },
        processing_context::ProcessingContext,
    },
    util::async_notifier::AsyncNotifier,
};
use async_trait::async_trait;
use futures::join;

const FRAME_A_MARKER: u8 = 0xAA;

#[derive(Clone, Default)]
struct LastFrameInfo(u64, u64, u8, Option<Arc<Frame<Rgb, CpuBuffer>>>);

pub struct DualFrameRawDecoder {
    input: InputProcessingNode,
    cfa_descriptor: CfaDescriptor,
    last_frame_info: AsyncNotifier<LastFrameInfo>,
    debug: bool,
}
impl Parameterizable for DualFrameRawDecoder {
    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new()
            .with("input", ParameterTypeDescriptor::Mandatory(ParameterType::NodeInput))
            .with(
                "debug",
                ParameterTypeDescriptor::Optional(
                    ParameterType::BoolParameter,
                    SerdeParameterValue::BoolParameter(false),
                ),
            )
            .with(
                "red-in-first-col",
                Optional(ParameterType::BoolParameter, SerdeParameterValue::BoolParameter(true)),
            )
            .with(
                "red-in-first-row",
                Optional(ParameterType::BoolParameter, SerdeParameterValue::BoolParameter(false)),
            )
    }

    fn from_parameters(
        mut parameters: Parameters,
        _is_input_to: &[NodeID],
        _context: &ProcessingContext,
    ) -> Result<Self> {
        Ok(Self {
            input: parameters.take("input")?,
            cfa_descriptor: CfaDescriptor {
                red_in_first_col: parameters.take("red-in-first-col")?,
                red_in_first_row: parameters.take("red-in-first-row")?,
            },
            last_frame_info: Default::default(),
            debug: parameters.take("debug")?,
        })
    }
}

#[async_trait]
impl ProcessingNode for DualFrameRawDecoder {
    async fn pull(
        &self,
        frame_number: u64,
        _puller_id: NodeID,
        context: &ProcessingContext,
    ) -> Result<Payload> {
        let LastFrameInfo(_, next_even, last_wrsel, old_frame) =
            self.last_frame_info.wait(move |LastFrameInfo(next, ..)| *next == frame_number).await;

        let mut offset = 2;
        let pulled_frames = (|| async {
            match old_frame {
                Some(frame_a) => {
                    self.last_frame_info
                        .update(|LastFrameInfo(_, _, _, old_frame)| *old_frame = None);

                    offset = 1;
                    let frame = self.input.pull(next_even, context).await?;
                    let frame_b =
                        context.ensure_cpu_buffer::<Rgb>(&frame).context("Wrong input format")?;
                    Result::<_>::Ok((frame_a, frame_b))
                }
                None => {
                    let frames = join!(
                        self.input.pull(next_even, context),
                        self.input.pull(next_even + 1, context)
                    );
                    let frame_a = context
                        .ensure_cpu_buffer::<Rgb>(&frames.0?)
                        .context("Wrong input format")?;
                    let frame_b = context
                        .ensure_cpu_buffer::<Rgb>(&frames.1?)
                        .context("Wrong input format")?;
                    Result::<_>::Ok((frame_a, frame_b))
                }
            }
        })()
        .await;

        if let Err(e) = pulled_frames {
            // println!("problem getting frame, {next_even} -> {}", next_even + offset);
            self.last_frame_info.update(move |LastFrameInfo(next, next_next_even, ..)| {
                *next = frame_number + 1;
                *next_next_even = next_even + offset;
            });
            return Err(e);
        }
        let (frame_a, frame_b) = pulled_frames.unwrap();
        let swap = frame_a.storage.as_slice(|frame_a| {
            frame_b.storage.as_slice(|frame_b| {
                let ctr_a = frame_a[0];
                let ctr_b = frame_b[0];
                (ctr_a > ctr_b) || ((ctr_a == 0) && (ctr_b >= 14))
            })
        });
        let (frame_a, frame_b) = if swap { (frame_b, frame_a) } else { (frame_a, frame_b) };

        let (is_correct, debug_info, wrsel) = frame_a.storage.as_slice(|frame_a| {
            frame_b.storage.as_slice(|frame_b| {
                let debug_info = format!(
                    "frame a: ctr: {}, wrsel: {}, ty: {}\n",
                    frame_a[0], frame_a[1], frame_a[2]
                ) + &format!(
                    "frame b: ctr: {}, wrsel: {}, ty: {}",
                    frame_b[0], frame_b[1], frame_b[2]
                );
                if self.debug {
                    println!("---------");
                    println!("{}", debug_info);
                }
                let wrsel_matches = frame_a[1] == frame_b[1];
                let ctr_a = frame_a[0];
                let ctr_b = frame_b[0];
                let ctr_is_ok = (ctr_a.max(ctr_b) - ctr_a.min(ctr_b)) == 1;
                let ctr_is_ok = ctr_is_ok || (ctr_b == 0);
                (wrsel_matches && ctr_is_ok && (frame_a[1] != last_wrsel), debug_info, frame_a[1])
            })
        });
        self.last_frame_info.update(
            |LastFrameInfo(next, next_next_even, last_wrsel, old_frame)| {
                *next = frame_number + 1;
                *next_next_even = next_even + offset;
                *last_wrsel = wrsel;
                if !is_correct {
                    // println!("slipped, offset = {offset}, next_even = {next_even}");
                    *old_frame = Some(frame_b.clone());
                }
            },
        );
        if !is_correct {
            return Err(anyhow!("frame slipped in DualFrameRawDecoder:\n{}", debug_info));
        }

        let interp = Raw {
            width: frame_a.interp.width * 2,
            height: frame_a.interp.height * 2,
            bit_depth: 12,
            cfa: self.cfa_descriptor,
            fps: frame_a.interp.fps / 2.0,
        };

        let mut new_buffer = unsafe { context.get_uninit_cpu_buffer(interp.required_bytes()) };

        let line_bytes = frame_a.interp.width as usize * 3;
        frame_a.storage.as_slice(|frame_a| {
            frame_b.storage.as_slice(|frame_b| {
                let (frame_a, frame_b) = if frame_a[2] == FRAME_A_MARKER {
                    (frame_a, frame_b)
                } else {
                    (frame_b, frame_a)
                };

                new_buffer.as_mut_slice(|new_buffer| {
                    for ((frame_a_chunk, frame_b_chunk), output_chunk) in frame_a
                        .chunks_exact(line_bytes)
                        .zip(frame_b.chunks_exact(line_bytes))
                        .zip(new_buffer.chunks_exact_mut(line_bytes * 2))
                    {
                        let mut chunks = output_chunk.chunks_exact_mut(line_bytes);
                        chunks.next().unwrap().copy_from_slice(frame_b_chunk);
                        chunks.next().unwrap().copy_from_slice(frame_a_chunk);
                    }
                });
            })
        });

        Ok(Payload::from(Frame { interp, storage: new_buffer }))
    }

    fn get_caps(&self) -> Caps {
        let upstream = self.input.get_caps();
        Caps { frame_count: upstream.frame_count.map(|x| x / 2), ..upstream }
    }
}


pub struct ReverseDualFrameRawDecoder {
    input: InputProcessingNode,
    flip: bool,
}
impl Parameterizable for ReverseDualFrameRawDecoder {
    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new()
            .with("input", ParameterTypeDescriptor::Mandatory(ParameterType::NodeInput))
            // For use with old recording, where we sometimes fucked up the A/B decoding,
            // and produced files with the lines swapped.
            // Should also be transparently fixed by DualFrameRawDecoder
            .with(
                "flip",
                ParameterTypeDescriptor::Optional(
                    ParameterType::BoolParameter,
                    SerdeParameterValue::BoolParameter(false),
                ),
            )
    }

    fn from_parameters(
        mut parameters: Parameters,
        _is_input_to: &[NodeID],
        _context: &ProcessingContext,
    ) -> Result<Self> {
        Ok(Self { input: parameters.take("input")?, flip: parameters.take("flip")? })
    }
}

#[async_trait]
impl ProcessingNode for ReverseDualFrameRawDecoder {
    async fn pull(
        &self,
        frame_number: u64,
        _puller_id: NodeID,
        context: &ProcessingContext,
    ) -> Result<Payload> {
        let downstream = frame_number / 2;
        let frame = self.input.pull(downstream, context).await?;
        let frame = context.ensure_cpu_buffer::<Raw>(&frame)?;
        let offset = if self.flip { 1 } else { 0 };
        let offset = ((frame_number + offset) % 2) as usize;

        let line_bytes = (frame.interp.width * 3 / 2) as usize;
        let out_buffer = unsafe {
            let mut buffer =
                context.get_uninit_cpu_buffer(line_bytes * frame.interp.height as usize / 2);
            buffer.as_mut_slice(|buffer| {
                frame.storage.as_slice(|input| {
                    for (out, input) in buffer
                        .chunks_exact_mut(line_bytes)
                        .zip(input.chunks_exact(line_bytes).skip(offset).step_by(2))
                    {
                        out.copy_from_slice(input)
                    }
                });
            });

            buffer
        };

        Ok(Payload::from(Frame {
            interp: Rgb {
                width: frame.interp.width / 2,
                height: frame.interp.height / 2,
                fps: frame.interp.fps * 2.0,
            },
            storage: out_buffer,
        }))
    }

    fn get_caps(&self) -> Caps {
        let upstream = self.input.get_caps();
        Caps { frame_count: upstream.frame_count.map(|x| x * 2), ..upstream }
    }
}
