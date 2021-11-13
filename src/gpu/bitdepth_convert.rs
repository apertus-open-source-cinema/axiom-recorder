use crate::{
    frame::raw_frame::RawFrame,
    gpu::gpu_util::CpuAccessibleBufferReadView,
    pipeline_processing::{
        parametrizable::{
            Parameterizable,
            Parameters,
            ParametersDescriptor,
            VulkanContext,
            VULKAN_CONTEXT,
        },
        execute::ProcessingStageLockWaiter,
        payload::Payload,
        processing_node::ProcessingNode,
    },
};
use anyhow::{anyhow, Context, Result};
use std::sync::Arc;
use vulkano::{
    buffer::{BufferUsage, CpuAccessibleBuffer, TypedBufferAccess},
    command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage::OneTimeSubmit},
    descriptor_set::persistent::PersistentDescriptorSet,
    device::{Device, Queue},
    pipeline::{ComputePipeline, PipelineBindPoint},
    sync,
    sync::GpuFuture,
};

mod compute_shader {
    vulkano_shaders::shader! {
        ty: "compute",
        path: "src/gpu/bitdepth_convert_12_8.glsl"
    }
}

pub struct GpuBitDepthConverter {
    device: Arc<Device>,
    pipeline: Arc<ComputePipeline>,
    queue: Arc<Queue>,
}

impl Parameterizable for GpuBitDepthConverter {
    fn describe_parameters() -> ParametersDescriptor { ParametersDescriptor::using_vulkan() }
    fn from_parameters(parameters: &Parameters) -> Result<Self>
    where
        Self: Sized,
    {
        let VulkanContext(device, queues) = parameters.get::<VulkanContext>(VULKAN_CONTEXT)?;
        let queue = queues.iter().find(|&q| q.family().supports_compute()).unwrap().clone();

        let pipeline = Arc::new({
            let shader = compute_shader::Shader::load(device.clone()).unwrap();
            ComputePipeline::new(device.clone(), &shader.main_entry_point(), &(), None, |_| {})
                .unwrap()
        });

        Ok(GpuBitDepthConverter { device, pipeline, queue })
    }
}

impl ProcessingNode for GpuBitDepthConverter {
    fn process(
        &self,
        input: &mut Payload,
        frame_lock: ProcessingStageLockWaiter,
    ) -> Result<Option<Payload>> {
        let frame = input.downcast::<RawFrame>().context("Wrong input format")?;

        if frame.bit_depth != 12 {
            return Err(anyhow!("A frame with bit_depth=8 is required. Repack the frame!"));
        }
        let source_buffer = CpuAccessibleBufferReadView::<u8>::from_buffer(
            self.device.clone(),
            frame.buffer.clone(),
        )?
        .as_cpu_accessible_buffer();
        let sink_buffer: Arc<CpuAccessibleBuffer<[u8]>> = unsafe {
            CpuAccessibleBuffer::uninitialized_array(
                self.device.clone(),
                source_buffer.len() * 8 / 12,
                BufferUsage {
                    storage_buffer: true,
                    ..BufferUsage::none()
                },
                true,
            )?
        };

        let push_constants = compute_shader::ty::PushConstantData { width: frame.width as u32 };

        let layout = self.pipeline.layout().descriptor_set_layouts()[0].clone();
        let set = Arc::new({
            let mut set = PersistentDescriptorSet::start(layout);
            set.add_buffer(source_buffer)?.add_buffer(sink_buffer.clone())?;
            set.build()?
        });

        let mut builder = AutoCommandBufferBuilder::primary(
            self.device.clone(),
            self.queue.family(),
            OneTimeSubmit,
        )
        .unwrap();
        builder
            .bind_pipeline_compute(self.pipeline.clone())
            .bind_descriptor_sets(
                PipelineBindPoint::Compute,
                self.pipeline.layout().clone(),
                0,
                set,
            )
            .push_constants(self.pipeline.layout().clone(), 0, push_constants)
            .dispatch([frame.width as u32 / 16 / 2, frame.height as u32 / 32, 1])?;
        let command_buffer = builder.build()?;

        let future = sync::now(self.device.clone())
            .then_execute(self.queue.clone(), command_buffer)?
            .then_signal_fence_and_flush()?;

        future.wait(None).unwrap();
        let output_data = CpuAccessibleBufferReadView::from_cpu_accessible_buffer(sink_buffer)?;
        Ok(Some(Payload::from(RawFrame::from_bytes(
            output_data,
            frame.width,
            frame.height,
            8,
            frame.cfa,
        )?)))
    }
}
