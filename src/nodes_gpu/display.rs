use crate::pipeline_processing::{
    frame::Rgb,
    gpu_util::ensure_gpu_buffer,
    node::{ProcessingNode, ProgressUpdate, SinkNode},
    parametrizable::{
        ParameterType,
        ParameterType::BoolParameter,
        ParameterTypeDescriptor::{Mandatory, Optional},
        ParameterValue,
        Parameterizable,
        Parameters,
        ParametersDescriptor,
    },
    processing_context::ProcessingContext,
    puller::OrderedPuller,
};
use anyhow::{Context, Result};
use async_trait::async_trait;
use std::{
    sync::{mpsc::TryRecvError, Arc},
    time::{Duration, Instant},
};
use vulkano::{
    buffer::BufferView,
    command_buffer::{
        AutoCommandBufferBuilder,
        CommandBufferUsage::OneTimeSubmit,
        SubpassContents,
    },
    descriptor_set::PersistentDescriptorSet,
    format::Format::R8_UNORM,
    image::{view::ImageView, ImageAccess, ImageUsage, SwapchainImage},
    pipeline::{viewport::Viewport, GraphicsPipeline, PipelineBindPoint},
    render_pass::{Framebuffer, FramebufferAbstract, RenderPass, Subpass},
    swapchain,
    swapchain::{AcquireError, PresentMode, Swapchain, SwapchainCreationError},
    sync,
    sync::{FlushError, GpuFuture},
};
use vulkano_win::VkSurfaceBuild;
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{Window, WindowBuilder},
};


