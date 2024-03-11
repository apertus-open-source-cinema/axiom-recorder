use crate::{
    pipeline_processing::{
        frame::{Frame, FrameInterpretation, Rgb},
        node::{Caps, NodeID, ProcessingNode, Request},
        parametrizable::prelude::*,
        payload::Payload,
        processing_context::ProcessingContext,
    },
    util::async_notifier::AsyncNotifier,
};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use std::{
    mem,
    sync::{Arc, RwLock},
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
    queue: AsyncNotifier<(u64, u64)>,
    stream: RwLock<CpuBufferQueueManager>,
    interp: Rgb,
    context: ProcessingContext,
}

impl Parameterizable for WebcamInput {
    const DESCRIPTION: Option<&'static str> =
        Some("read frames from a webcam (or webcam like source like a frame-grabber)");

    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new().with("device", Optional(NaturalWithZero()))
    }
    fn from_parameters(
        mut options: Parameters,
        _is_input_to: &[NodeID],
        context: &ProcessingContext,
    ) -> anyhow::Result<Self> {
        let dev =
            Device::new(options.take::<u64>("device")? as usize).expect("Failed to open device");
        let format = dev.format()?;
        let interp = Rgb { width: format.width as u64, height: format.height as u64, fps: 10000.0 };
        let mut stream = CpuBufferQueueManager::new(&dev);
        stream.start();

        Ok(Self {
            queue: Default::default(),
            stream: RwLock::new(stream),
            interp,
            context: context.clone(),
        })
    }
}

#[async_trait]
impl ProcessingNode for WebcamInput {
    async fn pull(&self, request: Request) -> Result<Payload> {
        // println!("pulling {frame_number}");
        let frame_number = request.frame_number();
        let (_, prev_seq) = self.queue.wait(move |(num, _)| *num == frame_number).await;
        let (frame, metadata) = {
            let mut stream = self.stream.write().unwrap();
            let (frame, metadata) = stream.dequeue();
            stream.enqueue();
            (frame, metadata)
        };
        // println!("got {frame_number}, {}", metadata.sequence);
        self.queue.update(|(num, prev_seq)| {
            *num = frame_number + 1;
            *prev_seq = metadata.sequence as _
        });
        if metadata.bytesused == 0 {
            return Err(anyhow!(
                "Zero size frame from v4l2, seq = {}, frame_number = {}",
                metadata.sequence,
                frame_number
            ));
        }
        if prev_seq + 1 != metadata.sequence as _ {
            println!("Frame slipped for frame_number = {frame_number}, prev_seq = {prev_seq}, sequence = {}", metadata.sequence);
        }
        // dbg!(frame_number, metadata.sequence);
        // frame, metadata.sequence

        let mut buffer =
            unsafe { self.context.get_uninit_cpu_buffer(self.interp.required_bytes()) };
        buffer.as_mut_slice(|buffer| {
            for (src, dst) in frame.chunks_exact(3).zip(buffer.chunks_exact_mut(3)) {
                dst[0] = src[2];
                dst[1] = src[1];
                dst[2] = src[0];
            }
        });

        return Ok(Payload::from(Frame { storage: buffer, interp: self.interp }));
    }

    fn get_caps(&self) -> Caps { Caps { frame_count: None, random_access: false } }
}

pub struct CpuBufferQueueManager {
    handle: Arc<Handle>,
    buffer_size: usize,
    buffers: Vec<Option<Vec<u8>>>,
}
impl CpuBufferQueueManager {
    fn new(dev: &Device) -> Self {
        let handle = dev.handle();
        let num_buffers = 8usize;

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
        for _ in 0..num_buffers {
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
