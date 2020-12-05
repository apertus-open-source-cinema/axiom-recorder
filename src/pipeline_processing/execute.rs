use crate::pipeline_processing::processing_node::{Payload, ProcessingNode};
use anyhow::Result;
use std::sync::Arc;
use std::sync::atomic::{Ordering, AtomicBool};
use std::sync::mpsc::channel;

pub fn execute_pipeline(nodes: Vec<Arc<dyn ProcessingNode>>) -> Result<()> {
    rayon::scope_fifo(|s| {
        let (tx, rx) = channel();

        let (source, rest) = nodes.split_first().unwrap();
        s.spawn_fifo(move |_| {
            while let Some(payload) = source.process(&mut Payload::empty()).unwrap() {
                tx.send(payload).unwrap()
            }
        });


        for payload in rx {
            let mut payload = payload.clone();
            let nodes = rest.clone();
            s.spawn_fifo(move |_| {
                for node in nodes {
                    match node.process(&mut payload).unwrap() {
                        Some(new_payload) => payload = new_payload,
                        None => { break },
                    }
                }
            });
        }
    });
    Ok(())
}