mod vertex_shader {
    vulkano_shaders::shader! {
        ty: "vertex",
        src: "
            #version 450

            layout(push_constant) uniform PushConstantData {
                uint width;
                uint height;
                uint window_width;
                uint window_height;
            } params;

            layout(location = 0) out vec2 tex_coords;

            void main() {
                float wanted_aspect = float(params.width) / float(params.height);
                float is_aspect = float(params.window_width) / float(params.window_height);
                float aspect = is_aspect / wanted_aspect;

                int idx = gl_VertexIndex;
                int top = idx & 1;
                int left = (idx & 2) / 2;
                gl_Position = vec4(
                    (2 * top - 1) / (aspect > 1 ? aspect : 1),
                    (2 * left - 1) * (aspect < 1 ? aspect : 1),
                    0.0,
                    1.0
                );
                tex_coords = vec2(top, left);
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
                uint window_width;
                uint window_height;
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


pub struct Display {
    mailbox: bool,
    live: bool,
    input: Arc<dyn ProcessingNode + Send + Sync>,
}

impl Parameterizable for Display {
    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::default()
            .with("mailbox", Optional(BoolParameter, ParameterValue::BoolParameter(false)))
            .with("live", Optional(BoolParameter, ParameterValue::BoolParameter(false)))
            .with("input", Mandatory(ParameterType::NodeInput))
    }

    fn from_parameters(parameters: &Parameters, _context: &ProcessingContext) -> Result<Self> {
        Ok(Self {
            mailbox: parameters.get("mailbox")?,
            live: parameters.get("live")?,
            input: parameters.get("input")?,
        })
    }
}


#[async_trait]
impl SinkNode for Display {
    async fn run(
        &self,
        context: &ProcessingContext,
        _progress_callback: Arc<dyn Fn(ProgressUpdate) + Send + Sync>,
    ) -> Result<()> {
        let puller = OrderedPuller::new(context, self.input.clone());
        let (device, queues) = context.require_vulkan()?;
        let live = self.live;

        let event_loop: EventLoop<()> = EventLoop::new();
        let surface = WindowBuilder::new()
            .with_title("axiom converter vulkan output")
            .build_vk_surface(&event_loop, device.instance().clone())
            .unwrap();
        let queue = queues
            .iter()
            .find(|&q| {
                q.family().supports_graphics() && surface.is_supported(q.family()).unwrap_or(false)
            })
            .unwrap()
            .clone();

        let caps = surface.capabilities(device.physical_device()).unwrap();
        let mut dimensions;
        let (mut swapchain, images) = {
            let alpha = caps.supported_composite_alpha.iter().next().unwrap();
            let format = caps.supported_formats[0].0;
            dimensions = surface.window().inner_size().into();
            let present_mode = if self.mailbox { PresentMode::Mailbox } else { PresentMode::Fifo };
            Swapchain::start(device.clone(), surface.clone())
                .usage(ImageUsage::color_attachment())
                .num_images(caps.min_image_count)
                .composite_alpha(alpha)
                .dimensions(dimensions)
                .format(format)
                .present_mode(present_mode)
                .build()
                .expect("cant create swapchain")
        };

        let vs = vertex_shader::Shader::load(device.clone()).unwrap();
        let fs = fragment_shader::Shader::load(device.clone()).unwrap();

        let render_pass = Arc::new(
            vulkano::single_pass_renderpass!(device.clone(),
                attachments: {
                    color: {
                        load: Clear,
                        store: Store,
                        format: swapchain.format(),
                        samples: 1,
                    }
                },
                pass: {
                    color: [color],
                    depth_stencil: {}
                }
            )
            .unwrap(),
        );

        let pipeline = Arc::new(
            GraphicsPipeline::start()
                .vertex_shader(vs.main_entry_point(), ())
                .triangle_strip()
                .viewports_dynamic_scissors_irrelevant(1)
                .fragment_shader(fs.main_entry_point(), ())
                .render_pass(Subpass::from(render_pass.clone(), 0).unwrap())
                .build(device.clone())
                .unwrap(),
        );

        let (mut framebuffers, mut viewport) =
            window_size_dependent_setup(&images, render_pass.clone());
        let mut recreate_swapchain = false;
        let mut previous_frame_end = Some(sync::now(device.clone()).boxed());
        let mut next_frame_time = Instant::now();
        let mut source_buffer = None;
        let mut source_future = None;
        let mut frame_width = 1u32;
        let mut frame_height = 1u32;
        let _frame_number = 0;

        event_loop.run(move |event, _, control_flow| match event {
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                *control_flow = ControlFlow::Exit;
            }
            Event::WindowEvent { event: WindowEvent::Resized(_), .. } => {
                recreate_swapchain = true;
            }
            Event::RedrawEventsCleared => {
                previous_frame_end.as_mut().unwrap().cleanup_finished();
                if recreate_swapchain {
                    dimensions = surface.window().inner_size().into();
                    let (new_swapchain, new_images) =
                        match swapchain.recreate().dimensions(dimensions).build() {
                            Ok(r) => r,
                            Err(SwapchainCreationError::UnsupportedDimensions) => return,
                            Err(e) => panic!("Failed to recreate swapchain: {:?}", e),
                        };

                    swapchain = new_swapchain;
                    let (new_framebuffers, new_viewport) =
                        window_size_dependent_setup(&new_images, render_pass.clone());
                    framebuffers = new_framebuffers;
                    viewport = new_viewport;
                    recreate_swapchain = false;
                }

                let now = Instant::now();
                let needs_new_frame = live || if now > next_frame_time { true } else { false };

                if needs_new_frame {
                    match puller.try_recv() {
                        Err(TryRecvError::Empty) => {}
                        Err(TryRecvError::Disconnected) => *control_flow = ControlFlow::Exit,
                        Ok(ref frame) => {
                            let (frame, fut) = ensure_gpu_buffer::<Rgb>(frame, queue.clone())
                                .context("Wrong input format")
                                .unwrap();
                            frame_width = frame.interp.width as _;
                            frame_height = frame.interp.height as _;

                            next_frame_time += Duration::from_secs_f64(1.0 / frame.interp.fps);
                            source_buffer = Some(frame);
                            source_future = Some(fut);
                        }
                    }
                }
                if source_buffer.is_none() {
                    *control_flow = ControlFlow::Poll;
                    return;
                }

                let (image_num, suboptimal, acquire_future) =
                    match swapchain::acquire_next_image(swapchain.clone(), None) {
                        Ok(r) => r,
                        Err(AcquireError::OutOfDate) => {
                            recreate_swapchain = true;
                            return;
                        }
                        Err(e) => panic!("Failed to acquire next image: {:?}", e),
                    };

                if suboptimal {
                    recreate_swapchain = true;
                }

                let layout = pipeline.layout().descriptor_set_layouts()[0].clone();
                let set = Arc::new({
                    let mut set = PersistentDescriptorSet::start(layout);
                    set.add_buffer_view(Arc::new(
                        BufferView::new(source_buffer.as_ref().unwrap().storage.typed(), R8_UNORM)
                            .unwrap(),
                    ))
                    .unwrap();
                    set.build().unwrap()
                });

                let push_constants = fragment_shader::ty::PushConstantData {
                    width: frame_width,
                    height: frame_height,
                    window_width: dimensions[0],
                    window_height: dimensions[1],
                };

                let clear_values = vec![[0.0, 0.0, 0.0, 1.0].into()];
                let mut builder = AutoCommandBufferBuilder::primary(
                    device.clone(),
                    queue.family(),
                    OneTimeSubmit,
                )
                .unwrap();
                builder
                    .bind_pipeline_graphics(pipeline.clone())
                    .begin_render_pass(
                        framebuffers[image_num].clone(),
                        SubpassContents::Inline,
                        clear_values,
                    )
                    .unwrap()
                    .set_viewport(0, viewport.clone())
                    .bind_descriptor_sets(
                        PipelineBindPoint::Graphics,
                        pipeline.layout().clone(),
                        0,
                        set,
                    )
                    .push_constants(pipeline.layout().clone(), 0, push_constants)
                    .draw(4, 1, 0, 0)
                    .unwrap()
                    .end_render_pass()
                    .unwrap();
                let command_buffer = builder.build().unwrap();

                let mut future = previous_frame_end.take().unwrap().join(acquire_future).boxed();
                if let Some(fut) = source_future.take() {
                    future = future.join(fut).boxed();
                }


                let future = future
                    .then_execute(queue.clone(), command_buffer)
                    .unwrap()
                    .then_swapchain_present(queue.clone(), swapchain.clone(), image_num)
                    .then_signal_fence_and_flush();

                match future {
                    Ok(future) => {
                        previous_frame_end = Some(future.boxed());
                    }
                    Err(FlushError::OutOfDate) => {
                        recreate_swapchain = true;
                        previous_frame_end = Some(sync::now(device.clone()).boxed());
                    }
                    Err(e) => {
                        println!("Failed to flush future: {:?}", e);
                        previous_frame_end = Some(sync::now(device.clone()).boxed());
                    }
                }
            }
            _ => {}
        });
    }
}

/// This method is called once during initialization, then again whenever the
/// window is resized
fn window_size_dependent_setup(
    images: &[Arc<SwapchainImage<Window>>],
    render_pass: Arc<RenderPass>,
) -> (Vec<Arc<dyn FramebufferAbstract + Send + Sync>>, Vec<Viewport>) {
    let dimensions = images[0].dimensions();

    let viewport = Viewport {
        origin: [0.0, 0.0],
        dimensions: [dimensions.width() as f32, dimensions.height() as f32],
        depth_range: 0.0..1.0,
    };
    let viewport = vec![viewport];

    (
        images
            .iter()
            .map(|image| {
                let view = ImageView::new(image.clone()).unwrap();
                Arc::new(
                    Framebuffer::start(render_pass.clone()).add(view).unwrap().build().unwrap(),
                ) as Arc<dyn FramebufferAbstract + Send + Sync>
            })
            .collect::<Vec<_>>(),
        viewport,
    )
}
