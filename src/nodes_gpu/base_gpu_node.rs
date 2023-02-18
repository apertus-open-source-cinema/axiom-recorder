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
use anyhow::{anyhow, bail, Context, Result};
use async_trait::async_trait;
use parking_lot::RwLock;
use std::{collections::HashMap, sync::Arc};
use vulkano::{
    buffer::{BufferAccess, BufferUsage, DeviceLocalBuffer},
    command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage::OneTimeSubmit},
    descriptor_set::{PersistentDescriptorSet, WriteDescriptorSet},
    device::{Device, Queue},
    image::ImageViewAbstract,
    pipeline::{ComputePipeline, Pipeline, PipelineBindPoint},
    sampler::Sampler,
    sync::GpuFuture,
    DeviceSize,
};

#[derive(Clone)]
pub enum BindingValue {
    U32(u32),
    F32(f32),
    Sampler((Arc<dyn ImageViewAbstract>, Arc<Sampler>)),
    Buffer(Arc<dyn BufferAccess>),
}

pub trait GpuNode: Parameterizable {
    fn get_glsl(&self) -> String;
    fn get_binding(
        &self,
        frame_interpretation: &FrameInterpretation,
    ) -> Result<HashMap<String, BindingValue>>;
    fn get_interpretation(&self, frame_interpretation: FrameInterpretation) -> FrameInterpretation {
        frame_interpretation
    }
}

#[derive(Clone)]
struct PipelineCacheItem {
    tag: FrameInterpretation,
    pipeline: Arc<ComputePipeline>,
    push_constant_names: Vec<(String, u32)>,
    binding_names: Vec<(String, u32)>,
}

pub struct GpuNodeImpl<T: GpuNode> {
    gpu_node: T,
    device: Arc<Device>,
    pipeline: Arc<RwLock<Option<PipelineCacheItem>>>,
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

        let mut binding = self.gpu_node.get_binding(&frame.interpretation)?;

        let output_interpretation = self.gpu_node.get_interpretation(frame.interpretation);


        // TODO: this is racy
        if self.pipeline.read().is_none()
            || self.pipeline.read().as_ref().unwrap().tag != frame.interpretation
        {
            let shader_code = generate_single_node_shader(
                self.gpu_node.get_glsl(),
                frame.interpretation,
                output_interpretation,
            )?;
            let spirv = compile_shader(&shader_code)
                .context(format!("compilation error. shader code is:\n{shader_code}"))?;

            let shader = unsafe {
                vulkano::shader::ShaderModule::from_words(self.device.clone(), &spirv.as_binary())
            }?;
            let pipeline = ComputePipeline::new(
                self.device.clone(),
                shader.entry_point("main").unwrap(),
                &(),
                None,
                |_| {},
            )?;

            let reflection = spirv_reflect::ShaderModule::load_u32_data(spirv.as_binary())
                .map_err(|e| anyhow!(e))?;

            let mut push_constant_names = Vec::new();
            for block in
                reflection.enumerate_push_constant_blocks(Some("main")).map_err(|e| anyhow!(e))?
            {
                for member in block.members {
                    push_constant_names.push((member.name.clone(), member.absolute_offset))
                }
            }

            let mut binding_names = Vec::new();
            for binding in
                reflection.enumerate_descriptor_bindings(Some("main")).map_err(|e| anyhow!(e))?
            {
                binding_names.push((binding.name, binding.binding));
            }

            self.pipeline.write().replace(PipelineCacheItem {
                tag: frame.interpretation.clone(),
                pipeline,
                push_constant_names,
                binding_names,
            });
        }

        let PipelineCacheItem { pipeline, push_constant_names, binding_names, .. } =
            self.pipeline.read().clone().unwrap();

        let sink_buffer = DeviceLocalBuffer::<[u8]>::array(
            self.device.clone(),
            output_interpretation.required_bytes() as DeviceSize,
            BufferUsage { storage_buffer: true, transfer_src: true, ..BufferUsage::none() },
            std::iter::once(self.queue.family()),
        )?;
        binding.insert("source".to_string(), BindingValue::Buffer(frame.storage.untyped()));
        binding.insert("sink".to_string(), BindingValue::Buffer(sink_buffer.clone()));

        let layout = pipeline.layout().set_layouts()[0].clone();
        let set = PersistentDescriptorSet::new(
            layout,
            binding_names.iter().map(|(name, n)| match binding.get(name) {
                Some(BindingValue::Buffer(buffer)) => {
                    WriteDescriptorSet::buffer(*n, buffer.clone())
                }
                Some(BindingValue::Sampler((image_view, sampler))) => {
                    WriteDescriptorSet::image_view_sampler(*n, image_view.clone(), sampler.clone())
                }
                Some(_) => panic!("invalid type for binding"),
                None => panic!("not all binding values were supplied"),
            }),
        )
        .unwrap();

        let mut builder = AutoCommandBufferBuilder::primary(
            self.device.clone(),
            self.queue.family(),
            OneTimeSubmit,
        )
        .unwrap();
        builder.bind_descriptor_sets(PipelineBindPoint::Compute, pipeline.layout().clone(), 0, set);

        for (name, absolute_offset) in push_constant_names {
            let value = binding
                .get(&name)
                .ok_or_else(|| anyhow!("couldn't find binding value for push constant '{name}'"))?;
            match value.clone() {
                BindingValue::U32(v) => {
                    builder.push_constants(pipeline.layout().clone(), absolute_offset, v);
                }
                BindingValue::F32(v) => {
                    builder.push_constants(pipeline.layout().clone(), absolute_offset, v);
                }
                BindingValue::Sampler(_) => bail!("Samplers are not Push Constants"),
                BindingValue::Buffer(_) => bail!("Buffers are not Push Constants"),
            }
        }

        builder.bind_pipeline_compute(pipeline.clone()).dispatch([
            (output_interpretation.width as u32 + 255) / 256,
            (output_interpretation.height as u32 + 3) / 4,
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
