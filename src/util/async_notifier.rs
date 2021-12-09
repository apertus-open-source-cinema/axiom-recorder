
use std::{
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
        Mutex,
    },
    task::{Context, Poll, Waker},
};

pub struct AsyncNotifierInner<T> {
    data: T,
    futures: Vec<(Arc<AsyncNotifierFuture>, Box<dyn Fn(&T) -> bool + Send + Sync>)>,
}
#[derive(Clone)]
pub struct AsyncNotifier<T>(Arc<Mutex<AsyncNotifierInner<T>>>);
impl<T> AsyncNotifier<T> {
    pub fn new(data: T) -> Self {
        Self(Arc::new(Mutex::new(AsyncNotifierInner { data, futures: Vec::new() })))
    }

    pub fn wait(
        &self,
        predicate: impl Fn(&T) -> bool + 'static + Send + Sync,
    ) -> Arc<AsyncNotifierFuture> {
        let fut = Arc::new(AsyncNotifierFuture { waker: None, ready: AtomicBool::new(false) });

        self.0.lock().unwrap().futures.push((fut.clone(), Box::new(predicate)));
        fut
    }

    pub fn update(&self, gen_new_value: impl Fn(&T) -> T + 'static + Send + Sync) {
        let mut lock = self.0.lock().unwrap();
        let value = gen_new_value(&lock.data);

        lock.futures.retain(|(future, predicate)| {
            let ready = predicate(&value);
            future.ready.store(ready, Ordering::Relaxed);
            !ready
        });

        lock.data = value;
    }
}
impl<T: Clone> AsyncNotifier<T> {
    pub fn get(&self) -> T { self.0.lock().unwrap().data.clone() }
}

impl<T: Default> Default for AsyncNotifier<T> {
    fn default() -> Self { AsyncNotifier::new(Default::default()) }
}

#[derive(Debug)]
pub struct AsyncNotifierFuture {
    waker: Option<Waker>,
    ready: AtomicBool,
}
impl Future for AsyncNotifierFuture {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.waker.replace(cx.waker().clone());
        if self.ready.load(Ordering::Relaxed) {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}
