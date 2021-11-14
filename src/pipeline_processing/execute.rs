use crate::pipeline_processing::{payload::Payload, processing_node::ProcessingNode};
use anyhow::Result;
use itertools::Itertools;
use rayon::prelude::*;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
    Condvar,
    Mutex,
};

pub fn execute_pipeline(nodes: Vec<Arc<dyn ProcessingNode>>) -> Result<()> {
    let nodes = Arc::new(nodes);
    let progress =
        Arc::new((0..nodes.len()).map(|_| ProcessingStageLock::new()).collect::<Vec<_>>());
    let frame = AtomicU64::new(1);

    let result = rayon::iter::repeat(0)
        .into_par_iter()
        .map(|_| {
            let frame = frame.fetch_add(1, Ordering::SeqCst);
            let mut payload = Payload::empty();
            for (node_num, node) in nodes.iter().enumerate() {
                // emits a waiter for the previous frame

                match node.process(&mut payload, progress[node_num].waiter_for(frame - 1)) {
                    Ok(Some(new_payload)) => payload = new_payload,
                    Ok(None) => {
                        return Some(Ok(()));
                    }
                    Err(e) => {
                        eprintln!(
                            "An error occured: \n{}",
                            e.chain().map(|e| format!("{}", e)).join("\n")
                        );
                        return Some(Err(e));
                    }
                }
                progress[node_num].process(frame);
            }

            None
        })
        .find_any(|result| result.is_some())
        .unwrap();
    result.unwrap()
}

pub struct ProcessingStageLock {
    condvar: Condvar,
    // hold the frame currently done
    val: Mutex<u64>,
}

pub struct ProcessingStageLockWaiter<'a> {
    lock: &'a ProcessingStageLock,
    frame: u64,
}

impl<'a> ProcessingStageLockWaiter<'a> {
    pub fn frame(&self) -> u64 { self.frame + 1 }

    pub fn wait(&self) {
        drop(
            self.lock
                .condvar
                .wait_while(self.lock.val.lock().unwrap(), |v| *v < self.frame)
                .unwrap(),
        )
    }
}

impl ProcessingStageLock {
    pub fn new() -> Self { ProcessingStageLock { condvar: Condvar::new(), val: Mutex::new(0) } }
    pub fn waiter_for<'a>(&'a self, val: u64) -> ProcessingStageLockWaiter<'a> {
        ProcessingStageLockWaiter { lock: self, frame: val }
    }
    pub fn process(&self, val: u64) {
        let mut locked = self.val.lock().unwrap();
        *locked = locked.max(val);
        self.condvar.notify_all();
    }
}
