use crate::pipeline_processing::{
    frame::{Frame, FrameInterpretation, Raw},
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
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use std::sync::Arc;
use vulkano::{
    command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage::OneTimeSubmit},
    descriptor_set::persistent::PersistentDescriptorSet,
    device::{Device, Queue},
    pipeline::{ComputePipeline, PipelineBindPoint},
    sync::{GpuFuture, now},
};


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
    input: Arc<dyn ProcessingNode + Send + Sync>,
}

impl Parameterizable for GpuBitDepthConverter {
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

        Ok(GpuBitDepthConverter { device, pipeline, queue, input: parameters.get("input")? })
    }
}

#[async_trait]
impl ProcessingNode for GpuBitDepthConverter {
    async fn pull(&self, frame_number: u64, context: &ProcessingContext) -> Result<Payload> {
        let mut input = self.input.pull(frame_number, &context).await?;

        let frame = context.ensure_cpu_buffer::<Raw>(&mut input)
            .context("Wrong input format")?;

        if frame.interp.bit_depth != 12 {
            return Err(anyhow!(
                "A frame with bit_depth=12 is required. Convert the bit depth of the frame!"
            ));
        }

        let interp = Raw { bit_depth: 8, ..frame.interp };
        let sink_buffer = unsafe {
            context.get_uninit_cpu_buffer(interp.required_bytes())
        };

        let push_constants = compute_shader::ty::PushConstantData { width: interp.width as u32 };

        let layout = self.pipeline.layout().descriptor_set_layouts()[0].clone();
        let set = Arc::new({
            let mut builder = PersistentDescriptorSet::start(layout);
            builder.add_buffer(frame.storage.cpu_accessible_buffer())?;
            builder.add_buffer(sink_buffer.cpu_accessible_buffer())?;
            builder.build()?
        });

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
            .dispatch([interp.width as u32 / 16 / 2, interp.height as u32 / 32, 1])?;
        let command_buffer = builder.build()?;

        let future =
            now(self.device.clone()).then_execute(self.queue.clone(), command_buffer)?.then_signal_fence_and_flush()?;
        
        future.wait(None).unwrap();
        Ok(Payload::from(Frame { interp, storage: sink_buffer }))
    }

    fn get_caps(&self) -> Caps { self.input.get_caps() }
}
