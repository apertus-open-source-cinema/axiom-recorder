use crate::pipeline_processing::{
    buffers::{CpuBuffer, GpuBuffer},
    frame::Frame,
    payload::Payload,
    prioritized_executor::PrioritizedReactor,
};
use anyhow::{anyhow, Result};
use std::{future::Future, sync::Arc};
use vulkano::{
    buffer::{BufferAccess, BufferUsage, CpuAccessibleBuffer},
    command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage, PrimaryCommandBuffer},
    device::{physical::PhysicalDevice, Device, DeviceExtensions, Queue},
    instance::Instance,
    Version,
};


#[derive(Clone)]
struct VulkanContext {
    device: Arc<Device>,
    queues: Vec<Arc<Queue>>,
}

// [u8 output priority, u56 frame number]
#[derive(Default, Copy, Clone, Ord, Eq, PartialEq, PartialOrd)]
pub struct Priority(u64);

impl Priority {
    const MASK: u64 = 0x0fff_ffff_ffff_ffff;

    pub fn new(output_priority: u8, frame_number: u64) -> Self {
        Self(((output_priority as u64) << 56) | (frame_number & Self::MASK))
    }

    pub fn for_frame(self, frame_number: u64) -> Self {
        Self((self.0 & !Self::MASK) | (frame_number & Self::MASK))
    }
}

impl std::fmt::Display for Priority {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let output_priority = self.0 >> 56;
        let frame_number = self.0 & Self::MASK;
        write!(f, "Priority(output = {}, frame = {})", output_priority, frame_number)
    }
}

#[derive(Clone)]
pub struct ProcessingContext {
    vulkan_device: Option<VulkanContext>,

    priority: Priority,
    prioritized_reactor: PrioritizedReactor<Priority>,
}
impl Default for ProcessingContext {
    fn default() -> Self {
        let vk_device =
            Instance::new(None, Version::V1_2, &vulkano_win::required_extensions(), None)
                .ok()
                .and_then(|instance| {
                    PhysicalDevice::enumerate(&instance).find_map(|physical| {
                        let queue_family = physical.queue_families().map(|qf| (qf, 0.5)); // All queues have the same priority
                        let device_ext = DeviceExtensions {
                            khr_swapchain: true,
                            khr_storage_buffer_storage_class: true,
                            khr_8bit_storage: true,
                            khr_shader_non_semantic_info: true,
                            ..(*physical.required_extensions())
                        };
                        Device::new(
                            physical,
                            physical.supported_features(),
                            &device_ext,
                            queue_family,
                        )
                        .ok()
                    })
                });
        match vk_device {
            None => {
                println!("using cpu only processing");
                Self {
                    vulkan_device: None,
                    priority: Default::default(),
                    prioritized_reactor: PrioritizedReactor::start(4),
                }
            }
            Some((device, queues)) => {
                println!("using gpu: {}", device.physical_device().properties().device_name);
                Self {
                    vulkan_device: Some(VulkanContext { device, queues: queues.collect() }),
                    priority: Default::default(),
                    prioritized_reactor: PrioritizedReactor::start(4),
                }
            }
        }
    }
}
impl ProcessingContext {
    pub fn for_priority(&self, priority: Priority) -> Self {
        Self {
            vulkan_device: self.vulkan_device.clone(),
            priority,
            prioritized_reactor: self.prioritized_reactor.clone(),
        }
    }
    pub fn for_frame(&self, frame: u64) -> Self {
        self.for_priority(self.priority.for_frame(frame))
    }

    pub unsafe fn get_uninit_cpu_buffer(&self, len: usize) -> CpuBuffer {
        if let Some(vulkan_context) = &self.vulkan_device {
            CpuAccessibleBuffer::uninitialized_array(
                vulkan_context.device.clone(),
                len as _,
                BufferUsage {
                    storage_buffer: true,
                    storage_texel_buffer: true,
                    transfer_source: true,
                    transfer_destination: true,
                    ..BufferUsage::none()
                },
                true,
            )
            .unwrap()
            .into()
        } else {
            unimplemented!()
        }
    }
    fn to_cpu_buffer<Interpretation: Clone + Send + Sync + 'static>(
        &self,
        frame: Arc<Frame<Interpretation, GpuBuffer>>,
    ) -> Result<Frame<Interpretation, CpuBuffer>> {
        let (device, queues) = self.require_vulkan()?;
        let queue =
            queues.iter().find(|&q| q.family().explicitly_supports_transfers()).unwrap().clone();

        let buffer = unsafe { self.get_uninit_cpu_buffer(frame.storage.untyped().size() as usize) };
        let mut cbb = AutoCommandBufferBuilder::primary(
            device,
            queue.family(),
            CommandBufferUsage::MultipleSubmit,
        )?;
        cbb.copy_buffer(frame.storage.typed(), buffer.cpu_accessible_buffer()).unwrap();
        let cb = cbb.build().unwrap();
        let future = match cb.execute(queue) {
            Ok(f) => f,
            Err(_) => unreachable!(),
        };

        // dropping this future blocks this thread until the gpu finished the work
        drop(future);

        Ok(Frame { interp: frame.interp.clone(), storage: buffer })
    }
    pub fn ensure_cpu_buffer<Interpretation: Clone + Send + Sync + 'static>(
        &self,
        payload: &Payload,
    ) -> anyhow::Result<Arc<Frame<Interpretation, CpuBuffer>>> {
        if let Ok(frame) = payload.downcast::<Frame<Interpretation, CpuBuffer>>() {
            Ok(frame)
        } else if let Ok(frame) = payload.downcast::<Frame<Interpretation, GpuBuffer>>() {
            Ok(Arc::new(self.to_cpu_buffer(frame)?))
        } else {
            Err(anyhow!(
                "wanted a frame with interpretation {}, but the payload was of type {}",
                std::any::type_name::<Interpretation>(),
                payload.type_name
            ))
        }
    }

    pub fn require_vulkan(&self) -> Result<(Arc<Device>, Vec<Arc<Queue>>)> {
        if let Some(vulkan_context) = &self.vulkan_device {
            Ok((vulkan_context.device.clone(), vulkan_context.queues.clone()))
        } else {
            Err(anyhow!("gpu required but not present"))
        }
    }

    pub fn spawn<O: Send + 'static>(
        &self,
        fut: impl Future<Output = O> + Send + 'static,
    ) -> impl Future<Output = O> {
        self.prioritized_reactor.spawn_with_priority(fut, self.priority)
    }
}
