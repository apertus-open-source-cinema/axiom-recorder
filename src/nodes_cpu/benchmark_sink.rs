use crate::pipeline_processing::{
    node::{InputProcessingNode, NodeID, ProgressUpdate, SinkNode},
    parametrizable::{Parameterizable, Parameters, ParametersDescriptor},
    processing_context::ProcessingContext,
    puller::{pull_ordered, pull_unordered},
};
use anyhow::Result;
use async_trait::async_trait;

use std::{sync::Arc, time::Instant};


use crate::{pipeline_processing::parametrizable::prelude::*, util::fps_report::FPSReporter};

pub struct BenchmarkSink {
    input: InputProcessingNode,
    priority: u8,
}

impl Parameterizable for BenchmarkSink {
    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new()
            .with("input", Mandatory(NodeInputParameter))
            .with("priority", WithDefault(U8(), ParameterValue::IntRangeValue(0)))
    }

    fn from_parameters(
        mut parameters: Parameters,
        _is_input_to: &[NodeID],
        _context: &ProcessingContext,
    ) -> Result<Self> {
        Ok(Self { input: parameters.take("input")?, priority: parameters.take("priority")? })
    }
}

fn mean_and_std(data: &[f64]) -> (f64, f64) {
    let mean = data.iter().sum::<f64>() / (data.len() as f64);
    let var = data
        .iter()
        .map(|v| {
            let diff = v - mean;
            diff * diff
        })
        .sum::<f64>()
        / (data.len() as f64 - 1.0);

    (mean, var.sqrt())
}


#[async_trait]
impl SinkNode for BenchmarkSink {
    async fn run(
        &self,
        context: &ProcessingContext,
        progress_callback: Arc<dyn Fn(ProgressUpdate) + Send + Sync>,
    ) -> Result<()> {
        if let Some(frame_count) = self.input.get_caps().frame_count {
            // let progress_callback = Arc::new(|_| {});

            println!("starting benchmark with {} frames...", frame_count);
            println!("warming cache...");
            let res = pull_unordered(
                &context.clone(),
                self.priority,
                progress_callback.clone(),
                self.input.clone_for_same_puller(),
                None,
                move |_input, _frame_number| Ok(()),
            )
            .await;
            println!("res = {:?}", res);
            println!("starting benchmark...");

            let mut durations = vec![];
            for _ in 0..10 {
                let start_time = Instant::now();
                pull_unordered(
                    &context.clone(),
                    self.priority,
                    progress_callback.clone(),
                    self.input.clone_for_same_puller(),
                    None,
                    move |_input, _frame_number| Ok(()),
                )
                .await?;
                durations.push((Instant::now() - start_time).as_secs_f64());
            }
            let (mean, std) = mean_and_std(&durations);
            println!(
                "time elapsed: ({:.2} +- {:.2})ms for {:.2} frames. {:.2} fps",
                mean * 1000.,
                std * 1000.,
                frame_count,
                frame_count as f64 / mean
            );

            Ok(())
        } else {
            let rx = pull_ordered(
                context,
                self.priority,
                progress_callback,
                self.input.clone_for_same_puller(),
                None,
            );
            let reporter = FPSReporter::new("pipeline");
            loop {
                rx.recv_async().await?;
                reporter.frame();
            }
        }
    }
}
