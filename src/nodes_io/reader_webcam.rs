use crate::{
    pipeline_processing::{
        frame::{Frame, FrameInterpretation, Rgb},
        node::{Caps, ProcessingNode},
        parametrizable::{
            ParameterType::IntRange,
            ParameterTypeDescriptor::Optional,
            ParameterValue,
            Parameterizable,
            Parameters,
            ParametersDescriptor,
        },
        payload::Payload,
        processing_context::ProcessingContext,
    },
    util::async_notifier::AsyncNotifier,
};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use std::{
    collections::VecDeque,
    mem,
    sync::{mpsc, Arc, Mutex},
    thread,
};
use v4l::{
    buffer::{Metadata, Type},
    device::Handle,
    v4l2,
    video::Capture,
    Device,
    Memory,
};
use v4l2_sys_mit::*;


pub struct WebcamInput {
    queue: AsyncNotifier<VecDeque<(u64, Payload)>>,
    last_frame_last_pulled: Mutex<(u64, u64)>,
}
impl Parameterizable for WebcamInput {
    const DESCRIPTION: Option<&'static str> =
        Some("read frames from a webcam (or webcam like source like a frame-grabber)");

    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new()
            .with("device", Optional(IntRange(0, i64::MAX), ParameterValue::IntRange(0)))
    }
    fn from_parameters(options: &Parameters, context: &ProcessingContext) -> anyhow::Result<Self> {
        let dev =
            Device::new(options.get::<u64>("device")? as usize).expect("Failed to open device");
        let format = dev.format()?;
        let interp = Rgb { width: format.width as u64, height: format.height as u64, fps: 10000.0 };
        let mut stream = CpuBufferQueueManager::new(&dev);
        stream.start();
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || loop {
            let (frame, metadata) = stream.dequeue();
            if metadata.bytesused > 0 {
                tx.send((frame, metadata.sequence)).unwrap();
            } else {
                println!("we got no data from the webcam :(");
            }
            stream.enqueue();
        });

        let queue = AsyncNotifier::new(VecDeque::new());
        let queue_clone = queue.clone();
        let context = context.clone();
        thread::spawn(move || loop {
            let mut buffer = unsafe { context.get_uninit_cpu_buffer(interp.required_bytes()) };
            let (src_buf, sequence) = rx.recv().unwrap();
            buffer.as_mut_slice(|buffer| {
                for (src, dst) in src_buf.chunks_exact(3).zip(buffer.chunks_exact_mut(3)) {
                    dst[0] = src[2];
                    dst[1] = src[1];
                    dst[2] = src[0];
                }
            });

            queue_clone.update(move |queue| {
                queue.push_back((
                    sequence as u64,
                    Payload::from(Frame { storage: buffer, interp }),
                ));
            });
        });

        Ok(Self { queue, last_frame_last_pulled: Default::default() })
    }
}

#[async_trait]
impl ProcessingNode for WebcamInput {
    async fn pull(&self, frame_number: u64, context: &ProcessingContext) -> Result<Payload> {
        self.queue
            .wait(move |queue| {
                queue
                    .iter()
                    .map(|(n, _)| n)
                    .max()
                    .map(|latest| *latest >= frame_number)
                    .unwrap_or(false)
            })
            .await;

        self.queue.update(|queue| {
            let pos = queue.iter().position(|(n, _)| *n == frame_number).ok_or(anyhow!(
                "Frame {} is not present anymore in webcam input buffer",
                frame_number
            ))?;
            let payload = queue.get(pos).unwrap().1.clone();

            let mut last_frame_last_pulled = self.last_frame_last_pulled.lock().unwrap();
            if last_frame_last_pulled.0 < context.frame() {
                queue.retain(|(n, _)| n >= &last_frame_last_pulled.1)
            }

            last_frame_last_pulled.0 = context.frame();
            last_frame_last_pulled.1 = frame_number;

            Ok(payload)
        })
    }

