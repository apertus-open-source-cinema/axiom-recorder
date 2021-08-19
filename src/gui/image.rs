use crate::{frame::rgb_frame::RgbFrame, gpu::gpu_util::CpuAccessibleBufferReadView};
use narui::{style::Style, *};
use std::sync::Arc;
use vulkano::{
    buffer::{BufferUsage, BufferView, CpuAccessibleBuffer},
    descriptor_set::PersistentDescriptorSet,
    format::Format::R8Unorm,
    pipeline::{vertex::BuffersDefinition, GraphicsPipeline, GraphicsPipelineAbstract},
    render_pass::{RenderPass, Subpass},
};

mod vertex_shader {
    vulkano_shaders::shader! {
        ty: "vertex",
        src: "
            #version 450
            layout(location = 0) in vec2 position;
            layout(location = 1) in vec2 tex;
            layout(location = 0) out vec2 tex_coords;
            void main() {
                gl_Position = vec4(position, 0.0, 1.0);
                tex_coords = tex;
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
            } params;

            layout(location = 0) in vec2 tex_coords;
            layout(location = 0) out vec4 f_color;

            layout( set = 0, binding = 0, r8 ) uniform readonly imageBuffer buf;

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
#[derive(Default, Debug, Clone)]
struct Vertex {
    position: [f32; 2],
    tex: [f32; 2],
}
vulkano::impl_vertex!(Vertex, position, tex);

fn vertex_buffer(rect: Rect, res: Vec2) -> Arc<CpuAccessibleBuffer<[Vertex]>> {
    let top_left = rect.pos / res * 2. - 1.;
    let bottom_right = (rect.pos + rect.size) / res * 2. - 1.;
    let device = VulkanContext::get().device;
    CpuAccessibleBuffer::<[Vertex]>::from_iter(
        device.clone(),
        BufferUsage::all(),
        false,
        [
            Vertex { position: [top_left.x, top_left.y], tex: [0., 0.] },
            Vertex { position: [top_left.x, bottom_right.y], tex: [0., 1.] },
            Vertex { position: [bottom_right.x, top_left.y], tex: [1., 0.] },
            Vertex { position: [bottom_right.x, bottom_right.y], tex: [1., 1.] },
        ]
            .iter()
            .cloned(),
    )
        .unwrap()
}

fn initialize(
    render_pass: Arc<RenderPass>,
) -> Arc<GraphicsPipeline<BuffersDefinition>> {
    let device = VulkanContext::get().device;

    let vs = vertex_shader::Shader::load(device.clone()).unwrap();
    let fs = fragment_shader::Shader::load(device.clone()).unwrap();

    Arc::new(
        GraphicsPipeline::start()
            .vertex_input_single_buffer::<Vertex>()
            .vertex_shader(vs.main_entry_point(), ())
            .triangle_strip()
            .viewports_dynamic_scissors_irrelevant(1)
            .fragment_shader(fs.main_entry_point(), ())
            .blend_alpha_blending()
            .render_pass(Subpass::from(render_pass, 0).unwrap())
            .build(device)
            .unwrap(),
    )
}

#[widget(style = Default::default())]
pub fn image(image: Arc<RgbFrame>, style: Style, context: Context) -> FragmentInner {
    let cloned_image = image.clone();
    let render_fn: Arc<RenderFnInner> =
        Arc::new(move |render_pass, command_buffer_builder, dynamic_state, rect, res| {
            let pipeline = initialize(render_pass);
            let vertex_buffer = vertex_buffer(rect, res);

            let source_buffer =
                CpuAccessibleBufferReadView::<u8>::from_buffer(cloned_image.buffer.clone())
                    .unwrap()
                    .as_cpu_accessible_buffer();

            let layout = pipeline.layout().descriptor_set_layouts()[0].clone();
            let set = Arc::new(
                PersistentDescriptorSet::start(layout)
                    .add_buffer_view(BufferView::new(source_buffer, R8Unorm).unwrap())
                    .unwrap()
                    .build()
                    .unwrap(),
            );

            let push_constants = fragment_shader::ty::PushConstantData {
                width: image.width as u32,
                height: image.height as u32,
            };
            command_buffer_builder
                .draw(pipeline, dynamic_state, vertex_buffer, set, push_constants)
                .expect("image draw failed");
        });
    FragmentInner {
        children: vec![],
        layout_object: Some(LayoutObject {
            style,
            measure_function: None,
            render_objects: vec![(KeyPart::RenderObject(0), RenderObject::Raw { render_fn })],
        }),
    }
}
