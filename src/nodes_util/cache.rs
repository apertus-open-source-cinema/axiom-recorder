use crate::{
    pipeline_processing::{
        node::{Caps, InputProcessingNode, NodeID, PinCache, ProcessingNode, Request},
        parametrizable::prelude::*,
        payload::Payload,
        processing_context::ProcessingContext,
    },
    util::async_notifier::AsyncNotifier,
};
use anyhow::Result;
use async_trait::async_trait;
use parking_lot::Mutex;
use std::{collections::HashMap, sync::Arc};

pub struct Cache {
    input: InputProcessingNode,
    cache: AsyncNotifier<Arc<Mutex<HashMap<u64, (Payload, usize)>>>>,
    nodes_to_feed: usize,
    capacity: usize,
}

impl Parameterizable for Cache {
    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new()
            .with("input", Mandatory(NodeInputParameter))
            .with("size", Optional(NaturalGreaterZero()))
    }

    fn from_parameters(
        mut parameters: Parameters,
        is_input_to: &[NodeID],
        _context: &ProcessingContext,
    ) -> Result<Self> {
        let capacity = parameters.take("size")?;

        Ok(Self {
            input: parameters.take("input")?,
            cache: AsyncNotifier::new(Arc::new(Mutex::new(HashMap::with_capacity(capacity)))),
            nodes_to_feed: is_input_to.len(),
            capacity,
        })
    }
}

#[async_trait]
impl ProcessingNode for Cache {
    async fn pull(&self, request: Request) -> Result<Payload> {
        let frame_number = request.frame_number();
        let capacity = self.capacity;

        // we need this loop in case the cache changes its content while between the
        // wait and the update
        loop {
            let request = request.clone();
            self.cache
                .wait(move |cache: &Arc<Mutex<HashMap<u64, (Payload, usize)>>>| {
                    let cache = cache.lock();
                    cache.contains_key(&frame_number) || cache.len() < capacity
                })
                .await;
            let result: Result<_> = self
                .cache
                .update(|cache| {
                    let cache = cache.clone();
                    async move {
                        let mut cache = cache.lock();
                        if cache.contains_key(&frame_number) {
                            let entry = cache.get_mut(&frame_number).unwrap();
                            let payload = entry.0.clone();
                            if request.get_extra::<PinCache>().is_none() {
                                entry.1 -= 1;
                                if entry.1 == 0 {
                                    cache.remove(&frame_number);
                                }
                            }
                            Ok(Some(payload))
                        } else if cache.len() < capacity {
                            let to_feed = if request.get_extra::<PinCache>().is_some() {
                                self.nodes_to_feed
                            } else {
                                self.nodes_to_feed - 1
                            };
                            let payload = self.input.pull(request).await?;
                            cache.insert(frame_number, (payload.clone(), to_feed));
                            Ok(Some(payload))
                        } else {
                            Ok(None)
                        }
                    }
                })
                .await;
            let result = result?;
            if let Some(payload) = result {
                return Ok(payload);
            }
        }
    }

    fn get_caps(&self) -> Caps { self.input.get_caps() }
}
