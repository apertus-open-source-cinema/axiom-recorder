use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll, Waker},
};

pub struct AsyncNotifierInner<T> {
    data: T,
    futures: Vec<(AsyncNotifierFuture, Box<dyn Fn(&T) -> bool + Send + Sync>)>,
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
    ) -> AsyncNotifierFuture {
        let mut lock = self.0.lock().unwrap();
        let ready = predicate(&lock.data);
        let fut = AsyncNotifierFuture(Arc::new(Mutex::new(AsyncNotifierFutureInner {
            waker: None,
            ready,
            poll_count: 0,
        })));
        lock.futures.push((fut.clone(), Box::new(predicate)));
        fut
    }

    pub fn update<R>(&self, modify: impl FnOnce(&mut T) -> R + Send + Sync) -> R {
        let mut lock = self.0.lock().unwrap();
        let AsyncNotifierInner { data, futures } = &mut *lock;
        let to_return = modify(data);

        futures.retain(|(future, predicate)| {
            let ready = predicate(data);
            if ready {
                let mut future = future.0.lock().unwrap();
                future.ready = true;
                if let Some(waker) = future.waker.take() {
                    waker.wake();
                } else {
                }
            }
            !ready
        });

        to_return
    }
}
impl<T: Clone> AsyncNotifier<T> {
    pub fn get(&self) -> T { self.0.lock().unwrap().data.clone() }
}

impl<T: Default> Default for AsyncNotifier<T> {
    fn default() -> Self { AsyncNotifier::new(Default::default()) }
}

#[derive(Debug)]
pub struct AsyncNotifierFutureInner {
    waker: Option<Waker>,
    ready: bool,
    poll_count: u64,
}
#[derive(Debug, Clone)]
pub struct AsyncNotifierFuture(Arc<Mutex<AsyncNotifierFutureInner>>);
impl Future for AsyncNotifierFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut lock = self.0.lock().unwrap();
        lock.poll_count += 1;
        lock.waker.replace(cx.waker().clone());
        if lock.ready {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}
