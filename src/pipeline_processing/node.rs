use std::fmt::Debug;
use crate::pipeline_processing::payload::Payload;
use async_trait::async_trait;
use anyhow::Result;

#[derive(Clone, Copy, Default)]
pub struct Caps {
    pub frame_count: Option<u64>,
    pub is_live: bool,
}

#[async_trait]
pub trait ProcessingNode {
    async fn pull(&self, frame_number: u64) -> Result<Payload>;
    fn get_caps(&self) -> Caps;
}


#[async_trait]
pub trait ProcessingSink {
    async fn run(&self);
}