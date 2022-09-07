use std::{
    io::Read,
    sync::{Arc, Mutex},
};

use crate::{
    pipeline_processing::{
        frame::{FrameInterpretation, FrameInterpretations},
        node::EOFError,
        parametrizable::{Parameterizable, Parameters, ParametersDescriptor},
        payload::Payload,
    },
    util::async_notifier::AsyncNotifier,
};
use anyhow::{Context, Result};


use crate::pipeline_processing::{
    frame::Frame,
    node::{Caps, NodeID, ProcessingNode},
    parametrizable::{ParameterType, ParameterTypeDescriptor},
    processing_context::ProcessingContext,
};
use async_trait::async_trait;

pub struct ZstdBlobReader {
    frame_and_file: AsyncNotifier<(
        u64,
        Arc<Mutex<zstd::stream::read::Decoder<'static, std::io::BufReader<std::fs::File>>>>,
    )>,
    interp: FrameInterpretations,
}
impl Parameterizable for ZstdBlobReader {
    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new()
            .with_interpretation()
            .with("file", ParameterTypeDescriptor::Mandatory(ParameterType::StringParameter))
    }
    fn from_parameters(
        mut options: Parameters,
        _is_input_to: &[NodeID],
        _context: &ProcessingContext,
    ) -> anyhow::Result<Self>
    where
        Self: Sized,
    {
        let path: String = options.take("file")?;
        let file = std::fs::File::open(path)?;

        let interp = options.get_interpretation()?;
        Ok(Self {
            frame_and_file: AsyncNotifier::new((
                0,
                Arc::new(Mutex::new(zstd::stream::read::Decoder::new(file)?)),
            )),
            interp,
        })
    }
}

#[async_trait]
impl ProcessingNode for ZstdBlobReader {
    async fn pull(
        &self,
        frame_number: u64,
        _puller_id: NodeID,
        context: &ProcessingContext,
    ) -> Result<Payload> {
        // TODO(robin): this is probably unsafe, because we can get multiple threads
        // waiting for the same frame_number, so then one will acquire the mutex
        // and the other will wait, but produce a frame for the same frame
        // number with different data, because it acquires the mutex after the
        // first thread is done
        let (_, decoder) =
            self.frame_and_file.wait(move |(frame_no, _)| *frame_no == frame_number).await;
        let mut decoder = decoder.lock().unwrap();

        let mut buffer = unsafe { context.get_uninit_cpu_buffer(self.interp.required_bytes()) };
        // dbg!(self.interp.required_bytes());
        buffer.as_mut_slice(move |buffer| {
            decoder.read_exact(buffer).context(EOFError)?;
            // dbg!(buffer[0], buffer[1], buffer[2]);
            anyhow::Result::<_, anyhow::Error>::Ok(())
        })?;
        self.frame_and_file.update(|(frame_no, _)| *frame_no = frame_number + 1);

        let payload = match self.interp {
            FrameInterpretations::Raw(interp) => Payload::from(Frame { storage: buffer, interp }),
            FrameInterpretations::Rgb(interp) => Payload::from(Frame { storage: buffer, interp }),
            FrameInterpretations::Rgba(interp) => Payload::from(Frame { storage: buffer, interp }),
        };

        Ok(payload)
    }

    fn get_caps(&self) -> Caps { Caps { frame_count: None, is_live: false } }
}
