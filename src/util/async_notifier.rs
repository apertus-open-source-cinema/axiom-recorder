use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll, Waker},
};

type CheckCondition<T> = dyn Fn(&T) -> bool + Send + Sync;

pub struct AsyncNotifierInner<T> {
    data: T,
    futures: Vec<(AsyncNotifierFuture<T>, Box<CheckCondition<T>>)>,
}
#[derive(Clone)]
pub struct AsyncNotifier<T>(Arc<Mutex<AsyncNotifierInner<T>>>);
impl<T: Clone> AsyncNotifier<T> {
    pub fn new(data: T) -> Self {
        Self(Arc::new(Mutex::new(AsyncNotifierInner { data, futures: Vec::new() })))
    }

    pub fn wait(
        &self,
        predicate: impl Fn(&T) -> bool + 'static + Send + Sync,
    ) -> AsyncNotifierFuture<T> {
        let mut lock = self.0.lock().unwrap();
        let data = if predicate(&lock.data) {
            Some(lock.data.clone())
        } else {
            None
        };

        let fut = AsyncNotifierFuture(Arc::new(Mutex::new(AsyncNotifierFutureInner {
            waker: None,
            data,
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
                future.data = Some(data.clone());
                if let Some(waker) = future.waker.take() {
                    waker.wake();
                }
            }
            !ready
        });

        to_return
    }
}

impl<T: Default + Clone> Default for AsyncNotifier<T> {
    fn default() -> Self { AsyncNotifier::new(Default::default()) }
}

#[derive(Debug)]
pub struct AsyncNotifierFutureInner<T> {
    waker: Option<Waker>,
    data: Option<T>,
}
#[derive(Debug)]
#[must_use = "futures do nothing unless awaited"]
pub struct AsyncNotifierFuture<T>(Arc<Mutex<AsyncNotifierFutureInner<T>>>);
impl<T> Future for AsyncNotifierFuture<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut lock = self.0.lock().unwrap();
        lock.waker.replace(cx.waker().clone());
        if let Some(data) = lock.data.take() {
            Poll::Ready(data)
        } else {
            Poll::Pending
        }
    }
}

impl<T> Clone for AsyncNotifierFuture<T> {
    fn clone(&self) -> Self { Self(self.0.clone()) }
}
