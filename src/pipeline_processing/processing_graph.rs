use anyhow::Result;
use futures::StreamExt;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use crate::{
    nodes::create_node_from_name,
    pipeline_processing::{
        node::{Node, NodeID, ProgressUpdate},
        parametrizable::Parameters,
        processing_context::ProcessingContext,
    },
};

#[derive(Debug)]
pub struct ProcessingNodeConfig<IdTy> {
    pub name: String,
    pub parameters: Parameters,
    pub inputs: HashMap<String, IdTy>,
}
impl<IdTy> ProcessingNodeConfig<IdTy> {
    pub fn single_input_node<T: Into<IdTy>>(
        name: String,
        parameters: Parameters,
        input: Option<T>,
    ) -> Self {
        ProcessingNodeConfig {
            inputs: ["input".to_owned()].into_iter().zip(input.map(|v| v.into())).collect(),
            parameters,
            name,
        }
    }
}

pub struct ProcessingGraph<IdTy> {
    nodes: HashMap<IdTy, Option<ProcessingNodeConfig<IdTy>>>,
}

impl<IdTy> Default for ProcessingGraph<IdTy> {
    fn default() -> Self { Self { nodes: HashMap::new() } }
}

impl<IdTy> ProcessingGraph<IdTy>
where
    IdTy: std::cmp::Eq + std::hash::Hash + std::fmt::Debug + Clone,
{
    pub fn new() -> Self { Default::default() }

    pub fn add(&mut self, name: IdTy, config: ProcessingNodeConfig<IdTy>) -> Result<NodeID> {
        let idx = self.nodes.len();
        match self.nodes.entry(name.clone()) {
            std::collections::hash_map::Entry::Occupied(e) => {
                let e = e.get();
                Err(anyhow::anyhow!(
                    "tried to add node {config:?} with name {name:?}, but it already exists: {e:?}"
                ))
            }
            std::collections::hash_map::Entry::Vacant(v) => {
                v.insert(Some(config));
                Ok(idx.into())
            }
        }
    }

    pub fn build(mut self, ctx: &ProcessingContext) -> Result<BuiltProcessingGraph> {
        if self.nodes.is_empty() {
            Ok(BuiltProcessingGraph { nodes: HashMap::new(), sinks: vec![] })
        } else {
            let mut id_to_nodeid = HashMap::<IdTy, NodeID>::new();
            let mut is_input_to = HashMap::<_, Vec<_>>::new();

            for (idx, id) in self.nodes.keys().enumerate() {
                id_to_nodeid.insert(id.clone(), idx.into());
            }

            for (id, node) in self.nodes.iter() {
                let idx = id_to_nodeid[id];
                for input in node.as_ref().unwrap().inputs.values() {
                    is_input_to.entry(id_to_nodeid[input]).or_default().push(idx);
                }
            }


            let mut built_nodes = HashMap::<NodeID, Node>::new();
            let mut avail: HashSet<IdTy> = id_to_nodeid.keys().cloned().collect();
            let mut queue = vec![];
            queue.push({
                let v = avail.iter().cloned().next().unwrap();
                avail.take(&v).unwrap()
            });
            let mut sinks = vec![];

            while !queue.is_empty() {
                let id = queue.last().unwrap().clone();
                let idx = id_to_nodeid[&id];
                let mut missing = vec![];
                let mut finished = HashMap::new();

                let node = self.nodes.remove(&id).unwrap().unwrap();

                for (name, input_id) in &node.inputs {
                    if let Some(node) = built_nodes.get(&id_to_nodeid[input_id]) {
                        finished.insert(name.clone(), node.clone());
                    } else {
                        missing.push(input_id.clone())
                    }
                }

                if !missing.is_empty() {
                    queue.append(&mut missing);
                    self.nodes.insert(id, Some(node));
                } else {
                    let built_node = create_node_from_name(
                        &node.name,
                        idx,
                        node.parameters,
                        finished,
                        is_input_to.entry(idx).or_default(),
                        ctx,
                    )?;
                    if built_node.is_sink() {
                        sinks.push(idx);
                    }
                    built_nodes.insert(idx, built_node);


                    avail.remove(&id);
                    queue.pop().unwrap();

                    if queue.is_empty() && !avail.is_empty() {
                        queue.push(avail.iter().next().unwrap().clone())
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
        ctx: ProcessingContext,
        progress_update_cb: FUNC,
    ) -> Result<()> {
        let ctx = Arc::new(ctx);
        if self.sinks.is_empty() {
            Err(anyhow::anyhow!("processing graph should contain atleast one sink"))
        } else {
            let res = ctx.block_on(async {
                anyhow::Result::<_, anyhow::Error>::Ok(
                    self.sinks
                        .iter()
                        .cloned()
                        .map(|id| {
                            let progress_update_cb = progress_update_cb.clone();
                            let ctx = ctx.clone();
                            let sink = self.nodes.get(&id).unwrap().assert_sink()?;
                            Ok(async move {
                                anyhow::Result::<_, anyhow::Error>::Ok(
                                    sink.run(&*ctx, std::sync::Arc::new(progress_update_cb))
                                        .await?,
                                )
                            })
                        })
                        .collect::<Result<futures::stream::FuturesUnordered<_>>>()?
                        .collect::<Vec<_>>()
                        .await,
                )
            })?;

            for r in res {
                r?
            }


            anyhow::Result::Ok(())
        }
    }
}
