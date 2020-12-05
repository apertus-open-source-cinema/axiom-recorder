use crate::pipeline_processing::parametrizable::{Parameterizable, Parameters, ParametersDescriptor};
use anyhow::{Result, Context};
use crate::pipeline_processing::processing_node::{ProcessingNode, Payload};
use crate::frame::raw_frame::RawFrame;
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

use std::sync::{Arc, Mutex};
use core::ptr;
use vulkano::buffer::TypedBufferAccess;
use vulkano::descriptor::descriptor_set::{FixedSizeDescriptorSetsPool, UnsafeDescriptorSetLayout};
use vulkano::device::Queue;
use vulkano::descriptor::pipeline_layout::{PipelineLayout, RuntimePipelineDesc};
use crate::debayer::gpu_util::CpuAccessibleBufferReadView;

mod cs {
    vulkano_shaders::shader! {
        ty: "compute",
        path: "src/debayer/resolution_loss.glsl"
    }
}

pub struct DebayerNode {
    device: Arc<Device>,
    pipeline: Arc<ComputePipeline<PipelineLayout<cs::Layout>>>,
    queue: Arc<Queue>,
}

impl Parameterizable for DebayerNode {
    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new()
    }
    fn from_parameters(parameters: &Parameters) -> Result<Self> where
        Self: Sized {
        let instance = Instance::new(None, &InstanceExtensions::none(), None).unwrap();
        let physical = PhysicalDevice::enumerate(&instance).next().unwrap();
        let queue_family = physical.queue_families().find(|&q| q.supports_compute()).unwrap();
        let (device, mut queues) = Device::new(
            physical,
            physical.supported_features(),
            &DeviceExtensions { khr_storage_buffer_storage_class: true, ..DeviceExtensions::none() },
            [(queue_family, 0.5)].iter().cloned(),
        )
            .unwrap();
        let queue = queues.next().unwrap();
        println!("Device initialized");

        let pipeline = Arc::new({
            let shader = cs::Shader::load(device.clone()).unwrap();
            ComputePipeline::new(device.clone(), &shader.main_entry_point(), &(), None).unwrap()
        });

        Ok(DebayerNode {
            device,
            pipeline,
            queue,
        })
    }
}

impl ProcessingNode for DebayerNode {
    fn process(&self, input: &mut Payload) -> Result<Option<Payload>> {
        let frame = input.downcast::<RawFrame>().context("Wrong input format")?;

        let frame_data = frame.buffer.unpacked_u8()?.clone();
        let frame_size = frame_data.len();
        let source_buffer = unsafe {
            let uninitialized: Arc<CpuAccessibleBuffer<[u8]>> = CpuAccessibleBuffer::uninitialized_array(self.device.clone(), frame_size, BufferUsage::all(), true)?;

            {
                let mut mapping = uninitialized.write().unwrap();
                for i in 0..frame_size {
                    mapping[i] = frame_data[i];
                }
            }
            uninitialized
        };
        let sink_buffer: Arc<CpuAccessibleBuffer<[u8]>> = unsafe {
            CpuAccessibleBuffer::uninitialized_array(self.device.clone(), frame_size, BufferUsage::all(), true)?
        };

        let push_constants = cs::ty::PushConstantData {
            width: frame.width as u32,
            height: frame.height as u32,
        };

        let layout = self.pipeline.layout().descriptor_set_layout(0).unwrap();
        let set = Arc::new(
            PersistentDescriptorSet::start(layout.clone())
            .add_buffer(source_buffer.clone())?
            .add_buffer(sink_buffer.clone())?
            .build()?
        );

        let mut builder = AutoCommandBufferBuilder::primary_one_time_submit(self.device.clone(), self.queue.family()).unwrap();
        builder.dispatch([frame.width as u32 / 32, frame.height as u32 / 32, 1], self.pipeline.clone(), set.clone(), push_constants)?;
        let command_buffer = builder.build()?;

        let future = sync::now(self.device.clone())
            .then_execute(self.queue.clone(), command_buffer)?
            .then_signal_fence_and_flush()?;

        future.wait(None).unwrap();
        let output_data = CpuAccessibleBufferReadView::new(sink_buffer)?;
        Ok(Some(Payload::from(RawFrame::from_bytes(output_data, frame.width, frame.height, 8)?)))
    }
}