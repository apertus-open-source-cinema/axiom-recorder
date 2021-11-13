use crate::pipeline_processing::{payload::Payload, processing_node::ProcessingNode};
use anyhow::Result;
use std::sync::{Arc, Condvar, Mutex, RwLock};

pub fn execute_pipeline(nodes: Vec<Arc<dyn ProcessingNode>>) -> Result<()> {
    let progress =
        (0..nodes.len()).map(|_| Arc::new(ProcessingStageLock::new())).collect::<Vec<_>>();

    let result = Arc::new(RwLock::new(None));
    rayon::in_place_scope_fifo(|s| {
        for frame in 1.. {
            if result.clone().read().unwrap().is_some() {
                return;
            }
            let nodes = nodes.clone();
            let result = result.clone();
            let progress = progress.clone();
            s.spawn_fifo(move |_| {
                let mut payload = Payload::empty();
                for (node_num, node) in nodes.into_iter().enumerate() {
                    // emits a waiter for the previous frame
                    match node.process(&mut payload, progress[node_num].waiter_for(frame - 1)) {
                        Ok(Some(new_payload)) => payload = new_payload,
                        Ok(None) => {
                            *result.write().unwrap() = Some(Ok(()));
                            return;
                        }
                        Err(e) => {
                            *result.write().unwrap() = Some(Err(e));
                            return;
                        }
                    }
                    progress[node_num].process(frame);
                }
            });
        }
    });
    Arc::try_unwrap(result).unwrap().into_inner().unwrap().unwrap()
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
        ProcessingStageLockWaiter { lock: &self, frame: val }
    }
    pub fn process(&self, val: u64) {
        let mut locked = self.val.lock().unwrap();
        *locked = locked.max(val);
        self.condvar.notify_all();
    }
}
