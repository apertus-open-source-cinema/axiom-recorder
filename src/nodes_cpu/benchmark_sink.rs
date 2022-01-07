use crate::pipeline_processing::{
    node::{ProcessingNode, SinkNode},
    parametrizable::{Parameterizable, Parameters, ParametersDescriptor},
    processing_context::ProcessingContext,
    puller::pull_unordered,
};
use anyhow::Result;
use async_trait::async_trait;

use crate::pipeline_processing::node::ProgressUpdate;
use std::{sync::Arc, time::Instant};


use crate::{
    pipeline_processing::{
        parametrizable::{ParameterType, ParameterTypeDescriptor},
        puller::OrderedPuller,
    },
    util::fps_report::FPSReporter,
};

pub struct BenchmarkSink {
    input: Arc<dyn ProcessingNode + Send + Sync>,
}

impl Parameterizable for BenchmarkSink {
    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new()
            .with("input", ParameterTypeDescriptor::Mandatory(ParameterType::NodeInput))
    }

    fn from_parameters(parameters: &Parameters, _context: &ProcessingContext) -> Result<Self> {
        Ok(Self { input: parameters.get("input")? })
    }
}


#[async_trait]
impl SinkNode for BenchmarkSink {
    async fn run(
        &self,
        context: &ProcessingContext,
        _progress_callback: Arc<dyn Fn(ProgressUpdate) + Send + Sync>,
    ) -> Result<()> {
        if let Some(frame_count) = self.input.get_caps().frame_count {
            let progress_callback = Arc::new(|_| {});

            println!("starting benchmark with {} frames...", frame_count);
            println!("warming cache...");
            pull_unordered(
                &context.clone(),
                progress_callback.clone(),
                self.input.clone(),
                0,
                move |_input, _frame_number| Ok(()),
            )
            .await?;
            println!("starting benchmark...");
            let start_time = Instant::now();
            pull_unordered(
                &context.clone(),
                progress_callback.clone(),
                self.input.clone(),
                0,
                move |_input, _frame_number| Ok(()),
            )
            .await?;
            let elapsed = (Instant::now() - start_time).as_secs_f64();
            println!(
                "time elapsed: {:.2}s for {:.2} frames. {:.2} fps",
                elapsed,
                frame_count,
                frame_count as f64 / elapsed
            );

            Ok(())
        } else {
            let puller = OrderedPuller::new(context, self.input.clone(), false, 0);
            let reporter = FPSReporter::new("pipeline");
            loop {
                puller.recv().unwrap();
                reporter.frame();
            }
        }
    }
}
