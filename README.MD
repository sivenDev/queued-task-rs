# Concurrent Queue Task Processing Library

This Rust library provides a robust solution for handling queue tasks in high concurrency scenarios. It ensures tasks
are processed in order, enhancing service stability. Additionally, it allows configurable parallel task processing to
optimize performance.

## Features

- **Queue-Based Task Management**: Tasks are enqueued and processed sequentially.
- **High Concurrency Handling**: Designed for high-load environments to maintain stability.
- **Configurable Parallel Processing**: Set the number of tasks processed concurrently via parameters.

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
queued-task = "0.1.0"
```

# Usage

```rust
    use std::sync::Arc;
    use std::time::Duration;
    use queued_task::QueuedTaskBuilder;
    
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
```