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
    descriptor_set::persistent::PersistentDescriptorSet,
    device::{Device, Queue},
    image::{view::ImageView, ImageViewAbstract, ImmutableImage},
    pipeline::{ComputePipeline, PipelineBindPoint},
    sampler::{Filter, MipmapMode, Sampler, SamplerAddressMode},
    sync::GpuFuture,
    DeviceSize,
};

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
    input: Arc<dyn ProcessingNode + Send + Sync>,
    lut_image_view: Arc<dyn ImageViewAbstract>,
    lut_sampler: Arc<Sampler>,
}

impl Parameterizable for Lut3d {
    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new()
            .with("input", ParameterTypeDescriptor::Mandatory(ParameterType::NodeInput))
            .with("file", ParameterTypeDescriptor::Mandatory(ParameterType::StringParameter))
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

        let lut_image = read_lut_texture_from_cube_file(parameters.get("file")?, queue.clone())?;
        let lut_sampler = Sampler::new(
            device.clone(),
            Filter::Linear,
            Filter::Linear,
            MipmapMode::Nearest,
            SamplerAddressMode::Repeat,
            SamplerAddressMode::Repeat,
            SamplerAddressMode::Repeat,
            0.0,
            1.0,
            0.0,
            0.0,
        )
        .unwrap();

        Ok(Lut3d {
            device,
            pipeline,
            queue,
            input: parameters.get("input")?,
            lut_image_view: ImageView::new(lut_image).unwrap(),
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
    async fn pull(&self, frame_number: u64, context: &ProcessingContext) -> Result<Payload> {
        let input = self.input.pull(frame_number, context).await?;

        let (frame, fut) =
            ensure_gpu_buffer::<Rgb>(&input, self.queue.clone()).context("Wrong input format")?;

        let sink_buffer = DeviceLocalBuffer::<[u8]>::array(
            self.device.clone(),
            frame.interp.required_bytes() as DeviceSize,
            BufferUsage { storage_buffer: true, storage_texel_buffer: true, ..BufferUsage::none() },
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
            builder.add_sampled_image(self.lut_image_view.clone(), self.lut_sampler.clone())?;
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
            .dispatch([
                (frame.interp.width as u32 + 31) / 32,
                (frame.interp.height as u32 + 31) / 32,
                1,
            ])?;
        let command_buffer = builder.build()?;

        let future =
            fut.then_execute(self.queue.clone(), command_buffer)?.then_signal_fence_and_flush()?;

        future.wait(None).unwrap();
        Ok(Payload::from(Frame { interp: frame.interp, storage: GpuBuffer::from(sink_buffer) }))
    }

    fn get_caps(&self) -> Caps { self.input.get_caps() }
}
