use crate::{
    gpu::gpu_util::{VulkanContext},
    pipeline_processing::{
        execute::ProcessingStageLockWaiter,
        parametrizable::{Parameterizable, Parameters, ParametersDescriptor},
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
    pipeline::{ComputePipeline, ComputePipelineAbstract},
    sync,
    sync::GpuFuture,
};
use vulkano::buffer::DeviceLocalBuffer;
use crate::frame::{Frame, GpuBuffer, Raw, Rgb};
use crate::gpu::gpu_util::ensure_gpu_buffer;

mod compute_shader {
    vulkano_shaders::shader! {
        ty: "compute",
        path: "src/gpu/debayer.glsl"
    }
}

pub struct Debayer {
    device: Arc<Device>,
    pipeline: Arc<ComputePipeline>,
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
    fn process(
        &self,
        input: &mut Payload,
        frame_lock: ProcessingStageLockWaiter,
    ) -> Result<Option<Payload>> {
        let (frame, fut) = ensure_gpu_buffer::<Raw>(input).context("Wrong input format")?;

        if frame.interp.bit_depth != 8 {
            return Err(anyhow!("A frame with bit_depth=8 is required. Repack the frame!"));
        }

        let sink_buffer = DeviceLocalBuffer::<[u8]>::array(
            self.device.clone(),
            frame.interp.width * frame.interp.height * 3,
            BufferUsage {
                storage_buffer: true,
                storage_texel_buffer: true,
                ..BufferUsage::none()
            },
            std::iter::once(self.queue.family())
        )?;

        let push_constants = compute_shader::ty::PushConstantData {
            width: frame.interp.width as u32,
            height: frame.interp.height as u32,
            first_red_x: (!frame.interp.cfa.first_is_red_x) as u32,
            first_red_y: (!frame.interp.cfa.first_is_red_y) as u32,
        };

        let layout = self.pipeline.layout().descriptor_set_layouts()[0].clone();
        let set = Arc::new(
            PersistentDescriptorSet::start(layout)
                .add_buffer(frame.storage.clone())?
                .add_buffer(sink_buffer.clone())?
                .build()?,
        );

        let mut builder = AutoCommandBufferBuilder::primary(
            self.device.clone(),
            self.queue.family(),
            OneTimeSubmit,
        )
        .unwrap();
        builder.dispatch(
            [frame.interp.width as u32 / 32, frame.interp.height as u32 / 32, 1],
            self.pipeline.clone(),
            set,
            push_constants,
        )?;
        let command_buffer = builder.build()?;

        let future = sync::now(self.device.clone())
            .then_execute(self.queue.clone(), command_buffer)?
            .then_signal_fence_and_flush()?;

        future.wait(None).unwrap();
        Ok(Some(Payload::from(Frame {
            interp: Rgb {
                width: frame.interp.width,
                height: frame.interp.height
            },
            storage: sink_buffer as GpuBuffer
        })))
    }
}
