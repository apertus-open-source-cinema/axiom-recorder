use crate::pipeline_processing::{
    parametrizable::{Parameterizable, Parameters, ParametersDescriptor},
    processing_node::ProcessingNode,
};
use anyhow::{Context, Result};
use std::sync::{Mutex, MutexGuard};

use vulkano::{
    buffer::{BufferUsage, BufferView, CpuAccessibleBuffer},
    command_buffer::{AutoCommandBufferBuilder, DynamicState, SubpassContents},
    descriptor::descriptor_set::PersistentDescriptorSet,
    device::{Device, DeviceExtensions},
    format::{Format, R8G8B8A8Unorm, R8Unorm},
    framebuffer::{Framebuffer, FramebufferAbstract, RenderPassAbstract, Subpass},
    image::{Dimensions, ImageUsage, ImmutableImage, SwapchainImage},
    instance::{Instance, PhysicalDevice},
    pipeline::{viewport::Viewport, GraphicsPipeline},
    sampler::{Filter, MipmapMode, Sampler, SamplerAddressMode},
    swapchain,
    swapchain::{
        AcquireError,
        ColorSpace,
        FullscreenExclusive,
        PresentMode,
        SurfaceTransform,
        Swapchain,
        SwapchainCreationError,
    },
    sync,
    sync::{FlushError, GpuFuture},
};

use crate::{
    frame::rgba_frame::RgbaFrame,
    gpu::gpu_util::{CpuAccessibleBufferReadView, VulkanContext},
    pipeline_processing::payload::Payload,
};
use gstreamer::glib::bitflags::_core::any::Any;
use itertools::join;
use std::{
    sync::{
        mpsc::{channel, sync_channel, Sender, SyncSender},
        Arc,
    },
    thread::{spawn, JoinHandle},
};
use vulkano_win::VkSurfaceBuild;
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    platform::{desktop::EventLoopExtDesktop, unix::EventLoopExtUnix},
    window::{Window, WindowBuilder},
};


