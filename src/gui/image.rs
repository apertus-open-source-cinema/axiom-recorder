use crate::pipeline_processing::{
    buffers::GpuBuffer,
    frame::{Frame, Rgb},
};
use derivative::Derivative;
use narui::{layout::Maximal, *};
use std::sync::Arc;
use vulkano::{
    buffer::view::{BufferView, BufferViewCreateInfo},
    command_buffer::{
        AutoCommandBufferBuilder,
        CommandBufferInheritanceInfo,
        CommandBufferInheritanceRenderPassType,
        CommandBufferUsage,
    },
    descriptor_set::{PersistentDescriptorSet, WriteDescriptorSet},
    device::DeviceOwned,
    format::Format::R8_UNORM,
    image::SampleCount,
    pipeline::{
        graphics::{
            input_assembly::{InputAssemblyState, PrimitiveTopology},
            multisample::MultisampleState,
            viewport::{Scissor, ViewportState},
        },
        GraphicsPipeline,
        PartialStateMode,
        Pipeline,
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

    let vs = vertex_shader::load(device.clone()).unwrap();
    let fs = fragment_shader::load(device.clone()).unwrap();

    GraphicsPipeline::start()
        .vertex_shader(vs.entry_point("main").unwrap(), ())
        .input_assembly_state(InputAssemblyState {
            topology: PartialStateMode::Fixed(PrimitiveTopology::TriangleStrip),
            ..Default::default()
        })
        .viewport_state(ViewportState::FixedScissor {
            scissors: vec![Scissor::irrelevant()],
            viewport_count_dynamic: false,
        })
        .multisample_state(MultisampleState {
            rasterization_samples: SampleCount::Sample4,
            ..MultisampleState::new()
        })
        .fragment_shader(fs.entry_point("main").unwrap(), ())
        .render_pass(Subpass::from(render_pass.clone(), 0).unwrap())
        .build(device.clone())
        .unwrap()
}

#[derive(Derivative)]
#[derivative(Clone(bound = ""))]
pub struct ArcPartialEqHelper<T>(pub Arc<T>);
impl<T> PartialEq for ArcPartialEqHelper<T> {
    fn eq(&self, other: &Self) -> bool { Arc::ptr_eq(&self.0, &other.0) }
}

#[widget]
pub fn image(image: Arc<Frame<Rgb, GpuBuffer>>, context: &mut WidgetContext) -> FragmentInner {
    let cloned_image = ArcPartialEqHelper(image.clone());
    let device = context.vulkan_context.device.clone();

    let pipeline_descriptor_set = context.effect(
        |context| {
            let pipeline = initialize(context.vulkan_context.render_pass.clone());
            let layout = pipeline.layout().set_layouts()[0].clone();
            let set = PersistentDescriptorSet::new(
                layout,
                [WriteDescriptorSet::buffer_view(
                    0,
                    BufferView::new(
                        cloned_image.0.storage.untyped(),
                        BufferViewCreateInfo { format: Some(R8_UNORM), ..Default::default() },
                    )
                    .unwrap(),
                )],
            )
            .unwrap();

            (pipeline, set)
        },
        cloned_image.clone(),
    );

    let vulkan_context = context.vulkan_context.clone();
    let render_fn: Arc<RenderFnInner> = Arc::new(move |viewport, z_index, rect, res| {
        let pipeline_descriptor_set = pipeline_descriptor_set.read();
        let pipeline = pipeline_descriptor_set.0.clone();
        let descriptor_set = pipeline_descriptor_set.1.clone();

        let origin = rect.inner().pos / *res.inner() * 2. - 1.;
        let size = rect.inner().size / *res.inner() * 2.;

        let push_constants = fragment_shader::ty::PushConstantData {
            origin: origin.into(),
            size: size.into(),
            z_index,
            width: image.interp.width as u32,
            height: image.interp.height as u32,
            _dummy0: Default::default(),
        };

        let queue =
            vulkan_context.queues.iter().find(|&q| q.family().supports_graphics()).unwrap().clone();
        let subpass = Subpass::from(vulkan_context.render_pass.clone(), 0).unwrap();

        let mut builder = AutoCommandBufferBuilder::secondary(
            device.clone(),
            queue.family(),
            CommandBufferUsage::MultipleSubmit,
            CommandBufferInheritanceInfo {
                render_pass: Some(CommandBufferInheritanceRenderPassType::from(subpass)),
                ..Default::default()
            },
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
