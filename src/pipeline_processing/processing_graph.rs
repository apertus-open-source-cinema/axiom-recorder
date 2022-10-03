use anyhow::Result;
use futures::StreamExt;
use serde::{Deserialize, Deserializer};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use crate::{
    nodes::create_node_from_name,
    pipeline_processing::{
        node::{Node, NodeID, ProgressUpdate},
        parametrizable::{ParameterValue, Parameters},
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


#[derive(Debug, Clone)]
pub enum SerdeNodeParam {
    Int(i64),
    Float(f64),
    Bool(bool),
    String(String),
    NodeInput(String),
    List(Vec<SerdeNodeParam>),
}

impl<'de> Deserialize<'de> for SerdeNodeParam {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize, Debug)]
        #[serde(untagged)]
        enum NodeParamSimple {
            Int(i64),
            Float(f64),
            Bool(bool),
            String(String),
            List(Vec<SerdeNodeParam>),
        }

        let param = NodeParamSimple::deserialize(deserializer)?;
        Ok(match param {
            NodeParamSimple::Float(f) => SerdeNodeParam::Float(f),
            NodeParamSimple::Int(i) => SerdeNodeParam::Int(i),
            NodeParamSimple::String(s) => {
                if let Some(s) = s.strip_prefix('<') {
                    SerdeNodeParam::NodeInput(s.to_owned())
                } else {
                    SerdeNodeParam::String(s)
                }
            }
            NodeParamSimple::Bool(b) => SerdeNodeParam::Bool(b),
            NodeParamSimple::List(l) => SerdeNodeParam::List(l),
        })
    }
}

impl TryFrom<SerdeNodeParam> for ParameterValue {
    type Error = ();

    fn try_from(value: SerdeNodeParam) -> Result<Self, Self::Error> {
        match value {
            SerdeNodeParam::Float(f) => Ok(ParameterValue::FloatRangeValue(f)),
            SerdeNodeParam::Int(i) => Ok(ParameterValue::IntRangeValue(i)),
            SerdeNodeParam::String(s) => Ok(ParameterValue::StringValue(s)),
            SerdeNodeParam::Bool(b) => Ok(ParameterValue::BoolValue(b)),
            SerdeNodeParam::NodeInput(_) => Err(()),
            SerdeNodeParam::List(l) => Ok(ParameterValue::ListValue(
                l.into_iter().map(ParameterValue::try_from).collect::<Result<_, Self::Error>>()?,
            )),
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct SerdeNodeConfig {
    #[serde(rename = "type")]
    ty: String,
    #[serde(flatten)]
    parameters: HashMap<String, SerdeNodeParam>,
}

impl From<SerdeNodeConfig> for ProcessingNodeConfig<String> {
    fn from(node_config: SerdeNodeConfig) -> Self {
        Self {
            name: node_config.ty,
            parameters: Parameters::new(
                node_config
                    .parameters
                    .clone()
                    .into_iter()
                    .filter_map(|(name, param)| match param {
                        SerdeNodeParam::NodeInput(_) => None,
                        param => param.try_into().ok().map(|v| (name, v)),
                    })
                    .collect(),
            ),
            inputs: node_config
                .parameters
                .into_iter()
                .filter_map(|(name, param)| match param {
                    SerdeNodeParam::NodeInput(i) => Some((name, i)),
                    _ => None,
                })
                .collect(),
        }
    }
}


pub struct ProcessingGraphBuilder<IdTy> {
    nodes: HashMap<IdTy, ProcessingNodeConfig<IdTy>>,
    node_ids: HashMap<IdTy, NodeID>,
}

impl<IdTy> Default for ProcessingGraphBuilder<IdTy> {
    fn default() -> Self { Self { nodes: HashMap::new(), node_ids: HashMap::new() } }
}

impl<IdTy> ProcessingGraphBuilder<IdTy>
where
    IdTy: Eq + std::hash::Hash + std::fmt::Debug + Clone,
{
    pub fn new() -> Self { Default::default() }

    pub fn add(&mut self, name: IdTy, config: ProcessingNodeConfig<IdTy>) -> Result<NodeID> {
        let node_id: NodeID = self.nodes.len().into();
        match self.nodes.entry(name.clone()) {
            std::collections::hash_map::Entry::Occupied(e) => {
                let e = e.get();
                Err(anyhow::anyhow!(
                    "tried to add node {config:?} with name {name:?}, but it already exists: {e:?}"
                ))
            }
            std::collections::hash_map::Entry::Vacant(v) => {
                v.insert(config);
                self.node_ids.insert(name, node_id);
                Ok(node_id)
            }
        }
    }

    pub fn build(mut self, ctx: &ProcessingContext) -> Result<ProcessingGraph> {
        if self.nodes.is_empty() {
            Ok(ProcessingGraph { nodes: HashMap::new(), sinks: vec![] })
        } else {
            let mut is_input_to = HashMap::<_, Vec<_>>::new();

            for (id, node) in self.nodes.iter() {
                let idx = self.node_ids[id];
                for input in node.inputs.values() {
                    is_input_to.entry(self.node_ids[input]).or_default().push(idx);
                }
            }

            let mut built_nodes = HashMap::<NodeID, Node>::new();
            let mut sinks = vec![];

            let mut avail: HashSet<IdTy> = self.node_ids.keys().cloned().collect();
            let mut queue = vec![];
            queue.push({
                let v = avail.iter().next().cloned().unwrap();
                avail.take(&v).unwrap()
            });

            while !queue.is_empty() {
                let id = queue.last().unwrap().clone();
                let idx = self.node_ids[&id];
                let mut missing = vec![];
                let mut finished = HashMap::new();

                let node = self.nodes.remove(&id).unwrap();

                for (name, input_id) in &node.inputs {
                    if let Some(node) = built_nodes.get(&self.node_ids[input_id]) {
                        finished.insert(name.clone(), node.clone());
                    } else {
                        missing.push(input_id.clone())
                    }
                }

                if !missing.is_empty() {
                    queue.append(&mut missing);
                    self.nodes.insert(id, node);
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

            Ok(ProcessingGraph { nodes: built_nodes, sinks })
        }
    }
}

pub struct ProcessingGraph {
    nodes: HashMap<NodeID, Node>,
    sinks: Vec<NodeID>,
}

impl ProcessingGraph {
    pub fn run<FUNC: Fn(ProgressUpdate) + Send + Sync + Clone + 'static>(
        &self,
        ctx: ProcessingContext,
        progress_update_cb: FUNC,
    ) -> Result<()> {
        let ctx = Arc::new(ctx);
        if self.sinks.is_empty() {
            Err(anyhow::anyhow!("processing graph should contain at least one sink"))
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

            Ok(())
        }
    }

    pub fn get_node(&self, id: NodeID) -> &Node { &self.nodes[&id] }
}
