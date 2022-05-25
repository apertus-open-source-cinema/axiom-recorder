// TODO(robin): explicit cache + drop signaling
// signaling of sinks that are done before others
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    sync::Arc,
};

use crate::pipeline_processing::{
    node::InputProcessingNode,
    parametrizable::{Parameterizable, Parameters, ParametersDescriptor},
    payload::Payload,
};
use anyhow::Result;
use futures::{
    future::{BoxFuture, Shared},
    lock::Mutex,
    FutureExt,
};


use crate::pipeline_processing::{
    node::{Caps, NodeID, ProcessingNode},
    parametrizable::{ParameterType, ParameterTypeDescriptor},
    processing_context::ProcessingContext,
};
use async_trait::async_trait;


#[derive(Default)]
struct ConsecutiveTracker {
    values: BTreeSet<u64>,
    highest_consecutive: Option<u64>,
}

impl ConsecutiveTracker {
    fn push(&mut self, value: u64) -> u64 {
        if let Some(highest_consecutive) = self.highest_consecutive {
            if value < highest_consecutive {
                eprintln!(
                    "went backwards, inserted {value}, highest_consecutive: {highest_consecutive}"
                );
                return highest_consecutive;
            }
        }
        self.values.insert(value);
        let mut lowest_curr = self.values.iter().cloned().next().unwrap();
        lowest_curr = match self.highest_consecutive {
            Some(v) => {
                if lowest_curr == v + 1 {
                    lowest_curr
                } else {
                    return v;
                }
            }
            None => {
                if lowest_curr == 0 {
                    lowest_curr
                } else {
                    return 0;
                }
            }
        };

        self.values.remove(&lowest_curr);
        while let Some(next) = self.values.iter().cloned().next() {
            if next == lowest_curr + 1 {
                self.values.remove(&next);
                lowest_curr = next;
            } else {
                break;
            }
        }

        self.highest_consecutive = Some(lowest_curr);
        lowest_curr
    }
}

// cache eviction policy:
// after 0 through n were pulled by a specific puller, remove 0 through n - 1
// this is of course bullshit, because, one might need the "last" frame multiple
// times, but because everything is out of order this eviction policy does not
// guarantee that availability


type PayloadFuture = Shared<BoxFuture<'static, std::result::Result<Payload, Arc<anyhow::Error>>>>;
type PayloadCache = HashMap<NodeID, (ConsecutiveTracker, BTreeMap<u64, PayloadFuture>)>;

pub struct Cache {
    input: InputProcessingNode,
    cache: Arc<Mutex<PayloadCache>>,
}

impl Parameterizable for Cache {
    fn describe_parameters() -> ParametersDescriptor {
        ParametersDescriptor::new()
            .with("input", ParameterTypeDescriptor::Mandatory(ParameterType::NodeInput))
    }

    fn from_parameters(
        mut parameters: Parameters,
        is_input_to: &[NodeID],
        _context: &ProcessingContext,
    ) -> Result<Self> {
        let mut cache = HashMap::new();
        for id in is_input_to {
            cache.insert(*id, Default::default());
        }
        Ok(Self { input: parameters.get("input")?, cache: Arc::new(Mutex::new(cache)) })
    }
}

#[async_trait]
impl ProcessingNode for Cache {
    async fn pull(
        &self,
        frame_number: u64,
        puller_id: NodeID,
        context: &ProcessingContext,
    ) -> Result<Payload> {
        let fut = {
            let mut cache = self.cache.lock().await;
            let (fut, insert) = {
                let (tracker, futs) = cache.get_mut(&puller_id).unwrap();
                let (fut, insert) = if let Some(fut) = futs.get(&frame_number) {
                    (fut.clone(), false)
                } else {
                    let input = self.input.clone_for_same_puller();
                    let context = context.clone();
                    let fut = async move {
                        let payload = input.pull(frame_number, &context).await;
                        payload.map_err(Arc::new)
                    }
                    .boxed()
                    .shared();
                    (fut, true)
                };

                let highest_consecutive = tracker.push(frame_number);
                if (frame_number >= highest_consecutive) && insert {
                    futs.insert(frame_number, fut.clone());
                }
                while let Some(v) = futs.keys().cloned().next() {
                    if v < highest_consecutive {
                        // eprintln!("removing cache frame {v} for puller {puller_id:?}");
                        futs.remove(&v);
                    } else {
                        break;
                    }
                }

                (fut, insert)
            };

            if insert {
                for (id, (_, futs)) in cache.iter_mut() {
                    if *id != puller_id {
                        // eprintln!("inserting {frame_number} for {id:?}");
                        futs.insert(frame_number, fut.clone());
                    }
                }
            }

            fut
        };

        fut.await.map_err(|e| anyhow::anyhow!("error from cache: {e}"))
    }

    fn get_caps(&self) -> Caps { self.input.get_caps() }
}
