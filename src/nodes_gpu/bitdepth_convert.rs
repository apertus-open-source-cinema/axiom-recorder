use crate::pipeline_processing::{
    buffers::GpuBuffer,
    frame::{Frame, FrameInterpretation, SampleInterpretation},
    gpu_util::ensure_gpu_buffer_frame,
    node::{Caps, InputProcessingNode, NodeID, ProcessingNode, Request},
    parametrizable::prelude::*,
    payload::Payload,
    processing_context::ProcessingContext,
};
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use std::sync::Arc;
use vulkano::{
    buffer::{BufferUsage, DeviceLocalBuffer},
    command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage::OneTimeSubmit},
    descriptor_set::{persistent::PersistentDescriptorSet, WriteDescriptorSet},
    device::{Device, Queue},
    pipeline::{ComputePipeline, Pipeline, PipelineBindPoint},
    sync::GpuFuture,
    DeviceSize,
};

// generated by the macro
#[allow(clippy::needless_question_mark)]
mod compute_shader {
    vulkano_shaders::shader! {
        ty: "compute",
        path: "src/nodes_gpu/bitdepth_convert_12_8.glsl"
    }
}

pub struct GpuBitDepthConverter {
    device: Arc<Device>,
    pipeline: Arc<ComputePipeline>,
    queue: Arc<Queue>,
    input: InputProcessingNode,
}

impl Parameterizable for GpuBitDepthConverter {
    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new().with("input", Mandatory(NodeInputParameter))
    }
    fn from_parameters(
        mut parameters: Parameters,
        _is_input_to: &[NodeID],
        context: &ProcessingContext,
    ) -> Result<Self>
    where
        Self: Sized,
    {
        let (device, queues) = context.require_vulkan()?;
        let queue = queues.iter().find(|&q| q.family().supports_compute()).unwrap().clone();

        let shader = compute_shader::load(device.clone()).unwrap();
        let pipeline = ComputePipeline::new(
            device.clone(),
            shader.entry_point("main").unwrap(),
            &(),
            None,
            |_| {},
        )
        .unwrap();

        Ok(GpuBitDepthConverter { device, pipeline, queue, input: parameters.take("input")? })
    }
}

#[async_trait]
impl ProcessingNode for GpuBitDepthConverter {
    async fn pull(&self, request: Request) -> Result<Payload> {
        let input = self.input.pull(request).await?;

        let (frame, fut) = ensure_gpu_buffer_frame(&input, self.queue.clone())
            .context("Wrong input format for GpuBitDepthConvert")?;


        if !matches!(frame.interpretation.sample_interpretation, SampleInterpretation::UInt(12)) {
            return Err(anyhow!(
                "A frame with bit_depth=12 is required. Convert the bit depth of the frame!"
            ));
        }

        let interpretation = FrameInterpretation {
            sample_interpretation: SampleInterpretation::UInt(8),
            ..frame.interpretation.clone()
        };
        let sink_buffer = DeviceLocalBuffer::<[u8]>::array(
            self.device.clone(),
            interpretation.required_bytes() as DeviceSize,
            BufferUsage { storage_buffer: true, transfer_src: true, ..BufferUsage::none() },
            std::iter::once(self.queue.family()),
        )?;

        let push_constants = compute_shader::ty::PushConstantData {
            width: interpretation.width as u32,
            height: interpretation.height as u32,
        };

        let layout = self.pipeline.layout().set_layouts()[0].clone();
        let set = PersistentDescriptorSet::new(
            layout,
            [
                WriteDescriptorSet::buffer(0, frame.storage.untyped()),
                WriteDescriptorSet::buffer(1, sink_buffer.clone()),
            ],
        )
        .unwrap();

        let mut builder = AutoCommandBufferBuilder::primary(
            self.device.clone(),
            self.queue.family(),
            OneTimeSubmit,
        )
        .unwrap();
        builder
            .bind_descriptor_sets(
                PipelineBindPoint::Compute,
                self.pipeline.layout().clone(),
                0,
                set,
            )
            .push_constants(self.pipeline.layout().clone(), 0, push_constants)
            .bind_pipeline_compute(self.pipeline.clone())
            .dispatch([
                (interpretation.width as u32 + 31) / 16 / 2,
                (interpretation.height as u32 + 31) / 32,
                1,
            ])?;
        let command_buffer = builder.build()?;

        let future =
            fut.then_execute(self.queue.clone(), command_buffer)?.then_signal_fence_and_flush()?;

        future.wait(None).unwrap();
        Ok(Payload::from(Frame { interpretation, storage: GpuBuffer::from(sink_buffer) }))
    }

    fn get_caps(&self) -> Caps { self.input.get_caps() }
}
