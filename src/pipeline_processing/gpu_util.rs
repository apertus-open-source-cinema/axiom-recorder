use anyhow::anyhow;


use crate::pipeline_processing::{
    buffers::{CpuBuffer, GpuBuffer},
    frame::Frame,
    payload::Payload,
};
use std::sync::Arc;
use vulkano::{
    buffer::{BufferUsage, ImmutableBuffer},
    command_buffer::{
        AutoCommandBufferBuilder,
        CommandBufferUsage,
        CopyBufferInfo,
        PrimaryCommandBuffer,
    },
    device::Queue,
    sync::GpuFuture,
};

pub fn to_immutable_buffer(
    frame: Arc<Frame<CpuBuffer>>,
    queue: Arc<Queue>,
) -> (Frame<GpuBuffer>, impl GpuFuture) {
    let device = queue.device();

    let (buffer, fut) = unsafe {
        let (buffer, init) = ImmutableBuffer::uninitialized_array(
            device.clone(),
            frame.storage.len() as _,
            BufferUsage {
                storage_buffer: true,
                storage_texel_buffer: true,
                transfer_dst: true,
                transfer_src: true,
                ..BufferUsage::none()
            },
        )
        .unwrap();

        let mut cbb = AutoCommandBufferBuilder::primary(
            device.clone(),
            queue.family(),
            CommandBufferUsage::MultipleSubmit,
        )
        .unwrap();

        cbb.copy_buffer(CopyBufferInfo::buffers(frame.storage.cpu_accessible_buffer(), init))
            .unwrap();
        let cb = cbb.build().unwrap();
        let future = match cb.execute(queue) {
            Ok(f) => f,
            Err(_) => unreachable!(),
        };

        (buffer, future)
    };

    (Frame { interpretation: frame.interpretation.clone(), storage: buffer.into() }, fut)
}

pub fn ensure_gpu_buffer_frame(
    payload: &Payload,
    queue: Arc<Queue>,
) -> anyhow::Result<(Arc<Frame<GpuBuffer>>, impl GpuFuture)> {
    if let Ok(frame) = payload.downcast::<Frame<CpuBuffer>>() {
        let (buf, fut) = to_immutable_buffer(frame, queue);
        Ok((Arc::new(buf), fut.boxed()))
    } else if let Ok(frame) = payload.downcast::<Frame<GpuBuffer>>() {
        Ok((frame, vulkano::sync::now(queue.device().clone()).boxed()))
    } else {
        Err(anyhow!(
            "wanted a frame with type {}, but the payload was of type {}",
            std::any::type_name::<Frame<GpuBuffer>>(),
            payload.type_name
        ))
    }
}
