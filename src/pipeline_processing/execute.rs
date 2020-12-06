use crate::pipeline_processing::processing_node::{Payload, ProcessingNode};
use anyhow::Result;
use std::sync::{Arc, Condvar, Mutex, RwLock};

pub fn execute_pipeline(nodes: Vec<Arc<dyn ProcessingNode>>) -> Result<()> {
    let progress =
        Arc::new((0..nodes.len()).map(|_| ProcessingStageLock::new()).collect::<Vec<_>>());

    let result = Arc::new(RwLock::new(None));
    rayon::scope_fifo(|s| {
        for frame in 0.. {
            if result.clone().read().unwrap().is_some() {
                return;
            }
            let nodes = nodes.clone();
            let result = result.clone();
            let progress = progress.clone();
            progress[0].wait_for(frame);
            s.spawn_fifo(move |_| {
                let mut payload = Payload::empty();
                for (node_num, node) in nodes.into_iter().enumerate() {
                    progress[node_num].process(frame);
                    match node.process(&mut payload) {
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
                }
            });
        }
    });
    Arc::try_unwrap(result).unwrap().into_inner().unwrap().unwrap()
}

pub struct ProcessingStageLock {
    condvar: Condvar,
    val: Mutex<u64>,
}
impl ProcessingStageLock {
    pub fn new() -> Self { ProcessingStageLock { condvar: Condvar::new(), val: Mutex::new(0) } }
    pub fn wait_for(&self, val: u64) {
        drop(self.condvar.wait_while(self.val.lock().unwrap(), |v| *v < val).unwrap())
    }
    pub fn process(&self, val: u64) {
        let mut locked = self.condvar.wait_while(self.val.lock().unwrap(), |v| *v < val).unwrap();
        *locked += 1;
        self.condvar.notify_all();
    }
}
