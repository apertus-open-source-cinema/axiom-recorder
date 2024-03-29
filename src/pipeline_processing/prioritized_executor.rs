use async_task::Runnable;
use std::{
    cmp::Reverse,
    collections::BinaryHeap,
    future::Future,
    sync::{Arc, Condvar, Mutex},
    thread,
};


#[derive(derivative::Derivative)]
#[derivative(PartialEq, Eq, PartialOrd, Ord)]
struct PrioritizedRunnable<T: Ord> {
    key: T,
    #[derivative(PartialEq = "ignore", PartialOrd = "ignore", Ord = "ignore")]
    runnable: Runnable,
}

type CVar<T> = Arc<(Mutex<T>, Condvar)>;

#[derive(Clone)]
pub struct PrioritizedReactor<T: Ord> {
    queue_cvar: CVar<BinaryHeap<Reverse<PrioritizedRunnable<T>>>>,
    pub num_threads: usize,
}

impl<P: Ord + Clone + Send + Sync + 'static> PrioritizedReactor<P> {
    pub(self) fn new(num_threads: usize) -> Self {
        Self { queue_cvar: Default::default(), num_threads }
    }

    pub(self) fn start_inner(&self) {
        for _ in 0..self.num_threads {
            let queue_cvar = self.queue_cvar.clone();
            thread::spawn(move || {
                let (queue, cvar) = &*queue_cvar;
                loop {
                    let task = {
                        let mut queue = queue.lock().unwrap();
                        loop {
                            match queue.pop() {
                                Some(v) => break v,
                                None => queue = cvar.wait(queue).unwrap(),
                            }
                        }
                    };
                    task.0.runnable.run();
                }
            });
        }
    }

    pub fn start(num_threads: usize) -> Self {
        let instance = Self::new(num_threads);
        instance.start_inner();
        instance
    }

    pub fn spawn_with_priority<O: Send + 'static>(
        &self,
        fut: impl Future<Output = O> + Send + 'static,
        priority: P,
    ) -> impl Future<Output = O> {
        let queue_cvar = self.queue_cvar.clone();
        let (runnable, task) = async_task::spawn(fut, move |runnable| {
            let (queue, cvar) = &*queue_cvar;
            queue
                .lock()
                .unwrap()
                .push(Reverse(PrioritizedRunnable { key: priority.clone(), runnable }));
            cvar.notify_one();
        });
        runnable.schedule();
        task
    }
}

#[cfg(test)]
mod prioritized_future_test {
    use super::*;
    use futures::join;
    use std::{
        pin::Pin,
        sync::atomic::{AtomicU64, Ordering},
        task::{Context, Poll},
    };

    #[test]
    fn test_smoke() {
        pollster::block_on(async {
            for _ in 0..100 {
                let pr = PrioritizedReactor::new(1);

                let output: Arc<Mutex<Vec<u64>>> = Default::default();

                let fut_3 = {
                    let output = output.clone();
                    pr.spawn_with_priority(async move { output.lock().unwrap().push(3) }, 3)
                };
                let fut_1 = {
                    let output = output.clone();
                    pr.spawn_with_priority(async move { output.lock().unwrap().push(1) }, 1)
                };
                let fut_2 = {
                    let output = output.clone();
                    pr.spawn_with_priority(async move { output.lock().unwrap().push(2) }, 2)
                };

                pr.start_inner();
                let _res = join!(fut_3, fut_1, fut_2);
                assert_eq!(&*output.lock().unwrap(), &vec![1, 2, 3]);
            }
        })
    }

    #[test]
    fn test_step_future() {
        pollster::block_on(async {
            struct StepFuture {
                current: AtomicU64,
            }
            impl Future for StepFuture {
                type Output = ();

                fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
                    match self.current.load(Ordering::Acquire) {
                        1_000 => Poll::Ready(()),
                        _ => {
                            self.current.fetch_add(1, Ordering::Release);
                            cx.waker().clone().wake();
                            Poll::Pending
                        }
                    }
                }
            }

            let pr = PrioritizedReactor::new(1);
            pr.start_inner();
            pr.spawn_with_priority(StepFuture { current: Default::default() }, 1).await;
        })
    }
}