mod vertex_shader {
    vulkano_shaders::shader! {
        ty: "vertex",
        src: "
            #version 450
            layout(location = 0) in vec2 position;
            layout(location = 0) out vec2 tex_coords;
            void main() {
                gl_Position = vec4(position, 0.0, 1.0);
                tex_coords = (position + vec2(1.)) / vec2(2.);
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

            layout( set = 0, binding = 0, r8 ) uniform imageBuffer buf;

            vec3 get_px(int x, int y) {
                return vec3(
                    imageLoad(buf, y * int(params.width) * 4 + x * 4 + 0).r,
                    imageLoad(buf, y * int(params.width) * 4 + x * 4 + 1).r,
                    imageLoad(buf, y * int(params.width) * 4 + x * 4 + 2).r
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
    tx: Mutex<SyncSender<Option<Arc<RgbaFrame>>>>,
    join_handle: Option<JoinHandle<()>>,
}
impl Parameterizable for Display {
    fn describe_parameters() -> ParametersDescriptor { ParametersDescriptor::new() }

    fn from_parameters(parameters: &Parameters) -> Result<Self>
    where
        Self: Sized,
    {
        let (tx, rx) = sync_channel(3);

        let join_handle = spawn(move || {
            let mut event_loop: EventLoop<()> = EventLoopExtUnix::new_any_thread();
            let device = VulkanContext::get().device;
            let surface = WindowBuilder::new()
                .build_vk_surface(&event_loop, device.instance().clone())
                .unwrap();
            let queue = VulkanContext::get()
                .queues
                .iter()
                .find(|&q| {
                    q.family().supports_graphics()
                        && surface.is_supported(q.family()).unwrap_or(false)
                })
                .unwrap()
                .clone();

            let (mut swapchain, images) = {
                let caps = surface.capabilities(device.physical_device()).unwrap();
                let alpha = caps.supported_composite_alpha.iter().next().unwrap();
                let format = caps.supported_formats[0].0;
                let dimensions: [u32; 2] = surface.window().inner_size().into();

                Swapchain::new(
                    device.clone(),
                    surface.clone(),
                    caps.min_image_count,
                    format,
                    dimensions,
                    1,
                    ImageUsage::color_attachment(),
                    &queue,
                    SurfaceTransform::Identity,
                    alpha,
                    PresentMode::Fifo,
                    FullscreenExclusive::Default,
                    true,
                    ColorSpace::SrgbNonLinear,
                )
                .unwrap()
            };

            #[derive(Default, Debug, Clone)]
            struct Vertex {
                position: [f32; 2],
            }
            vulkano::impl_vertex!(Vertex, position);

            let vertex_buffer = CpuAccessibleBuffer::<[Vertex]>::from_iter(
                device.clone(),
                BufferUsage::all(),
                false,
                [
                    Vertex { position: [-1., -1.] },
                    Vertex { position: [-1., 1.] },
                    Vertex { position: [1., -1.] },
                    Vertex { position: [1., 1.] },
                ]
                .iter()
                .cloned(),
            )
            .unwrap();

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
                    .vertex_input_single_buffer::<Vertex>()
                    .vertex_shader(vs.main_entry_point(), ())
                    .triangle_strip()
                    .viewports_dynamic_scissors_irrelevant(1)
                    .fragment_shader(fs.main_entry_point(), ())
                    .blend_alpha_blending()
                    .render_pass(Subpass::from(render_pass.clone(), 0).unwrap())
                    .build(device.clone())
                    .unwrap(),
            );

            let mut dynamic_state = DynamicState {
                line_width: None,
                viewports: None,
                scissors: None,
                compare_mask: None,
                write_mask: None,
                reference: None,
            };
            let mut framebuffers =
                window_size_dependent_setup(&images, render_pass.clone(), &mut dynamic_state);
            let mut recreate_swapchain = false;
            let mut previous_frame_end = Some(sync::now(device.clone()).boxed());
            let mut source_buffer = CpuAccessibleBuffer::from_iter(
                VulkanContext::get().device,
                BufferUsage::all(),
                true,
                (0..1),
            )
            .unwrap();
            let mut frame_width = 1u32;
            let mut frame_height = 1u32;
            event_loop.run_return(move |event, _, control_flow| match event {
                Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                    *control_flow = ControlFlow::Exit;
                }
                Event::WindowEvent { event: WindowEvent::Resized(_), .. } => {
                    recreate_swapchain = true;
                }
                Event::RedrawEventsCleared => {
                    previous_frame_end.as_mut().unwrap().cleanup_finished();
                    if recreate_swapchain {
                        let dimensions: [u32; 2] = surface.window().inner_size().into();
                        let (new_swapchain, new_images) =
                            match swapchain.recreate_with_dimensions(dimensions) {
                                Ok(r) => r,
                                Err(SwapchainCreationError::UnsupportedDimensions) => return,
                                Err(e) => panic!("Failed to recreate swapchain: {:?}", e),
                            };

                        swapchain = new_swapchain;
                        framebuffers = window_size_dependent_setup(
                            &new_images,
                            render_pass.clone(),
                            &mut dynamic_state,
                        );
                        recreate_swapchain = false;
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

                    let frame: core::result::Result<Option<Arc<RgbaFrame>>, _> = rx.try_recv();
                    match frame {
                        Err(_) => {}
                        Ok(None) => *control_flow = ControlFlow::Exit,
                        Ok(Some(frame)) => {
                            source_buffer = CpuAccessibleBufferReadView::<u8>::from_buffer(
                                frame.buffer.clone(),
                            )
                            .unwrap()
                            .as_cpu_accessible_buffer();
                            frame_width = frame.width as u32;
                            frame_height = frame.height as u32;
                        }
                    }

                    let layout = pipeline.layout().descriptor_set_layout(0).unwrap();
                    let set = Arc::new(
                        PersistentDescriptorSet::start(layout.clone())
                            .add_buffer_view(
                                BufferView::new(source_buffer.clone(), R8Unorm).unwrap(),
                            )
                            .unwrap()
                            .build()
                            .unwrap(),
                    );

                    let push_constants = fragment_shader::ty::PushConstantData {
                        width: frame_width,
                        height: frame_height,
                    };

                    let clear_values = vec![[0.0, 0.0, 0.0, 1.0].into()];
                    let mut builder = AutoCommandBufferBuilder::primary_one_time_submit(
                        device.clone(),
                        queue.family(),
                    )
                    .unwrap();
                    builder
                        .begin_render_pass(
                            framebuffers[image_num].clone(),
                            SubpassContents::Inline,
                            clear_values,
                        )
                        .unwrap()
                        .draw(
                            pipeline.clone(),
                            &dynamic_state,
                            vertex_buffer.clone(),
                            set.clone(),
                            push_constants,
                        )
                        .unwrap()
                        .end_render_pass()
                        .unwrap();
                    let command_buffer = builder.build().unwrap();

                    let future = previous_frame_end
                        .take()
                        .unwrap()
                        .join(acquire_future)
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
        });

        Ok(Self { tx: Mutex::new(tx), join_handle: Some(join_handle) })
    }
}
impl ProcessingNode for Display {
    fn process(&self, input: &mut Payload, frame_lock: MutexGuard<u64>) -> Result<Option<Payload>> {
        let frame = input.downcast::<RgbaFrame>().context("Wrong input format")?;
        self.tx.lock().unwrap().send(Some(frame))?;
        Ok(Some(Payload::empty()))
    }
}
impl Drop for Display {
    fn drop(&mut self) {
        self.tx.lock().unwrap().send(None);
        self.join_handle.take().unwrap().join().unwrap();
    }
}

/// This method is called once during initialization, then again whenever the
/// window is resized
fn window_size_dependent_setup(
    images: &[Arc<SwapchainImage<Window>>],
    render_pass: Arc<dyn RenderPassAbstract + Send + Sync>,
    dynamic_state: &mut DynamicState,
) -> Vec<Arc<dyn FramebufferAbstract + Send + Sync>> {
    let dimensions = images[0].dimensions();

    let viewport = Viewport {
        origin: [0.0, 0.0],
        dimensions: [dimensions[0] as f32, dimensions[1] as f32],
        depth_range: 0.0..1.0,
    };
    dynamic_state.viewports = Some(vec![viewport]);

    images
        .iter()
        .map(|image| {
            Arc::new(
                Framebuffer::start(render_pass.clone())
                    .add(image.clone())
                    .unwrap()
                    .build()
                    .unwrap(),
            ) as Arc<dyn FramebufferAbstract + Send + Sync>
        })
        .collect::<Vec<_>>()
}
