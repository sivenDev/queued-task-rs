#![doc = include_str!("../README.MD")]
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::error::{SendError, SendTimeoutError};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::{mpsc, Notify, Semaphore};
use tokio::time::Instant;
use tracing::{Instrument, Span};

/// Internal shared state for task results.
/// Replaces the manual `Shared` implementation with a more direct Arc-wrapped struct.
struct InnerShared<R> {
    data: tokio::sync::Mutex<Option<R>>,
    notify: Notify,
}

pub struct Task<T, R> {
    inner: T,
    shared: Arc<InnerShared<R>>,
    start_time: Instant,
    span: Option<Span>,
}

impl<T, R> Task<T, R> {
    fn new(inner: T, shared: Arc<InnerShared<R>>, span: Option<Span>) -> Self {
        Self {
            inner,
            shared,
            start_time: Instant::now(),
            span,
        }
    }
}

pub struct TaskState<R> {
    shared: Arc<InnerShared<R>>,
}

impl<R> TaskState<R> {
    /// Wait for the task to complete and return the result.
    pub async fn wait_result(self) -> Option<R> {
        self.shared.notify.notified().await;
        self.shared.data.lock().await.take()
    }
}

pub struct QueuedTask<T, R> {
    sender: Sender<Task<T, R>>,
}

impl<T, R> QueuedTask<T, R> {
    pub fn capacity(&self) -> usize {
        self.sender.capacity()
    }

    pub async fn push(&self, inner: T) -> Result<TaskState<R>, SendError<Task<T, R>>> {
        self.push_with_span(inner, None).await
    }

    pub async fn push_with_span(
        &self,
        inner: T,
        span: Option<Span>,
    ) -> Result<TaskState<R>, SendError<Task<T, R>>> {
        let shared = Arc::new(InnerShared {
            data: tokio::sync::Mutex::new(None),
            notify: Notify::new(),
        });
        self.sender
            .send(Task::new(inner, shared.clone(), span))
            .await?;
        Ok(TaskState { shared })
    }

    pub async fn push_timeout(
        &self,
        inner: T,
        timeout: Duration,
    ) -> Result<TaskState<R>, SendTimeoutError<Task<T, R>>> {
        self.push_timeout_with_span(inner, timeout, None).await
    }

    pub async fn push_timeout_with_span(
        &self,
        inner: T,
        time_out: Duration,
        span: Option<Span>,
    ) -> Result<TaskState<R>, SendTimeoutError<Task<T, R>>> {
        let shared = Arc::new(InnerShared {
            data: tokio::sync::Mutex::new(None),
            notify: Notify::new(),
        });
        self.sender
            .send_timeout(Task::new(inner, shared.clone(), span), time_out)
            .await?;
        Ok(TaskState { shared })
    }
}

pub struct QueuedTaskBuilder<F, T, R> {
    handle: Option<F>,
    queue_len: usize,
    rate: usize,
}

impl<F, T, Fut, R> QueuedTaskBuilder<F, T, R>
where
    F: Fn(Duration, T) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = R> + Send + 'static,
    T: Send + 'static,
    R: Send + 'static,
{
    pub fn new(queue_len: usize, rate: usize) -> Self {
        Self {
            handle: None,
            queue_len,
            rate,
        }
    }

    pub fn handle(mut self, f: F) -> Self {
        self.handle = Some(f);
        self
    }

    pub fn build(self) -> QueuedTask<T, R> {
        let (sender, mut receiver) = mpsc::channel(self.queue_len);
        let handle = Arc::new(self.handle.expect("handle must be set"));
        let sem = Arc::new(Semaphore::new(self.rate));

        tokio::spawn(async move {
            while let Some(task) = receiver.recv().await {
                // Correct semaphore usage: acquire permit before spawning task execution.
                let permit = match sem.clone().acquire_owned().await {
                    Ok(p) => p,
                    Err(_) => break,
                };
                
                let h = handle.clone();
                let Task { inner, shared, start_time, span } = task;

                tokio::spawn(async move {
                    let wait = start_time.elapsed();
                    let result = match span {
                        Some(s) => h(wait, inner).instrument(s).await,
                        None => h(wait, inner).await,
                    };

                    let mut data = shared.data.lock().await;
                    *data = Some(result);
                    shared.notify.notify_one();
                    drop(permit);
                });
            }
        });

        QueuedTask { sender }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test() {
        let t = QueuedTaskBuilder::new(10, 2).handle(handle).build();

        async fn handle(wait_time: Duration, c: usize) -> usize {
            tokio::time::sleep(Duration::from_secs(1)).await;
            println!("task {} waited {}ms", c, wait_time.as_millis());
            c
        }

        let mut ts = vec![];

        for i in 0..10 {
            let state = t.push(i).await.unwrap();
            ts.push(tokio::spawn(async move {
                let result = state.wait_result().await;
                assert_eq!(result, Some(i));
            }));
        }

        for x in ts {
            let _ = x.await;
        }
    }
}
