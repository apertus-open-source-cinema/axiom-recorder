use crate::pipeline_processing::processing_node::{Payload, ProcessingNode};
use anyhow::Result;

pub fn execute_pipeline(nodes: Vec<Box<dyn ProcessingNode>>) -> Result<()> {
    loop {
        let mut payload = Payload::empty();
        for node in &nodes {
            match node.process(&mut payload)? {
                Some(new_payload) => payload = new_payload,
                None => return Ok(()),
            }
        }
    }
}
