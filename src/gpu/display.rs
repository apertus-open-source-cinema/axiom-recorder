use crate::{
    frame::rgb_frame::RgbFrame,
    gpu::gpu_util::{CpuAccessibleBufferReadView, VulkanContext},
    pipeline_processing::{
        parametrizable::{
            ParameterType::BoolParameter,
            ParameterTypeDescriptor::Optional,
            ParameterValue,
            Parameterizable,
            Parameters,
            ParametersDescriptor,
        },
        payload::Payload,
        processing_node::ProcessingNode,
    },
};
use anyhow::{Context, Result};
use std::{
    sync::{
        mpsc::{
            sync_channel,
            SyncSender,
            TrySendError::{Disconnected, Full},
        },
        Arc,
        Mutex,
        MutexGuard,
    },
    thread,
    thread::JoinHandle,
};
use vulkano::{
    buffer::{BufferUsage, BufferView, CpuAccessibleBuffer},
    command_buffer::{
        AutoCommandBufferBuilder,
        CommandBufferUsage::OneTimeSubmit,
        DynamicState,
        SubpassContents,
    },
    descriptor_set::PersistentDescriptorSet,
    device::DeviceOwned,
    format::Format::R8Unorm,
    image::{
        view::ImageView,
        AttachmentImage,
        ImageAccess,
        ImageUsage,
        SampleCount::Sample4,
        SwapchainImage,
    },
    pipeline::{viewport::Viewport, GraphicsPipeline, GraphicsPipelineAbstract},
    render_pass::{Framebuffer, FramebufferAbstract, RenderPass, Subpass},
    swapchain,
    swapchain::{AcquireError, Swapchain, SwapchainCreationError},
    sync,
    sync::{FlushError, GpuFuture},
};
use vulkano_win::VkSurfaceBuild;
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    platform::{run_return::EventLoopExtRunReturn, unix::EventLoopExtUnix},
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
    tx: Mutex<SyncSender<Option<Arc<RgbFrame>>>>,
    join_handle: Option<JoinHandle<()>>,
    blocking: bool,
}
impl Parameterizable for Display {
    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new()
            .with("blocking", Optional(BoolParameter, ParameterValue::BoolParameter(true)))
    }

    fn from_parameters(parameters: &Parameters) -> Result<Self>
    where
        Self: Sized,
    {
        let (tx, rx) = sync_channel(10);

        let join_handle = thread::Builder::new().name("display".to_string()).spawn(move || {
            let mut event_loop: EventLoop<()> = EventLoopExtUnix::new_any_thread();
            let device = VulkanContext::get().device;
            let surface = WindowBuilder::new()
                .with_title("axiom converter vulkan output")
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

            let caps = surface.capabilities(device.physical_device()).unwrap();
            let (mut swapchain, images) = {
                let alpha = caps.supported_composite_alpha.iter().next().unwrap();
                let format = caps.supported_formats[0].0;
                dbg!(format);
                let dimensions = surface.window().inner_size().into();
                Swapchain::start(device.clone(), surface.clone())
                    .usage(ImageUsage::color_attachment())
                    .num_images(caps.min_image_count)
                    .composite_alpha(alpha)
                    .dimensions(dimensions)
                    .format(format)
                    .build()
                    .expect("cant create swapchain")
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
                0..1,
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
                            match swapchain.recreate().dimensions(dimensions).build() {
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

                    let frame: core::result::Result<Option<Arc<RgbFrame>>, _> = rx.try_recv();
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

                    let layout = pipeline.layout().descriptor_set_layouts()[0].clone();
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
                    let mut builder = AutoCommandBufferBuilder::primary(
                        device.clone(),
                        queue.family(),
                        OneTimeSubmit,
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
                            set,
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
        })?;

        Ok(Self {
            tx: Mutex::new(tx),
            join_handle: Some(join_handle),
            blocking: parameters.get("blocking")?,
        })
    }
}
impl ProcessingNode for Display {
    fn process(
        &self,
        input: &mut Payload,
        _frame_lock: MutexGuard<u64>,
    ) -> Result<Option<Payload>> {
        let frame = input.downcast::<RgbFrame>().context("Wrong input format")?;
        if self.blocking {
            match self.tx.lock().unwrap().send(Some(frame)) {
                Ok(_) => Ok(Some(Payload::empty())),
                Err(_) => Ok(None),
            }
        } else {
            self.tx
                .lock()
                .unwrap()
                .try_send(Some(frame))
                .map(|_| Ok(Some(Payload::empty())))
                .unwrap_or_else(|e| match e {
                    Full(_) => Ok(Some(Payload::empty())),
                    Disconnected(_) => Ok(None),
                })
        }
    }
}
impl Drop for Display {
    fn drop(&mut self) {
        self.tx.lock().unwrap().send(None).unwrap();
        self.join_handle.take().unwrap().join().unwrap();
    }
}

/// This method is called once during initialization, then again whenever the
/// window is resized
fn window_size_dependent_setup(
    images: &[Arc<SwapchainImage<Window>>],
    render_pass: Arc<RenderPass>,
    dynamic_state: &mut DynamicState,
) -> Vec<Arc<dyn FramebufferAbstract + Send + Sync>> {
    let dimensions = images[0].dimensions();

    let viewport = Viewport {
        origin: [0.0, 0.0],
        dimensions: [dimensions.width() as f32, dimensions.height() as f32],
        depth_range: 0.0..1.0,
    };
    dynamic_state.viewports = Some(vec![viewport]);

    images
        .iter()
        .map(|image| {
            let intermediary = ImageView::new(
                AttachmentImage::transient_multisampled(
                    render_pass.device().clone(),
                    [dimensions.width(), dimensions.height()],
                    Sample4,
                    image.format(),
                )
                .unwrap(),
            )
            .unwrap();
            let view = ImageView::new(image.clone()).unwrap();
            Arc::new(
                Framebuffer::start(render_pass.clone())
                    .add(intermediary)
                    .unwrap()
                    .add(view)
                    .unwrap()
                    .build()
                    .unwrap(),
            ) as Arc<dyn FramebufferAbstract + Send + Sync>
        })
        .collect::<Vec<_>>()
}
