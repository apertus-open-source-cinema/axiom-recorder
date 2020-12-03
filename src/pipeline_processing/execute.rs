use crate::pipeline_processing::processing_node::{Payload, ProcessingNode};
use anyhow::Result;
use std::sync::Arc;

pub fn execute_pipeline(nodes: Vec<Arc<dyn ProcessingNode>>) -> Result<()> {
    rayon::scope(|s| {
        s.spawn(move |_| {
            let nodes = &nodes.clone();
            loop {
                let mut payload = Payload::empty();
                for node in nodes {
                    match node.process(&mut payload).unwrap() {
                        Some(new_payload) => payload = new_payload,
                        None => return,
                    }
                }
            }
        });
    });
    Ok(())
}
