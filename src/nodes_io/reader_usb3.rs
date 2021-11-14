// TODO: this is broken and should probably be rewritten trading a single coppy versus much
// simplified code ;).

use anyhow::{Context, Result};
use ft60x::ft60x::{FT60x, DEFAULT_PID, DEFAULT_VID};
use num::integer::div_ceil;
use std::{
    sync::{
        mpsc::{channel, Receiver, Sender, TryRecvError},
        Mutex,
    },
    thread,
};
use crate::pipeline_processing::execute::ProcessingStageLockWaiter;
use crate::pipeline_processing::frame::{Frame, FrameInterpretation, Raw};
use crate::pipeline_processing::parametrizable::{Parameterizable, Parameters, ParametersDescriptor};
use crate::pipeline_processing::payload::Payload;
use crate::pipeline_processing::processing_context::ProcessingContext;
use crate::pipeline_processing::processing_node::ProcessingNode;

const MIN_READ_LEN: usize = 2048 * 4;

pub struct Usb3Reader {
    data_rx: Mutex<Receiver<std::result::Result<Vec<u8>, ft60x::Error>>>,
    request_tx: Mutex<Sender<u64>>,
    interp: Raw,
    context: ProcessingContext,
}

impl Parameterizable for Usb3Reader {
    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new()
            .with_raw_interpretation()
    }

    fn from_parameters(parameters: &Parameters, context: ProcessingContext) -> Result<Self>
    where
        Self: Sized,
    {
        let interp = parameters.get_raw_interpretation()?;
        let ft60x = FT60x::new(DEFAULT_VID, DEFAULT_PID)
            .context("cant open ft60x. maybe try running with sudo?")?;
        let (empty_buffers_tx, full_buffers_rx, _) = ft60x.data_stream_mpsc(5);

        let (request_tx, request_rx) = channel();
        thread::Builder::new().name("usb3-allocate".to_string()).spawn(move || {
            let blanking = MIN_READ_LEN; // we need this to align the datastream to frames
            let frame_len = interp.required_bytes();
            let aligned_len = div_ceil(frame_len + blanking, MIN_READ_LEN) * MIN_READ_LEN;

            loop {
                match request_rx.try_recv() {
                    Ok(len_override) => {
                        empty_buffers_tx.send(vec![0u8; len_override as usize]).unwrap()
                    }
                    Err(TryRecvError::Empty) => {
                        empty_buffers_tx.send(vec![0u8; aligned_len as usize]).unwrap()
                    }
                    Err(e) => Err(e).unwrap(),
                }
            }
        })?;

        Ok(Self {
            data_rx: Mutex::new(full_buffers_rx),
            request_tx: Mutex::new(request_tx),
            interp: Raw { bit_depth, width, height, cfa },
            context
        })
    }
}

impl ProcessingNode for Usb3Reader {
    fn process(
        &self,
        _input: &mut Payload,
        _frame_lock: ProcessingStageLockWaiter,
    ) -> Result<Option<Payload>> {
        let mut wait_for_slip_size = 0;
        let buf = loop {
            let buf = self.data_rx.lock().unwrap().recv()??;
            if wait_for_slip_size == 0 {
                let u32_buf: &[u32] = bytemuck::cast_slice(&buf);
                let mut iter = u32_buf.iter().enumerate();
                let mut seen_zeros = 0;
                let offset = loop {
                    let (i, elem) = iter.next().unwrap();
                    if *elem == 0 {
                        seen_zeros += 1;
                    } else if seen_zeros > 10 {
                        break i * 4;
                    } else {
                        seen_zeros = 0;
                    }
                };

                if offset < MIN_READ_LEN as usize {
                    let buf_len = buf.len();
                    let sub_buffer = SubBuffer::from_buffer(buf, offset..buf_len);
                    break sub_buffer;
                } else {
                    let slip_len = offset as u64 / MIN_READ_LEN * MIN_READ_LEN;
                    println!("slip {}", slip_len);
                    self.request_tx.lock().unwrap().send(slip_len)?;
                    wait_for_slip_size = slip_len;
                }
            } else {
                println!("\t\t got {}", buf.len());
                if buf.len() == wait_for_slip_size as usize {
                    wait_for_slip_size = 0;
                    println!("end of slip");
                }
            }
        };

        Ok(Some(Payload::from(Frame { storage: buffer, interp: self.interp })))
    }
}
