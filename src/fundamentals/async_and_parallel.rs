//! # Async and Parallel Patterns
//!
//! Asynchronous programming (tokio) and data parallelism (rayon) patterns
//! for production Rust. These require the `async-parallel` feature flag.
//!
//! **Enable with:** `cargo build --features async-parallel`
//!
//! **Note:** This module is cfg-gated and won't compile without the feature.

#![cfg(feature = "async-parallel")]

// ============================================================================
// Tokio: Async/Await Patterns
// ============================================================================

#[cfg(feature = "async-parallel")]
mod tokio_patterns {
    use tokio::time::{Duration, sleep, timeout};

    /// Basic async function that awaits another async operation.
    #[allow(dead_code)]
    async fn fetch_data(id: u32) -> String {
        // Simulate async I/O with sleep
        sleep(Duration::from_millis(100)).await;
        format!("Data for ID {id}")
    }

    /// Demonstrate basic async/await.
    #[allow(dead_code)]
    async fn demonstrate_basic_async() {
        // Call async function and await result
        let data = fetch_data(42).await;
        println!("{data}");

        // Sequential awaits
        let data1 = fetch_data(1).await;
        let data2 = fetch_data(2).await;
        println!("{data1}, {data2}");
    }

    /// Spawn concurrent tasks with tokio::spawn.
    #[allow(dead_code)]
    async fn demonstrate_spawn() {
        // Spawn tasks that run concurrently
        let task1 = tokio::spawn(async {
            sleep(Duration::from_millis(100)).await;
            println!("Task 1 complete");
            42
        });

        let task2 = tokio::spawn(async {
            sleep(Duration::from_millis(50)).await;
            println!("Task 2 complete");
            100
        });

        // Await both tasks
        let result1 = task1.await.unwrap();
        let result2 = task2.await.unwrap();
        println!("Results: {result1}, {result2}");
    }

    /// Concurrent execution with join! macro.
    #[allow(dead_code)]
    async fn demonstrate_join() {
        // Run multiple futures concurrently and wait for all
        let (data1, data2, data3) = tokio::join!(fetch_data(1), fetch_data(2), fetch_data(3),);

        println!("{data1}, {data2}, {data3}");
    }

    /// Select first completed future with select! macro.
    #[allow(dead_code)]
    async fn demonstrate_select() {
        let task1 = fetch_data(1);
        let task2 = fetch_data(2);

        // Wait for first to complete, cancel the other
        tokio::select! {
            result = task1 => println!("Task 1 won: {result}"),
            result = task2 => println!("Task 2 won: {result}"),
        }
    }

    /// Timeout pattern with tokio::time::timeout.
    #[allow(dead_code)]
    async fn demonstrate_timeout() {
        let slow_operation = async {
            sleep(Duration::from_secs(10)).await;
            "Done!"
        };

        // Timeout after 100ms
        match timeout(Duration::from_millis(100), slow_operation).await {
            Ok(result) => println!("Success: {result}"),
            Err(_) => println!("Timeout!"),
        }
    }

    /// Spawn multiple tasks and collect results.
    #[allow(dead_code)]
    async fn demonstrate_spawn_many() {
        let mut handles = vec![];

        for i in 0..5 {
            let handle = tokio::spawn(async move {
                sleep(Duration::from_millis(50 * i)).await;
                i * 2
            });
            handles.push(handle);
        }

        // Collect all results
        let results: Vec<u64> = futures::future::join_all(handles)
            .await
            .into_iter()
            .map(|r| r.unwrap())
            .collect();

        println!("Results: {results:?}");
    }

    /// Async channels for message passing.
    #[allow(dead_code)]
    async fn demonstrate_async_channels() {
        use tokio::sync::mpsc;

        // Create channel with buffer size
        let (tx, mut rx) = mpsc::channel(32);

        // Spawn sender task
        tokio::spawn(async move {
            for i in 0..5 {
                tx.send(i).await.unwrap();
                sleep(Duration::from_millis(50)).await;
            }
        });

        // Receive messages
        while let Some(msg) = rx.recv().await {
            println!("Received: {msg}");
        }
    }

