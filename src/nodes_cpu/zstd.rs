use crate::{
    pipeline_processing::{
        frame::{Frame, FrameInterpretation},
        node::{Caps, EOFError, NodeID, ProcessingNode, Request},
        parametrizable::prelude::*,
        payload::Payload,
        processing_context::ProcessingContext,
    },
    util::async_notifier::AsyncNotifier,
};
use anyhow::{Context, Result};
use async_trait::async_trait;
use std::{
    io::Read,
    sync::{Arc, Mutex},
};

pub struct ZstdBlobReader {
    frame_and_file: AsyncNotifier<(
        u64,
        Arc<Mutex<zstd::stream::read::Decoder<'static, std::io::BufReader<std::fs::File>>>>,
    )>,
    interpretation: FrameInterpretation,
    context: ProcessingContext,
}
impl Parameterizable for ZstdBlobReader {
    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new().with_interpretation().with("file", Mandatory(StringParameter))
    }
    fn from_parameters(
        mut options: Parameters,
        _is_input_to: &[NodeID],
        context: &ProcessingContext,
    ) -> anyhow::Result<Self>
    where
        Self: Sized,
    {
        let path: String = options.take("file")?;
        let file = std::fs::File::open(path)?;

        let interpretation = options.get_interpretation()?;
        Ok(Self {
            frame_and_file: AsyncNotifier::new((
                0,
                Arc::new(Mutex::new(zstd::stream::read::Decoder::new(file)?)),
            )),
            interpretation,
            context: context.clone(),
        })
    }
}

#[async_trait]
impl ProcessingNode for ZstdBlobReader {
    async fn pull(&self, request: Request) -> Result<Payload> {
        // TODO(robin): this is probably unsafe, because we can get multiple threads
        // waiting for the same frame_number, so then one will acquire the mutex
        // and the other will wait, but produce a frame for the same frame
        // number with different data, because it acquires the mutex after the
        // first thread is done
        let frame_number = request.frame_number();
        let (_, decoder) =
            self.frame_and_file.wait(move |(frame_no, _)| *frame_no == frame_number).await;
        let mut decoder = decoder.lock().unwrap();

        let mut buffer =
            unsafe { self.context.get_uninit_cpu_buffer(self.interpretation.required_bytes()) };
        buffer.as_mut_slice(move |buffer| {
            decoder.read_exact(buffer).context(EOFError)?;
            Result::<_, anyhow::Error>::Ok(())
        })?;
        self.frame_and_file.update(|(frame_no, _)| *frame_no = frame_number + 1);

        Ok(Payload::from(Frame { interpretation: self.interpretation.clone(), storage: buffer }))
    }

    fn get_caps(&self) -> Caps { Caps { frame_count: None, random_access: false } }
}
