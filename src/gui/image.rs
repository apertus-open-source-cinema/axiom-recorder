use crate::{frame::rgb_frame::RgbFrame, gpu::gpu_util::CpuAccessibleBufferReadView};
use narui::{layout::Maximal, *};
use std::sync::Arc;
use vulkano::{
    buffer::BufferView,
    command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage},
    descriptor_set::PersistentDescriptorSet,
    device::DeviceOwned,
    format::Format::R8_UNORM,
    pipeline::{
        blend::{AttachmentBlend, BlendFactor, BlendOp},
        GraphicsPipeline,
        PipelineBindPoint,
    },
    render_pass::{RenderPass, Subpass},
};

mod vertex_shader {
    vulkano_shaders::shader! {
        ty: "vertex",
        src: "
            #version 450
            layout(push_constant) uniform PushConstantData {
                uint width;
                uint height;
                float z_index;
                vec2 origin;
                vec2 size;
            } params;

            layout(location = 0) out vec2 tex_coords;

            void main() {
                int idx = gl_VertexIndex;
                int top = idx & 1;
                int left = (idx & 2) / 2;

                tex_coords = vec2(0) + vec2(top, left);

                vec2 pos = params.origin + vec2(top, left) * params.size;
                gl_Position = vec4(pos, params.z_index, 1.0);
            }
        "
    }
}
mod fragment_shader {
    vulkano_shaders::shader! {
        ty: "fragment",
        src: "
            #version 450

            layout(push_constant) uniform PushConstantData {
                uint width;
                uint height;
                float z_index;
                vec2 origin;
                vec2 size;
            } params;

            layout(location = 0) in vec2 tex_coords;
            layout(location = 0) out vec4 f_color;

            layout(set = 0, binding = 0, r8) uniform readonly imageBuffer buf;

            vec3 get_px(int x, int y) {
                return vec3(
                    imageLoad(buf, y * int(params.width) * 3 + x * 3 + 0).r,
                    imageLoad(buf, y * int(params.width) * 3 + x * 3 + 1).r,
                    imageLoad(buf, y * int(params.width) * 3 + x * 3 + 2).r
                );
            }

            void main() {
                int x = int(tex_coords.x * params.width);
                int y = int(tex_coords.y * params.height);
                f_color = vec4(get_px(x, y), 1.);
            }
        "
    }
}

fn initialize(render_pass: Arc<RenderPass>) -> Arc<GraphicsPipeline> {
    let device = render_pass.device();

    let vs = vertex_shader::Shader::load(device.clone()).unwrap();
    let fs = fragment_shader::Shader::load(device.clone()).unwrap();

    Arc::new(
        GraphicsPipeline::start()
            .vertex_shader(vs.main_entry_point(), ())
            .triangle_strip()
            .viewports_dynamic_scissors_irrelevant(1)
            .fragment_shader(fs.main_entry_point(), ())
            .blend_collective(AttachmentBlend {
                enabled: true,
                color_op: BlendOp::Add,
                color_source: BlendFactor::SrcAlpha,
                color_destination: BlendFactor::OneMinusSrcAlpha,
                alpha_op: BlendOp::Max,
                alpha_source: BlendFactor::One,
                alpha_destination: BlendFactor::One,
                mask_red: true,
                mask_green: true,
                mask_blue: true,
                mask_alpha: true,
            })
            .render_pass(Subpass::from(render_pass.clone(), 0).unwrap())
            .build(device.clone())
            .unwrap(),
    )
}

#[widget]
pub fn image(image: Arc<RgbFrame>, context: &mut WidgetContext) -> FragmentInner {
    let cloned_image = image.clone();
    let device = context.vulkan_context.device.clone();

    let pipeline_descriptor_set = context.effect(
        |context| {
            let pipeline = initialize(context.vulkan_context.render_pass.clone());
            let source_buffer = CpuAccessibleBufferReadView::<u8>::from_buffer(
                device.clone(),
                cloned_image.buffer.clone(),
            )
            .unwrap()
            .as_cpu_accessible_buffer();
            let layout = pipeline.layout().descriptor_set_layouts()[0].clone();
            let set = Arc::new({
                let mut set = PersistentDescriptorSet::start(layout);
                set.add_buffer_view(Arc::new(BufferView::new(source_buffer, R8_UNORM).unwrap()))
                    .unwrap();
                set.build().unwrap()
            });

            (pipeline, set)
        },
        image.clone(),
    );
    let pipeline_descriptor_set = pipeline_descriptor_set.read();
    let pipeline = pipeline_descriptor_set.0.clone();
    let descriptor_set = pipeline_descriptor_set.1.clone();

    let queue = context
        .vulkan_context
        .queues
        .iter()
        .find(|&q| q.family().supports_graphics())
        .unwrap()
        .clone();

    let render_fn: Arc<RenderFnInner> = Arc::new(move |viewport, z_index, rect, res| {
        let origin = rect.pos / res * 2. - 1.;
        let size = rect.size / res * 2.;

        let push_constants = fragment_shader::ty::PushConstantData {
            origin: origin.into(),
            size: size.into(),
            z_index,
            width: image.width as u32,
            height: image.height as u32,
            _dummy0: Default::default(),
        };

        let mut builder = AutoCommandBufferBuilder::secondary_graphics(
            device.clone(),
            queue.family(),
            CommandBufferUsage::MultipleSubmit,
            pipeline.subpass().clone(),
        )
        .unwrap();

        builder
            .bind_descriptor_sets(
                PipelineBindPoint::Graphics,
                pipeline.layout().clone(),
                0,
                descriptor_set.clone(),
            )
            .bind_pipeline_graphics(pipeline.clone())
            .push_constants(pipeline.layout().clone(), 0, push_constants)
            .set_viewport(0, std::iter::once(viewport.clone()))
            .draw(4, 1, 0, 0)
            .unwrap();

        builder.build().unwrap()
    });

    FragmentInner::Leaf {
        render_object: RenderObject::Raw { render_fn },
        layout: Box::new(Maximal),
    }
}
