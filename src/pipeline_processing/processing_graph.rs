use anyhow::Result;
use std::collections::{HashMap, HashSet};

use crate::{
    nodes::create_node_from_name,
    pipeline_processing::{
        node::{Node, NodeID, ProgressUpdate},
        parametrizable::Parameters,
        processing_context::ProcessingContext,
    },
};

pub struct ProcessingNodeConfig {
    name: String,
    parameters: Parameters,
    inputs: HashMap<String, usize>,
}
impl ProcessingNodeConfig {
    pub fn single_input_node(name: String, parameters: Parameters, input: Option<NodeID>) -> Self {
        ProcessingNodeConfig {
            inputs: ["input".to_owned()].into_iter().zip(input.map(|v| v.into())).collect(),
            parameters,
            name,
        }
    }
}

#[derive(Default)]
pub struct ProcessingGraph {
    nodes: Vec<Option<ProcessingNodeConfig>>,
}

impl ProcessingGraph {
    pub fn new() -> Self { Default::default() }

    pub fn add(&mut self, config: ProcessingNodeConfig) -> NodeID {
        let idx = self.nodes.len();
        self.nodes.push(Some(config));
        idx.into()
    }

    pub fn build(mut self, ctx: &ProcessingContext) -> Result<BuiltProcessingGraph> {
        if self.nodes.is_empty() {
            Ok(BuiltProcessingGraph { nodes: HashMap::new(), sinks: vec![] })
        } else {
            let mut is_input_to = HashMap::<_, Vec<_>>::new();

            for (idx, node) in self.nodes.iter().enumerate() {
                for input in node.as_ref().unwrap().inputs.values() {
                    is_input_to.entry(*input).or_default().push(idx.into());
                }
            }


            let mut built_nodes = HashMap::<_, Node>::new();
            let mut avail: HashSet<_> = (1..self.nodes.len()).collect();
            let mut queue = vec![0];
            let mut sinks = vec![];

            while !queue.is_empty() {
                let idx = *queue.last().unwrap();
                let mut missing = vec![];
                let mut finished = HashMap::new();

                let node = std::mem::take(&mut self.nodes[idx]).unwrap();

                for (name, input_id) in &node.inputs {
                    if let Some(node) = built_nodes.get(&(*input_id).into()) {
                        finished.insert(name.clone(), node.clone());
                    } else {
                        missing.push(*input_id)
                    }
                }

                if !missing.is_empty() {
                    queue.append(&mut missing);
                    self.nodes[idx] = Some(node);
                } else {
                    let built_node = create_node_from_name(
                        &node.name,
                        idx.into(),
                        node.parameters,
                        finished,
                        is_input_to.entry(idx).or_default(),
                        ctx,
                    )?;
                    if built_node.is_sink() {
                        sinks.push(idx.into());
                    }
                    built_nodes.insert(idx.into(), built_node);


                    avail.remove(&idx);
                    queue.pop().unwrap();

                    if queue.is_empty() && !avail.is_empty() {
                        queue.push(*avail.iter().next().unwrap())
                    }
                }
            }

            Ok(BuiltProcessingGraph { nodes: built_nodes, sinks })
        }
    }
}

pub struct BuiltProcessingGraph {
    nodes: HashMap<NodeID, Node>,
    sinks: Vec<NodeID>,
}

impl BuiltProcessingGraph {
    pub fn run<FUNC: Fn(ProgressUpdate) + Send + Sync + Clone + 'static>(
        &self,
        ctx: &ProcessingContext,
        progress_update_cb: FUNC,
    ) -> Result<()> {
        if self.sinks.is_empty() {
            Err(anyhow::anyhow!("processing graph should contain atleast one sink"))
        } else {
            pollster::block_on(futures::future::try_join_all(self.sinks.iter().map(|id| async {
                let sink = self.nodes.get(id).unwrap().assert_sink()?;
                anyhow::Result::Ok(
                    sink.run(ctx, std::sync::Arc::new(progress_update_cb.clone())).await?,
                )
            })))
            .map(|_v| ())
        }
    }
}
