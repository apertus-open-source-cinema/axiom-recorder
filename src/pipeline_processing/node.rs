use crate::pipeline_processing::{payload::Payload, processing_context::ProcessingContext};
use anyhow::Result;
use async_trait::async_trait;
use std::{
    fmt::{Debug, Formatter},
    sync::Arc,
};

#[derive(thiserror::Error, Debug)]
#[error("end of file")]
pub struct EOFError;

#[derive(Clone, Copy, Default)]
pub struct Caps {
    pub frame_count: Option<u64>,
    pub is_live: bool,
}

#[async_trait]
pub trait ProcessingNode {
    async fn pull(
        &self,
        frame_number: u64,
        requester: NodeID,
        context: &ProcessingContext,
    ) -> Result<Payload>;
    fn get_caps(&self) -> Caps;
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct NodeID(u16);

impl From<NodeID> for usize {
    fn from(value: NodeID) -> Self { value.0 as _ }
}

impl From<usize> for NodeID {
    fn from(value: usize) -> Self { NodeID(value as _) }
}

pub struct InputProcessingNode {
    node: Arc<dyn ProcessingNode + Send + Sync>,
    puller_id: NodeID,
}

impl InputProcessingNode {
    pub(crate) fn new(puller_id: NodeID, node: Arc<dyn ProcessingNode + Send + Sync>) -> Self {
        Self { node, puller_id }
    }

    pub async fn pull(&self, frame_number: u64, context: &ProcessingContext) -> Result<Payload> {
        self.node.pull(frame_number, self.puller_id, context).await
    }

    fn copy_with(&self, puller_id: NodeID) -> Self { Self { node: self.node.clone(), puller_id } }

    pub(crate) fn clone_for_same_puller(&self) -> Self { self.copy_with(self.puller_id) }

    pub fn get_caps(&self) -> Caps { self.node.get_caps() }
}

#[async_trait]
pub trait SinkNode {
    async fn run(
        &self,
        context: &ProcessingContext,
        progress_callback: Arc<dyn Fn(ProgressUpdate) + Send + Sync>,
    ) -> Result<()>;
}

#[derive(Copy, Clone, Debug)]
pub struct ProgressUpdate {
    pub latest_frame: u64,
    pub total_frames: Option<u64>,
}


#[derive(Clone)]
pub enum Node {
    Node(Arc<dyn ProcessingNode + Send + Sync + 'static>),
    Sink(Arc<dyn SinkNode + Send + Sync + 'static>),
}
impl Node {
    pub fn is_sink(&self) -> bool { matches!(self, Node::Sink(_)) }
    pub fn assert_input_node(&self) -> Result<Arc<dyn ProcessingNode + Send + Sync + 'static>> {
        match self {
            Self::Node(node) => Ok(node.clone()),
            Self::Sink(_sink) => Err(anyhow::anyhow!("wanted to get a input node, was a sink")),
        }
    }

    pub fn assert_sink(&self) -> Result<Arc<dyn SinkNode + Send + Sync + 'static>> {
        match self {
            Self::Sink(sink) => Ok(sink.clone()),
            Self::Node(_node) => {
                Err(anyhow::anyhow!("wanted to get a sink node, was a normal node"))
            }
        }
    }
}
impl Debug for Node {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Node::Node(_) => f.write_str("ProcessingElement::Node"),
            Node::Sink(_) => f.write_str("ProcessingElement::Sink"),
        }
    }
}
pub trait ProcessingNodeIntoNode {
    fn into_processing_element(self) -> Node;
}
impl<T: ProcessingNode + Send + Sync + 'static> ProcessingNodeIntoNode for T {
    fn into_processing_element(self) -> Node { Node::Node(Arc::new(self)) }
}
pub trait SinkNodeIntoNode {
    fn into_processing_element(self) -> Node;
}
impl<T: SinkNode + Send + Sync + 'static> SinkNodeIntoNode for T {
    fn into_processing_element(self) -> Node { Node::Sink(Arc::new(self)) }
}
