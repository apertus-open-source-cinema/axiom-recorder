use crate::pipeline_processing::{
    buffers::{CpuBuffer, GpuBuffer},
    frame::Frame,
    payload::Payload,
};
use anyhow::{anyhow, Result};
use std::sync::Arc;
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

#[derive(Clone)]
pub struct ProcessingContext {
    vulkan_device: Option<VulkanContext>,
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
                Self { vulkan_device: None }
            }
            Some((device, queues)) => {
                println!("using gpu: {}", device.physical_device().properties().device_name);
                Self { vulkan_device: Some(VulkanContext { device, queues: queues.collect() }) }
            }
        }
    }
}
impl ProcessingContext {
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
        payload: &mut Payload,
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
}