    fn get_caps(&self) -> Caps { Caps { frame_count: None, is_live: true } }
}

pub struct CpuBufferQueueManager {
    handle: Arc<Handle>,
    buffer_size: usize,
    buffers: Vec<Option<Vec<u8>>>,
}
impl CpuBufferQueueManager {
    fn new(dev: &Device) -> Self {
        let handle = dev.handle();
        let num_buffers = 4usize;

        let mut v4l2_reqbufs: v4l2_requestbuffers;
        unsafe {
            v4l2_reqbufs = mem::zeroed();
            v4l2_reqbufs.type_ = Type::VideoCapture as u32;
            v4l2_reqbufs.count = num_buffers as u32;
            v4l2_reqbufs.memory = Memory::UserPtr as u32;
            v4l2::ioctl(
                handle.fd(),
                v4l2::vidioc::VIDIOC_REQBUFS,
                &mut v4l2_reqbufs as *mut _ as *mut std::os::raw::c_void,
            )
            .unwrap();
        }

        let mut v4l2_fmt: v4l2_format;
        unsafe {
            v4l2_fmt = mem::zeroed();
            v4l2_fmt.type_ = Type::VideoCapture as u32;
            v4l2::ioctl(
                handle.fd(),
                v4l2::vidioc::VIDIOC_G_FMT,
                &mut v4l2_fmt as *mut _ as *mut std::os::raw::c_void,
            )
            .unwrap();
        }
        let buffer_size = unsafe { v4l2_fmt.fmt.pix.sizeimage } as usize;

        let buffers = vec![None; num_buffers];
        let mut to_return = Self { handle, buffers, buffer_size };
        for _ in 0..(num_buffers - 1) {
            to_return.enqueue()
        }

        to_return
    }

    fn start(&mut self) {
        unsafe {
            let mut typ = Type::VideoCapture as u32;
            v4l2::ioctl(
                self.handle.fd(),
                v4l2::vidioc::VIDIOC_STREAMON,
                &mut typ as *mut _ as *mut std::os::raw::c_void,
            )
            .unwrap();
        }
    }

    fn enqueue(&mut self) {
        let buffer = vec![0u8; self.buffer_size];
        let index = self.buffers.iter().position(|x| x.is_none()).unwrap();

        let mut v4l2_buf: v4l2_buffer;
        unsafe {
            v4l2_buf = mem::zeroed();
            v4l2_buf.type_ = Type::VideoCapture as u32;
            v4l2_buf.memory = Memory::UserPtr as u32;
            v4l2_buf.index = index as u32;
            v4l2_buf.m.userptr = buffer.as_ptr() as std::os::raw::c_ulong;
            v4l2_buf.length = buffer.len() as u32;
            v4l2::ioctl(
                self.handle.fd(),
                v4l2::vidioc::VIDIOC_QBUF,
                &mut v4l2_buf as *mut _ as *mut std::os::raw::c_void,
            )
            .unwrap();
        }

        self.buffers[index].replace(buffer);
    }

    fn dequeue(&mut self) -> (Vec<u8>, Metadata) {
        let mut v4l2_buf: v4l2_buffer;
        unsafe {
            v4l2_buf = mem::zeroed();
            v4l2_buf.type_ = Type::VideoCapture as u32;
            v4l2_buf.memory = Memory::UserPtr as u32;
            v4l2::ioctl(
                self.handle.fd(),
                v4l2::vidioc::VIDIOC_DQBUF,
                &mut v4l2_buf as *mut _ as *mut std::os::raw::c_void,
            )
            .unwrap();
        }

        let metadata = Metadata {
            bytesused: v4l2_buf.bytesused,
            flags: v4l2_buf.flags.into(),
            field: v4l2_buf.field,
            timestamp: v4l2_buf.timestamp.into(),
            sequence: v4l2_buf.sequence,
        };
        let index = v4l2_buf.index as usize;

        (self.buffers[index].take().unwrap(), metadata)
    }
}
