use crate::pipeline_processing::{
    payload::Payload,
    processing_context::{Priority, ProcessingContext},
};
use anyhow::Result;
use anymap::CloneAny;
use async_trait::async_trait;

use std::{
    fmt::{Debug, Formatter},
    sync::Arc,
};

#[derive(thiserror::Error, Debug)]
#[error("end of file")]
pub struct EOFError;

#[derive(Clone, Copy, Default, Debug)]
pub struct Caps {
    pub frame_count: Option<u64>,

    // iff this is true, it is allowed to access frames in a random order (with gaps; in reverse
    // direction; ...). otherwise every frame has to be pulled (or explicitly dropped) in
    // ascending order.
    pub random_access: bool,
}

#[derive(Clone, Debug)]
pub struct Request {
    frame_number: u64,
    priority: Priority,
    requester: NodeID,
    extra: anymap::Map<dyn CloneAny + Send + Sync>,
}

impl Request {
    // you shall only use this function if you are a sink. otherwise you shal derive
    // your requests from your input request!
    pub fn new(output_priority: u8, frame_number: u64) -> Self {
        Self {
            priority: Priority::new(output_priority, frame_number),
            frame_number,
            requester: NodeID::from(usize::MAX),
            extra: anymap::Map::new(),
        }
    }
    fn with_requester(&self, requester: NodeID) -> Self { Self { requester, ..self.clone() } }
    pub fn with_frame_number(&self, frame_number: u64) -> Self {
        Self { frame_number, ..self.clone() }
    }
    pub fn frame_number(&self) -> u64 { self.frame_number }
    pub fn priority(&self) -> Priority { self.priority }
    pub fn get_extra<T>(&self) -> Option<&T>
    where
        T: Clone + Send + Sync + 'static,
    {
        self.extra.get::<T>()
    }
}

// types that are common to end up in the extra AnyMap of Request:

/// Indicates that the frame request does not really need to be fulfilled but it
/// is okay to omit all the work and instead return an error. A processingBlock
/// _must_ pull all its inputs as it would usually do and indecate the FrameDrop
/// to them.
#[derive(Copy, Clone, Debug)]
pub struct Drop;

/// Indicates that we would like to be able to re-request the requested frame
/// (so dont evict it)
#[derive(Copy, Clone, Debug)]
pub struct PinCache;


#[async_trait]
pub trait ProcessingNode {
    async fn pull(&self, request: Request) -> Result<Payload>;
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

// This struct is passed as a wrapper to other ProcessingNodes as input so that
// they dont need to supply their NodeID
pub struct InputProcessingNode {
    node: Arc<dyn ProcessingNode + Send + Sync>,
    node_id: NodeID,
}

impl InputProcessingNode {
    pub(crate) fn new(puller_id: NodeID, node: Arc<dyn ProcessingNode + Send + Sync>) -> Self {
        Self { node, node_id: puller_id }
    }

    pub async fn pull(&self, request: Request) -> Result<Payload> {
        self.node.pull(request.with_requester(self.node_id)).await
    }

    fn copy_with(&self, node_id: NodeID) -> Self { Self { node: self.node.clone(), node_id } }

    pub(crate) fn clone_for_same_puller(&self) -> Self { self.copy_with(self.node_id) }

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
