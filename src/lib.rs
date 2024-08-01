/// # Concurrent Queue Task Processing Library
///
/// This Rust library provides a robust solution for handling queue tasks in high concurrency scenarios. It ensures tasks
/// are processed in order, enhancing service stability. Additionally, it allows configurable parallel task processing to
/// optimize performance.
///
/// ## Features
///
/// - **Queue-Based Task Management**: Tasks are enqueued and processed sequentially.
/// - **High Concurrency Handling**: Designed for high-load environments to maintain stability.
/// - **Configurable Parallel Processing**: Set the number of tasks processed concurrently via parameters.
///
/// ## Installation
///
/// Add this to your `Cargo.toml`:
///
/// ```toml
/// [dependencies]
/// queued-task = "0.1.0"
/// ```
///
/// # Usage
///
/// ```rust
///     use std::sync::Arc;
///     use std::time::Duration;
///     use queued_task::QueuedTaskBuilder;
///
///     #[tokio::test]
///     async fn test() {
///         let t = Arc::new(QueuedTaskBuilder::new(10, 2).handle(handle).build());
///
///         async fn handle(wait_time: Duration, c: usize) -> usize {
///             tokio::time::sleep(Duration::from_secs(1)).await;
///             println!("{} {}", c, wait_time.as_millis());
///             c
///         }
///
///         let mut ts = vec![];
///
///         for i in 0..20 {
///             let tt = t.clone();
///             ts.push(tokio::spawn(async move {
///                 /// push task
///                 let state = tt.push(i).await.unwrap();
///                 /// waiting for task result
///                 let result = state.wait_result().await;
///                 dbg!(result);
///             }));
///         }
///
///         for x in ts {
///             let _ = x.await;
///         }
///     }
/// ```
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::error::{SendError, SendTimeoutError};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::{mpsc, Mutex};
use tokio::sync::{Notify, Semaphore};
use tokio::time::Instant;

struct Shared<R> {
    notify: Arc<Notify>,
    data: Arc<Mutex<Option<R>>>,
}

impl<R> Clone for Shared<R> {
    fn clone(&self) -> Self {
        Self {
            notify: self.notify.clone(),
            data: self.data.clone(),
        }
    }
}

impl<R> Shared<R> {
    fn new() -> Self {
        Self {
            notify: Arc::new(Notify::new()),
            data: Arc::new(Mutex::new(None)),
        }
    }

    async fn set_result(self, result: R) {
        self.data.lock().await.replace(result);
        self.notify.notify_one();
    }

    async fn wait_result(self) -> Option<R> {
        self.notify.notified().await;
        self.data.lock().await.take()
    }
}

pub struct Task<T, R> {
    inner: T,
    shared: Shared<R>,
    start_time: Instant,
}

impl<T, R> Task<T, R> {
    fn new(inner: T, shared: Shared<R>) -> Self {
        Self {
            inner,
            shared,
            start_time: Instant::now(),
        }
    }
}

pub struct TaskState<R> {
    shared: Shared<R>,
}

impl<R> TaskState<R> {
    pub async fn wait_result(self) -> Option<R> {
        self.shared.wait_result().await
    }
}

// #[derive(Debug)]
// pub struct Config {
//     length: usize,
//     keep_alive_timeout: Duration,
// }
//
// impl Default for Config {
//     fn default() -> Self {
//         Self {
//             length: 16,
//             keep_alive_timeout: Duration::from_secs(30),
//         }
//     }
// }

pub struct QueuedTask<T, R> {
    sender: Sender<Task<T, R>>,
}

impl<T, R> QueuedTask<T, R> {
    pub fn capacity(&self) -> usize {
        self.sender.capacity()
    }

    pub async fn push(&self, inner: T) -> Result<TaskState<R>, SendError<Task<T, R>>> {
        let shared = Shared::new();
        self.sender.send(Task::new(inner, shared.clone())).await?;
        Ok(TaskState { shared })
    }

    pub async fn push_timeout(
        &self,
        inner: T,
        time_out: Duration,
    ) -> Result<TaskState<R>, SendTimeoutError<Task<T, R>>> {
        let shared = Shared::new();
        self.sender
            .send_timeout(Task::new(inner, shared.clone()), time_out)
            .await?;
        Ok(TaskState { shared })
    }
}

pub struct QueuedTaskBuilder<F, T, R> {
    // config: Config,
    handle: Option<F>,
    sem: Semaphore,
    sender: Sender<Task<T, R>>,
    receiver: Receiver<Task<T, R>>,
}

impl<F, T, Fut, R> QueuedTaskBuilder<F, T, R>
where
    F: Fn(Duration, T) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = R> + Send + 'static,
    T: Send + 'static,
    R: Send + 'static,
{
    pub fn new(queue_len: usize, rate: usize) -> Self {
        let (sender, receiver) = mpsc::channel(queue_len);
        Self {
            // config,
            sem: Semaphore::new(rate),
            handle: None,
            sender,
            receiver,
        }
    }

    pub fn handle(mut self, f: F) -> Self {
        self.handle = Some(f);
        self
    }

    pub fn build(self) -> QueuedTask<T, R> {
        let Self {
            sem,
            mut handle,
            sender,
            mut receiver,
            ..
        } = self;
        let handle = handle.take().unwrap();
        tokio::spawn(async move {
            let arc_sem = Arc::new(sem);
            let arc_handle = Arc::new(handle);
            while let Some(Task {
                inner,
                shared,
                start_time,
            }) = receiver.recv().await
            {
                let p = arc_sem.clone().acquire_owned().await.unwrap();
                let h = arc_handle.clone();
                tokio::spawn(async move {
                    let wait = start_time.elapsed();
                    let result = h(wait, inner).await;
                    shared.set_result(result).await;
                    drop(p)
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
        let t = Arc::new(QueuedTaskBuilder::new(10, 2).handle(handle).build());

        async fn handle(wait_time: Duration, c: usize) -> usize {
            tokio::time::sleep(Duration::from_secs(1)).await;
            println!("{} {}", c, wait_time.as_millis());
            c
        }

        let mut ts = vec![];

        for i in 0..20 {
            let tt = t.clone();
            ts.push(tokio::spawn(async move {
                // push task
                let state = tt.push(i).await.unwrap();
                // waiting for task result
                let result = state.wait_result().await;
                dbg!(result);
            }));
        }

        for x in ts {
            let _ = x.await;
        }
    }
}
