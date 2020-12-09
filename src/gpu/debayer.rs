use crate::{
    frame::raw_frame::RawFrame,
    pipeline_processing::{
        parametrizable::{Parameterizable, Parameters, ParametersDescriptor},
        processing_node::ProcessingNode,
    },
};
use anyhow::{anyhow, Context, Result};
use vulkano::{
    buffer::{BufferUsage, CpuAccessibleBuffer},
    command_buffer::AutoCommandBufferBuilder,
    descriptor::{descriptor_set::PersistentDescriptorSet, PipelineLayoutAbstract},
    device::{Device, DeviceExtensions},
    instance::{Instance, InstanceExtensions, PhysicalDevice},
    pipeline::ComputePipeline,
    sync,
    sync::GpuFuture,
};

use crate::{frame::rgba_frame::RgbaFrame, gpu::gpu_util::CpuAccessibleBufferReadView};

use crate::{
    frame::rgb_frame::RgbFrame,
    gpu::gpu_util::VulkanContext,
    pipeline_processing::payload::Payload,
};
use std::sync::{Arc, MutexGuard};
use vulkano::{
    buffer::{BufferView, TypedBufferAccess},
    descriptor::pipeline_layout::PipelineLayout,
    device::Queue,
    format::{R8G8B8A8Unorm, R8G8B8Unorm, R8Unorm},
};

mod compute_shader {
    vulkano_shaders::shader! {
        ty: "compute",
        path: "src/gpu/debayer.glsl"
    }
}

pub struct Debayer {
    device: Arc<Device>,
    pipeline: Arc<ComputePipeline<PipelineLayout<compute_shader::Layout>>>,
    queue: Arc<Queue>,
}

impl Parameterizable for Debayer {
    fn describe_parameters() -> ParametersDescriptor { ParametersDescriptor::new() }
    fn from_parameters(_parameters: &Parameters) -> Result<Self>
    where
        Self: Sized,
    {
        let device = VulkanContext::get().device;
        let queue = VulkanContext::get()
            .queues
            .iter()
            .find(|&q| q.family().supports_compute())
            .unwrap()
            .clone();

        let pipeline = Arc::new({
            let shader = compute_shader::Shader::load(device.clone()).unwrap();
            ComputePipeline::new(device.clone(), &shader.main_entry_point(), &(), None).unwrap()
        });

        Ok(Debayer { device, pipeline, queue })
    }
}

impl ProcessingNode for Debayer {
    fn process(&self, input: &mut Payload, frame_lock: MutexGuard<u64>) -> Result<Option<Payload>> {
        drop(frame_lock);
        let frame = input.downcast::<RawFrame>().context("Wrong input format")?;

        if frame.bit_depth != 8 {
            return Err(anyhow!("A frame with bit_depth=8 is required. Repack the frame!"));
        }
        let source_buffer = CpuAccessibleBufferReadView::<u8>::from_buffer(frame.buffer.clone())?
            .as_cpu_accessible_buffer();
        let sink_buffer: Arc<CpuAccessibleBuffer<[u8]>> = unsafe {
            CpuAccessibleBuffer::uninitialized_array(
                self.device.clone(),
                source_buffer.len() * 3,
                BufferUsage::all(),
                true,
            )?
        };

        let push_constants = compute_shader::ty::PushConstantData {
            width: frame.width as u32,
            height: frame.height as u32,
            first_red_x: (!frame.cfa.first_is_red_x) as u32,
            first_red_y: (!frame.cfa.first_is_red_y) as u32,
        };

        let layout = self.pipeline.layout().descriptor_set_layout(0).unwrap();
        let set = Arc::new(
            PersistentDescriptorSet::start(layout.clone())
                .add_buffer_view(BufferView::new(source_buffer, R8Unorm)?)?
                .add_buffer_view(BufferView::new(sink_buffer.clone(), R8Unorm)?)?
                .build()?,
        );

        let mut builder = AutoCommandBufferBuilder::primary_one_time_submit(
            self.device.clone(),
            self.queue.family(),
        )
        .unwrap();
        builder.dispatch(
            [frame.width as u32 / 32, frame.height as u32 / 32, 1],
            self.pipeline.clone(),
            set,
            push_constants,
        )?;
        let command_buffer = builder.build()?;

        let future = sync::now(self.device.clone())
            .then_execute(self.queue.clone(), command_buffer)?
            .then_signal_fence_and_flush()?;

        future.wait(None).unwrap();
        let output_data = CpuAccessibleBufferReadView::from_cpu_accessible_buffer(sink_buffer)?;
        Ok(Some(Payload::from(RgbFrame::from_bytes(output_data, frame.width, frame.height)?)))
    }
}
