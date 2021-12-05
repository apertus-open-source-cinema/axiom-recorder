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
    async fn pull(&self, frame_number: u64, context: ProcessingContext) -> Result<Payload>;
    fn get_caps(&self) -> Caps;
}

#[async_trait]
pub trait ProcessingSink {
    async fn run(&self, context: ProcessingContext) -> Result<()>;
}

pub enum ProcessingElement {
    Node(Arc<dyn ProcessingNode + Send + Sync + 'static>),
    Sink(Arc<dyn ProcessingSink + Send + Sync + 'static>),
}
impl Debug for ProcessingElement {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ProcessingElement::Node(_) => f.write_str("ProcessingElement::Node"),
            ProcessingElement::Sink(_) => f.write_str("ProcessingElement::Sink"),
        }
    }
}
pub trait ProcessingNodeIntoProcessingElement {
    fn into_processing_element(self) -> ProcessingElement;
}
impl<T: ProcessingNode + Send + Sync + 'static> ProcessingNodeIntoProcessingElement for T {
    fn into_processing_element(self) -> ProcessingElement {
        ProcessingElement::Node(Arc::new(self))
    }
}
pub trait ProcessingSinkIntoProcessingElement {
    fn into_processing_element(self) -> ProcessingElement;
}
impl<T: ProcessingSink + Send + Sync + 'static> ProcessingSinkIntoProcessingElement for T {
    fn into_processing_element(self) -> ProcessingElement {
        ProcessingElement::Sink(Arc::new(self))
    }
}
