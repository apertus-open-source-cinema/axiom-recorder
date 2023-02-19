use crate::{
    nodes_gpu::base_gpu_node::{BindingValue, GpuNode},
    pipeline_processing::{
        frame::{ColorInterpretation, FrameInterpretation},
        node::NodeID,
        parametrizable::prelude::*,
        processing_context::ProcessingContext,
    },
};
use anyhow::{anyhow, bail, Result};
use indoc::indoc;
use std::{
    collections::HashMap,
    fs::File,
    io::{BufReader, Read},
    sync::Arc,
};
use vulkano::{
    device::Queue,
    image::{view::ImageView, ImmutableImage},
    sampler::{Filter, Sampler, SamplerAddressMode, SamplerCreateInfo},
};

pub struct Lut3d {
    sampler: BindingValue,
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

        let sampler =
            BindingValue::Sampler((ImageView::new_default(lut_image).unwrap(), lut_sampler));

        Ok(Lut3d { sampler })
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

impl GpuNode for Lut3d {
    fn get_glsl(&self) -> String {
        indoc!(
            "
            layout(...) uniform sampler3D lut_sampler;

            dtype3 produce_pixel(uvec2 pos) {
                return dtype3(texture(lut_sampler, read_pixel(pos)));
            }
        "
        )
        .to_string()
    }

    fn get_binding(
        &self,
        frame_interpretation: &FrameInterpretation,
    ) -> Result<HashMap<String, BindingValue>> {
        if frame_interpretation.color_interpretation != ColorInterpretation::Rgb {
            bail!("Lut3d node only supports rgb images")
        }

        Ok(HashMap::from([("lut_sampler".to_string(), self.sampler.clone())]))
    }
}


#[cfg(test)]
mod tests {
    use super::Lut3d;
    use crate::{
        nodes_gpu::base_gpu_node::GpuNodeImpl,
        nodes_util::null_source::NullFrameSource,
        pipeline_processing::{
            frame::{ColorInterpretation, Compression, FrameInterpretation, SampleInterpretation},
            node::{InputProcessingNode, NodeID, ProcessingNode, Request},
            parametrizable::{
                prelude::{NodeInputValue, StringValue},
                Parameterizable,
                Parameters,
            },
            processing_context::TEST_CONTEXT,
        },
    };
    use std::{collections::HashMap, sync::Arc};

    fn test_basic(lut_path: &str) {
        let source = NodeInputValue(InputProcessingNode::new(
            NodeID::default(),
            Arc::new(NullFrameSource {
                context: TEST_CONTEXT.clone(),
                interpretation: FrameInterpretation {
                    width: 1920,
                    height: 1080,
                    fps: Some(24.0),
                    color_interpretation: ColorInterpretation::Rgb,
                    sample_interpretation: SampleInterpretation::FP16,
                    compression: Compression::Uncompressed,
                },
            }),
        ));
        let parameters = Parameters::new(HashMap::from([
            ("input".to_string(), source),
            ("file".to_string(), StringValue(lut_path.to_string())),
        ]))
        .add_defaults(GpuNodeImpl::<Lut3d>::describe_parameters());
        let dut = GpuNodeImpl::<Lut3d>::from_parameters(parameters, &[], &TEST_CONTEXT).unwrap();

        for _ in 0..10 {
            let _payload = pollster::block_on(dut.pull(Request::new(0, 0))).unwrap();
        }
    }

    macro_rules! test_file {
        ($fname:expr) => {
            concat!(env!("CARGO_MANIFEST_DIR"), "/resources/test/", $fname)
        };
    }

    #[test]
    fn test_17_point() { test_basic(test_file!("luts/17point.cube")) }
    #[test]
    fn test_33_point() { test_basic(test_file!("luts/33point.cube")) }
    #[test]
    fn test_65_point() { test_basic(test_file!("luts/65point.cube")) }
}
