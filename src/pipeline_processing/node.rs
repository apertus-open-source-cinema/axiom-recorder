use crate::pipeline_processing::{payload::Payload, processing_context::ProcessingContext};
use anyhow::Result;
use async_trait::async_trait;
use std::{
    fmt::{Debug, Formatter},
    sync::Arc,
};

#[derive(Clone, Copy, Default)]
pub struct Caps {
    pub frame_count: Option<u64>,
    pub is_live: bool,
}

#[async_trait]
pub trait ProcessingNode {
    async fn pull(&self, frame_number: u64, context: &ProcessingContext) -> Result<Payload>;
    fn get_caps(&self) -> Caps;
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


pub enum Node {
    Node(Arc<dyn ProcessingNode + Send + Sync + 'static>),
    Sink(Arc<dyn SinkNode + Send + Sync + 'static>),
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
    fn into_processing_element(self) -> Node {
        Node::Node(Arc::new(self))
    }
}
pub trait SinkNodeIntoNode {
    fn into_processing_element(self) -> Node;
}
impl<T: SinkNode + Send + Sync + 'static> SinkNodeIntoNode for T {
    fn into_processing_element(self) -> Node {
        Node::Sink(Arc::new(self))
    }
}
