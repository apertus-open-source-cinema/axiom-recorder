use crate::pipeline_processing::{
    buffers::GpuBuffer,
    frame::Frame,
    gpu_util::ensure_gpu_buffer_frame,
    node::{Caps, InputProcessingNode, NodeID, ProcessingNode, Request},
    parametrizable::prelude::*,
    payload::Payload,
    processing_context::ProcessingContext,
};
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use std::{
    fs::File,
    io::{BufReader, Read},
    sync::Arc,
};
use vulkano::{
    buffer::{BufferUsage, DeviceLocalBuffer},
    command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage::OneTimeSubmit},
    descriptor_set::{persistent::PersistentDescriptorSet, WriteDescriptorSet},
    device::{Device, Queue},
    image::{view::ImageView, ImageViewAbstract, ImmutableImage},
    pipeline::{ComputePipeline, Pipeline, PipelineBindPoint},
    sampler::{Filter, Sampler, SamplerAddressMode, SamplerCreateInfo},
    sync::GpuFuture,
    DeviceSize,
};

// generated by the macro
#[allow(clippy::needless_question_mark)]
mod compute_shader {
    vulkano_shaders::shader! {
        ty: "compute",
        path: "src/nodes_gpu/lut_3d.glsl"
    }
}

pub struct Lut3d {
    device: Arc<Device>,
    pipeline: Arc<ComputePipeline>,
    queue: Arc<Queue>,
    input: InputProcessingNode,
    lut_image_view: Arc<dyn ImageViewAbstract>,
    lut_sampler: Arc<Sampler>,
}

impl Parameterizable for Lut3d {
    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new()
            .with("input", Mandatory(NodeInputParameter))
            .with("file", Mandatory(StringParameter))
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

        let lut_image = read_lut_texture_from_cube_file(parameters.take("file")?, queue.clone())?;
        let lut_sampler = Sampler::new(
            device.clone(),
            SamplerCreateInfo {
                mag_filter: Filter::Linear,
                min_filter: Filter::Linear,
                address_mode: [
                    SamplerAddressMode::Repeat,
                    SamplerAddressMode::Repeat,
                    SamplerAddressMode::Repeat,
                ],
                ..Default::default()
            },
        )
        .unwrap();

        Ok(Lut3d {
            device,
            pipeline,
            queue,
            input: parameters.take("input")?,
            lut_image_view: ImageView::new_default(lut_image).unwrap(),
            lut_sampler,
        })
    }
}

fn read_cube_size(file_contents: &str) -> Result<usize> {
    for (line_idx, line) in file_contents.lines().enumerate() {
        if line.is_empty() {
            continue;
        }

        if line.starts_with("LUT_3D_SIZE") {
            let parts: Vec<_> = line.split(' ').collect();
            if parts.len() < 2 {
                return Err(anyhow!(
                    "Invalid cube file: LUT_3D_SIZE is missing an argument (line {})",
                    line_idx + 1
                ));
            }
            return Ok(parts[1].parse()?);
        }
    }

    Err(anyhow!("Invalid cube file: Couldn't find LUT_3D_SIZE"))
}

fn read_cube_data(file_contents: &str, mut on_value: impl FnMut(f32, f32, f32)) -> Result<()> {
    for (line_idx, line) in file_contents.lines().enumerate() {
        if line.is_empty() {
            continue;
        }

        let first_char = line.chars().next().unwrap();
        if first_char.is_ascii_digit() {
            let parts: Vec<_> = line.trim().split(' ').collect();

            if parts.len() != 3 {
                // throw error
                return Err(anyhow!(
                    "Invalid cube file: Expected 3 numbers in line (line {})",
                    line_idx + 1
                ));
            }

            let r = parts[0].parse::<f32>()?;
            let g = parts[1].parse::<f32>()?;
            let b = parts[2].parse::<f32>()?;

            on_value(r, g, b);
        }
    }

    Ok(())
}

fn read_lut_texture_from_cube_file(path: String, queue: Arc<Queue>) -> Result<Arc<ImmutableImage>> {
    let file = File::open(path)?;

    let mut reader = BufReader::new(file);
    let mut file_contents = String::new();
    reader.read_to_string(&mut file_contents)?;

    let size = read_cube_size(&file_contents)?;
    let mut buffer: Vec<u8> = Vec::with_capacity(size.pow(3) * 4);

    read_cube_data(&file_contents, |r, g, b| {
        buffer.push((b * 255.0) as u8);
        buffer.push((g * 255.0) as u8);
        buffer.push((r * 255.0) as u8);
        buffer.push(255);
    })?;

    if size.pow(3) * 4 != buffer.len() {
        let received_lines = buffer.len() / 4;
        let expected_lines = size.pow(3);
        return Err(anyhow!(
            "Invalid cube file: Expected {0:}x{0:}x{0:} = {1:} lines, found {2:} lines",
            size,
            expected_lines,
            received_lines,
        ));
    }

    let (image, _image_fut) = ImmutableImage::from_iter(
        buffer.into_iter(),
        vulkano::image::ImageDimensions::Dim3d {
            width: size as u32,
            height: size as u32,
            depth: size as u32,
        },
        vulkano::image::MipmapsCount::One,
        vulkano::format::Format::B8G8R8A8_UNORM,
        queue,
    )?;

    Ok(image)
}

#[async_trait]
impl ProcessingNode for Lut3d {
    async fn pull(&self, request: Request) -> Result<Payload> {
        let input = self.input.pull(request).await?;

        let (frame, fut) = ensure_gpu_buffer_frame(&input, self.queue.clone())
            .context("Wrong input forma for Lut3d")?;

        let sink_buffer = DeviceLocalBuffer::<[u8]>::array(
            self.device.clone(),
            frame.interpretation.required_bytes() as DeviceSize,
            BufferUsage {
                storage_buffer: true,
                storage_texel_buffer: true,
                transfer_src: true,
                ..BufferUsage::none()
            },
            std::iter::once(self.queue.family()),
        )?;

        let push_constants = compute_shader::ty::PushConstantData {
            width: frame.interpretation.width as _,
            height: frame.interpretation.height as _,
        };

        let layout = self.pipeline.layout().set_layouts()[0].clone();
        let set = PersistentDescriptorSet::new(
            layout,
            [
                WriteDescriptorSet::buffer(0, frame.storage.untyped()),
                WriteDescriptorSet::buffer(1, sink_buffer.clone()),
                WriteDescriptorSet::image_view_sampler(
                    2,
                    self.lut_image_view.clone(),
                    self.lut_sampler.clone(),
                ),
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
                (frame.interpretation.width as u32 + 31) / 32,
                (frame.interpretation.height as u32 + 31) / 32,
                1,
            ])?;
        let command_buffer = builder.build()?;

        let future =
            fut.then_execute(self.queue.clone(), command_buffer)?.then_signal_fence_and_flush()?;

        future.wait(None).unwrap();
        Ok(Payload::from(Frame {
            interpretation: frame.interpretation.clone(),
            storage: GpuBuffer::from(sink_buffer),
        }))
    }

    fn get_caps(&self) -> Caps { self.input.get_caps() }
}
