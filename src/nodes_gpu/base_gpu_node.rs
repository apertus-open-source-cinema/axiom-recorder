use crate::{
    nodes_gpu::shader_util::{compile_shader, generate_single_node_shader},
    pipeline_processing::{
        buffers::GpuBuffer,
        frame::{Frame, FrameInterpretation},
        gpu_util::ensure_gpu_buffer_frame,
        node::{Caps, InputProcessingNode, NodeID, ProcessingNode, Request},
        parametrizable::{
            prelude::*,
            Parameterizable,
            ParameterizableDescriptor,
            Parameters,
            ParametersDescriptor,
        },
        payload::Payload,
        processing_context::ProcessingContext,
    },
};
use anyhow::{Context, Result};
use async_trait::async_trait;

use parking_lot::{RwLock};
use std::{collections::HashMap, sync::Arc};
use vulkano::{
    buffer::{BufferUsage, DeviceLocalBuffer},
    command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage::OneTimeSubmit},
    descriptor_set::{PersistentDescriptorSet, WriteDescriptorSet},
    device::{Device, Queue},
    pipeline::{ComputePipeline, Pipeline, PipelineBindPoint},
    sync::GpuFuture,
    DeviceSize,
};

pub enum PushConstantValue {
    U32(u32),
}

type PushConstants = HashMap<String, PushConstantValue>;

pub trait GpuNode: Parameterizable {
    fn get_glsl(&self) -> String;
    fn get_binding(
        &self,
        frame_interpretation: &FrameInterpretation,
    ) -> Result<HashMap<String, PushConstantValue>>;
    fn get_interpretation(&self, frame_interpretation: FrameInterpretation) -> FrameInterpretation {
        frame_interpretation
    }
}


pub struct GpuNodeImpl<T: GpuNode> {
    gpu_node: T,
    device: Arc<Device>,
    pipeline: Arc<RwLock<Option<(FrameInterpretation, Arc<ComputePipeline>)>>>,
    queue: Arc<Queue>,
    input: InputProcessingNode,
}

impl<T: GpuNode> Parameterizable for GpuNodeImpl<T> {
    fn describe_parameters() -> ParametersDescriptor {
        T::describe_parameters().with("input", Mandatory(NodeInputParameter))
    }
    fn from_parameters(
        mut parameters: Parameters,
        is_input_to: &[NodeID],
        context: &ProcessingContext,
    ) -> Result<Self>
    where
        Self: Sized,
    {
        let input = parameters.take("input")?;
        let gpu_node = T::from_parameters(parameters, is_input_to, context)?;

        let (device, queues) = context.require_vulkan()?;
        let queue = queues.iter().find(|&q| q.family().supports_compute()).unwrap().clone();

        let pipeline = Default::default();

        Ok(Self { gpu_node, device, pipeline, queue, input })
    }

    fn get_name() -> String { T::get_name() }
    fn describe() -> ParameterizableDescriptor { T::describe() }
}

#[async_trait]
impl<T: GpuNode + Send + Sync> ProcessingNode for GpuNodeImpl<T> {
    async fn pull(&self, request: Request) -> Result<Payload> {
        let input = self.input.pull(request).await?;
        let (frame, fut) = ensure_gpu_buffer_frame(&input, self.queue.clone())
            .context(format!("Wrong input format for node {}", Self::get_name()))?;

        let _binding = self.gpu_node.get_binding(&frame.interpretation)?;
        let output_interpretation = self.gpu_node.get_interpretation(frame.interpretation);

        if self.pipeline.read().is_none()
            || self.pipeline.read().as_ref().unwrap().0 != frame.interpretation
        {
            let shader_code = generate_single_node_shader(
                self.gpu_node.get_glsl(),
                frame.interpretation,
                output_interpretation,
            )?;
            let shader = compile_shader(&shader_code, self.device.clone())?;
            let pipeline = ComputePipeline::new(
                self.device.clone(),
                shader.entry_point("main").unwrap(),
                &(),
                None,
                |_| {},
            )?;

            self.pipeline.write().replace((frame.interpretation.clone(), pipeline));
        }

        let (_, pipeline) = self.pipeline.read().clone().unwrap();

        let sink_buffer = DeviceLocalBuffer::<[u8]>::array(
            self.device.clone(),
            output_interpretation.required_bytes() as DeviceSize,
            BufferUsage { storage_buffer: true, transfer_src: true, ..BufferUsage::none() },
            std::iter::once(self.queue.family()),
        )?;

        // TOOD: generate push constants
        let layout = pipeline.layout().set_layouts()[0].clone();
        let set = PersistentDescriptorSet::new(
            layout,
            [
                WriteDescriptorSet::buffer(0, frame.storage.untyped()),
                WriteDescriptorSet::buffer(1, sink_buffer.clone()),
                // TODO: generate other buffer bindings
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
                pipeline.layout().clone(),
                0,
                set,
            )
            //.push_constants(self.pipeline.layout().clone(), 0, push_constants)
            .bind_pipeline_compute(pipeline.clone())
            .dispatch([
                (output_interpretation.width as u32 + 31) / 16,
                (output_interpretation.height as u32 + 31) / 16,
                1,
            ])?;
        let command_buffer = builder.build()?;

        let future =
            fut.then_execute(self.queue.clone(), command_buffer)?.then_signal_fence_and_flush()?;

        future.wait(None).unwrap();
        Ok(Payload::from(Frame {
            interpretation: output_interpretation,
            storage: GpuBuffer::from(sink_buffer),
        }))
    }

    fn get_caps(&self) -> Caps { self.input.get_caps() }
}
