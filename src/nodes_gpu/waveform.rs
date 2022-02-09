use crate::pipeline_processing::{
    buffers::GpuBuffer,
    frame::{Frame, FrameInterpretation, Rgb},
    gpu_util::ensure_gpu_buffer,
    node::{Caps, ProcessingNode},
    parametrizable::{
        ParameterType,
        ParameterTypeDescriptor,
        Parameterizable,
        Parameters,
        ParametersDescriptor,
    },
    payload::Payload,
    processing_context::ProcessingContext,
};
use anyhow::{Context, Result};
use async_trait::async_trait;
use std::{iter, sync::Arc};
use vulkano::{
    buffer::{BufferUsage, CpuAccessibleBuffer, DeviceLocalBuffer},
    command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage::OneTimeSubmit},
    descriptor_set::persistent::PersistentDescriptorSet,
    device::{Device, Queue},
    pipeline::{ComputePipeline, PipelineBindPoint},
    sync::GpuFuture,
    DeviceSize,
};


mod compute_shader {
    vulkano_shaders::shader! {
        ty: "compute",
        path: "src/nodes_gpu/waveform.glsl"
    }
}

pub struct Waveform {
    device: Arc<Device>,
    pipeline: Arc<ComputePipeline>,
    queue: Arc<Queue>,
    input: Arc<dyn ProcessingNode + Send + Sync>,
}

impl Parameterizable for Waveform {
    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new()
            .with("input", ParameterTypeDescriptor::Mandatory(ParameterType::NodeInput))
    }
    fn from_parameters(parameters: &Parameters, context: &ProcessingContext) -> Result<Self>
    where
        Self: Sized,
    {
        let (device, queues) = context.require_vulkan()?;
        let queue = queues.iter().find(|&q| q.family().supports_compute()).unwrap().clone();

        let pipeline = Arc::new({
            let shader = compute_shader::Shader::load(device.clone()).unwrap();
            ComputePipeline::new(device.clone(), &shader.main_entry_point(), &(), None, |_| {})
                .unwrap()
        });

        Ok(Waveform { device, pipeline, queue, input: parameters.get("input")? })
    }
}

#[async_trait]
impl ProcessingNode for Waveform {
    async fn pull(&self, frame_number: u64, context: &ProcessingContext) -> Result<Payload> {
        let mut input = self.input.pull(frame_number, &context).await?;

        let (frame, fut) = ensure_gpu_buffer::<Rgb>(&mut input, self.queue.clone())
            .context("Wrong input format")?;

        let interp = Rgb { height: 1024, ..frame.interp };


        let empty_buffer = CpuAccessibleBuffer::from_iter(
            self.device.clone(),
            BufferUsage { transfer_source: true, ..BufferUsage::none() },
            false,
            (0..interp.required_bytes()).map(|_| 0),
        )?;


        let sink_buffer = DeviceLocalBuffer::<[u8]>::array(
            self.device.clone(),
            interp.required_bytes() as DeviceSize,
            BufferUsage {
                storage_buffer: true,
                storage_texel_buffer: true,
                transfer_destination: true,
                ..BufferUsage::none()
            },
            std::iter::once(self.queue.family()),
        )?;

        let push_constants = compute_shader::ty::PushConstantData {
            width: frame.interp.width as _,
            height: frame.interp.height as _,
        };

        let layout = self.pipeline.layout().descriptor_set_layouts()[0].clone();
        let set = Arc::new({
            let mut builder = PersistentDescriptorSet::start(layout);
            builder.add_buffer(frame.storage.untyped())?;
            builder.add_buffer(sink_buffer.clone())?;
            builder.build()?
        });

        let mut builder = AutoCommandBufferBuilder::primary(
            self.device.clone(),
            self.queue.family(),
            OneTimeSubmit,
        )
        .unwrap();

        builder
            .copy_buffer(empty_buffer, sink_buffer.clone())?
            .bind_descriptor_sets(
                PipelineBindPoint::Compute,
                self.pipeline.layout().clone(),
                0,
                set,
            )
            .push_constants(self.pipeline.layout().clone(), 0, push_constants)
            .bind_pipeline_compute(self.pipeline.clone())
            .dispatch([
                (frame.interp.width as u32 + 31) / 32 / 2,
                (frame.interp.height as u32 + 31) / 32 / 2,
                1,
            ])?;
        let command_buffer = builder.build()?;

        let future =
            fut.then_execute(self.queue.clone(), command_buffer)?.then_signal_fence_and_flush()?;

        future.wait(None).unwrap();
        Ok(Payload::from(Frame { interp, storage: GpuBuffer::from(sink_buffer) }))
    }

    fn get_caps(&self) -> Caps { self.input.get_caps() }
}