    /// Shared state with Arc<Mutex<T>> in async context.
    #[allow(dead_code)]
    async fn demonstrate_async_mutex() {
        use std::sync::Arc;
        use tokio::sync::Mutex;

        let counter = Arc::new(Mutex::new(0));
        let mut handles = vec![];

        for _ in 0..5 {
            let counter = Arc::clone(&counter);
            let handle = tokio::spawn(async move {
                let mut num = counter.lock().await;
                *num += 1;
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.await.unwrap();
        }

        println!("Final count: {}", *counter.lock().await);
    }

    /// Async read-write lock (RwLock).
    #[allow(dead_code)]
    async fn demonstrate_async_rwlock() {
        use std::sync::Arc;
        use tokio::sync::RwLock;

        let data = Arc::new(RwLock::new(vec![1, 2, 3]));

        // Multiple readers
        let data1 = Arc::clone(&data);
        let reader1 = tokio::spawn(async move {
            let read = data1.read().await;
            println!("Reader 1: {read:?}");
        });

        let data2 = Arc::clone(&data);
        let reader2 = tokio::spawn(async move {
            let read = data2.read().await;
            println!("Reader 2: {read:?}");
        });

        reader1.await.unwrap();
        reader2.await.unwrap();

        // Single writer
        let mut write = data.write().await;
        write.push(4);
        println!("After write: {write:?}");
    }

    /// Stream processing with async iteration.
    #[allow(dead_code)]
    async fn demonstrate_streams() {
        use tokio_stream::{self as stream, StreamExt};

        // Create stream from range
        let mut stream = stream::iter(vec![1, 2, 3, 4, 5]);

        // Process stream items
        while let Some(item) = stream.next().await {
            println!("Stream item: {item}");
        }

        // Stream combinators
        let stream = stream::iter(vec![1, 2, 3, 4, 5])
            .map(|x| x * 2)
            .filter(|x| x % 4 == 0);

        let results: Vec<i32> = stream.collect().await;
        println!("Filtered stream: {results:?}");
    }

    /// Interval timer for periodic tasks.
    #[allow(dead_code)]
    async fn demonstrate_interval() {
        use tokio::time::interval;

        let mut interval = interval(Duration::from_millis(100));

        for i in 0..5 {
            interval.tick().await;
            println!("Tick {i}");
        }
    }

    /// Practical pattern: Concurrent HTTP requests (concept).
    #[allow(dead_code)]
    async fn fetch_multiple_urls() {
        // Simulated async HTTP fetch
        async fn fetch_url(url: &str) -> String {
            sleep(Duration::from_millis(100)).await;
            format!("Content from {url}")
        }

        let urls = vec![
            "https://example.com/1",
            "https://example.com/2",
            "https://example.com/3",
        ];

        // Fetch concurrently
        let futures = urls.iter().map(|url| fetch_url(url));
        let results = futures::future::join_all(futures).await;

        for result in results {
            println!("{result}");
        }
    }

    /// Practical pattern: Task cancellation.
    #[allow(dead_code)]
    async fn demonstrate_cancellation() {
        use tokio::sync::oneshot;

        let (cancel_tx, cancel_rx) = oneshot::channel();

        let task = tokio::spawn(async move {
            tokio::select! {
                _ = cancel_rx => {
                    println!("Task cancelled");
                }
                _ = sleep(Duration::from_secs(10)) => {
                    println!("Task completed");
                }
            }
        });

        // Cancel after 100ms
        sleep(Duration::from_millis(100)).await;
        cancel_tx.send(()).unwrap();

        task.await.unwrap();
    }

    /// Running the tokio runtime.
    #[allow(dead_code)]
    fn demonstrate_tokio_runtime() {
        // Create runtime
        let rt = tokio::runtime::Runtime::new().unwrap();

        // Execute async code
        rt.block_on(async {
            let result = fetch_data(42).await;
            println!("{result}");
        });

        // Or use #[tokio::main] macro on main function
    }
}

// ============================================================================
// Rayon: Data Parallelism Patterns
// ============================================================================

#[cfg(feature = "async-parallel")]
mod rayon_patterns {
    use rayon::prelude::*;

    /// Basic parallel iterator.
    #[allow(dead_code)]
    fn demonstrate_par_iter() {
        let numbers = vec![1, 2, 3, 4, 5, 6, 7, 8];

        // Sequential iterator
        let sum: i32 = numbers.iter().sum();
        println!("Sequential sum: {sum}");

        // Parallel iterator (automatically uses thread pool)
        let sum: i32 = numbers.par_iter().sum();
        println!("Parallel sum: {sum}");
    }

    /// Parallel map operation.
    #[allow(dead_code)]
    fn demonstrate_par_map() {
        let numbers = vec![1, 2, 3, 4, 5];

        // Parallel map - expensive operation on each element
        let squared: Vec<i32> = numbers
            .par_iter()
            .map(|&x| {
                // Simulate expensive computation
                std::thread::sleep(std::time::Duration::from_millis(10));
                x * x
            })
            .collect();

        println!("Squared: {squared:?}");
    }

    /// Parallel filter and map.
    #[allow(dead_code)]
    fn demonstrate_par_filter_map() {
        let numbers = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

        // Filter evens and square them in parallel
        let result: Vec<i32> = numbers
            .par_iter()
            .filter(|&&x| x % 2 == 0)
            .map(|&x| x * x)
            .collect();

        println!("Filtered and squared: {result:?}");
    }

    /// Parallel fold (reduce).
    #[allow(dead_code)]
    fn demonstrate_par_fold() {
        let numbers = vec![1, 2, 3, 4, 5, 6, 7, 8];

        // Parallel fold with reduce
        let sum = numbers
            .par_iter()
            .fold(|| 0, |acc, &x| acc + x)
            .reduce(|| 0, |a, b| a + b);

        println!("Parallel fold sum: {sum}");

        // Simpler: use sum() which does this automatically
        let sum: i32 = numbers.par_iter().sum();
        println!("Parallel sum: {sum}");
    }

    /// Parallel sorting.
    #[allow(dead_code)]
    fn demonstrate_par_sort() {
        let mut numbers = vec![5, 2, 8, 1, 9, 3, 7, 4, 6];

        // Parallel sort (in-place)
        numbers.par_sort_unstable();
        println!("Sorted: {numbers:?}");

        // Parallel sort with custom comparator
        numbers.par_sort_unstable_by(|a, b| b.cmp(a)); // Descending
        println!("Sorted descending: {numbers:?}");
    }

    /// Parallel partition.
    #[allow(dead_code)]
    fn demonstrate_par_partition() {
        let numbers = vec![1, 2, 3, 4, 5, 6, 7, 8];

        // Partition into evens and odds in parallel
        let (evens, odds): (Vec<&i32>, Vec<&i32>) = numbers.par_iter().partition(|&&x| x % 2 == 0);

        println!("Evens: {evens:?}");
        println!("Odds: {odds:?}");
    }

    /// Parallel find (short-circuits on first match).
    #[allow(dead_code)]
    fn demonstrate_par_find() {
        let numbers = vec![1, 2, 3, 4, 5, 6, 7, 8];

        // Find first number > 5 (in parallel)
        if let Some(&result) = numbers.par_iter().find_first(|&&x| x > 5) {
            println!("Found: {result}");
        }

        // find_any (non-deterministic, may return any match)
        if let Some(&result) = numbers.par_iter().find_any(|&&x| x > 5) {
            println!("Found any: {result}");
        }
    }

    /// Parallel chunks processing.
    #[allow(dead_code)]
    fn demonstrate_par_chunks() {
        let numbers: Vec<i32> = (0..100).collect();

        // Process in parallel chunks of 10
        let chunk_sums: Vec<i32> = numbers
            .par_chunks(10)
            .map(|chunk| chunk.iter().sum())
            .collect();

        println!("Chunk sums: {chunk_sums:?}");
    }

    /// Custom thread pool configuration.
    #[allow(dead_code)]
    fn demonstrate_thread_pool() {
        use rayon::ThreadPoolBuilder;

        // Create custom thread pool with 4 threads
        let pool = ThreadPoolBuilder::new().num_threads(4).build().unwrap();

        // Execute work in the pool
        let sum = pool.install(|| (0..100).into_par_iter().map(|x| x * x).sum::<i32>());

        println!("Sum in custom pool: {sum}");
    }

    /// Parallel iteration over multiple collections (zip).
    #[allow(dead_code)]
    fn demonstrate_par_zip() {
        let a = vec![1, 2, 3, 4, 5];
        let b = vec![10, 20, 30, 40, 50];

        // Parallel zip and map
        let result: Vec<i32> = a.par_iter().zip(&b).map(|(&x, &y)| x + y).collect();

        println!("Zipped sum: {result:?}");
    }

    /// Practical pattern: Parallel map-reduce.
    #[allow(dead_code)]
    fn parallel_word_count(documents: &[String]) -> usize {
        use std::collections::HashMap;

        // Count words in each document in parallel
        let counts: Vec<HashMap<String, usize>> = documents
            .par_iter()
            .map(|doc| {
                let mut count = HashMap::new();
                for word in doc.split_whitespace() {
                    *count.entry(word.to_string()).or_insert(0) += 1;
                }
                count
            })
            .collect();

        // Merge counts (sequential for simplicity)
        let mut total = HashMap::new();
        for count in counts {
            for (word, freq) in count {
                *total.entry(word).or_insert(0) += freq;
            }
        }

        total.values().sum()
    }

    /// Practical pattern: Parallel image processing (concept).
    #[allow(dead_code)]
    fn process_images(pixels: &mut [u8]) {
        // Process pixels in parallel chunks
        pixels.par_chunks_mut(4).for_each(|rgba| {
            // Example: invert colors
            rgba[0] = 255 - rgba[0]; // R
            rgba[1] = 255 - rgba[1]; // G
            rgba[2] = 255 - rgba[2]; // B
            // rgba[3] is alpha, leave unchanged
        });
    }

    /// Practical pattern: Parallel file processing (concept).
    #[allow(dead_code)]
    fn process_files(file_paths: &[String]) -> Vec<String> {
        file_paths
            .par_iter()
            .map(|path| {
                // Simulate reading and processing file
                format!("Processed: {path}")
            })
            .collect()
    }

    /// When NOT to use rayon (overhead example).
    #[allow(dead_code)]
    fn demonstrate_overhead() {
        let small_vec = vec![1, 2, 3, 4, 5];

        // Sequential is faster for small data
        let _sum: i32 = small_vec.iter().sum();

        // Parallel has overhead - not worth it for small data
        let _sum: i32 = small_vec.par_iter().sum();

        // Rule of thumb: Use rayon when work per item is significant
        // or data set is large (thousands+ of items)
    }
}

// ============================================================================
// Comparison and Best Practices
// ============================================================================

#[allow(dead_code)]
fn demonstrate_comparison() {
    println!("=== Async (tokio) vs Parallel (rayon) ===");
    println!();

    println!("Use tokio for:");
    println!("  - I/O-bound operations (network, file system, database)");
    println!("  - Concurrent requests (HTTP clients, APIs)");
    println!("  - Event-driven systems");
    println!("  - Thousands of concurrent tasks (lightweight)");
    println!("  - Waiting for external events");
    println!();

    println!("Use rayon for:");
    println!("  - CPU-bound operations (computation, processing)");
    println!("  - Data parallelism (process collection items in parallel)");
    println!("  - Embarrassingly parallel problems");
    println!("  - Large data sets with independent work items");
    println!("  - Image/video processing, scientific computing");
    println!();

    println!("Hybrid approach:");
    println!("  - Use tokio for I/O coordination");
    println!("  - Use rayon for CPU-intensive work within async tasks");
    println!("  - Example: async HTTP server using rayon for request processing");
}

/// Example: Combining tokio and rayon.
#[cfg(feature = "async-parallel")]
#[allow(dead_code)]
async fn hybrid_example() {
    use rayon::prelude::*;
    use tokio::task::spawn_blocking;

    // CPU-intensive work in rayon (blocking)
    let result: i32 = spawn_blocking(|| (0..1_000_000).into_par_iter().map(|x| x * x).sum::<i32>())
        .await
        .unwrap();

    println!("Computed result: {result}");
}

// ============================================================================
// Usage Notes
// ============================================================================

/// Enable this module with:
/// ```bash
/// cargo build --features async-parallel
/// cargo test --features async-parallel
/// ```
///
/// Or add to your Cargo.toml:
/// ```toml
/// [dependencies]
/// tokio = { version = "1", features = ["full"] }
/// rayon = "1.10"
/// ```
///
/// Common imports:
/// ```rust
/// use tokio::time::{sleep, timeout, Duration};
/// use rayon::prelude::*;
/// ```
#[allow(dead_code)]
const USAGE_NOTES: &str = "See module docs for usage";
