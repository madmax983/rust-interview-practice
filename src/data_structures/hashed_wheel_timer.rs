//! # Hashed Wheel Timer Implementation
//!
//! An efficient timer facility for managing large numbers of timeouts with O(1) insertion and cancellation.
//!
//! **Replaces Crates:** `hierarchical_hash_wheel_timer`, `tokio-timer` (historical internals), `netty` (Java)
//!
//! **Real-world Usage:**
//! - Network protocols (TCP retransmission, Keep-Alive).
//! - Task scheduling in event loops.
//! - I/O timeout management.
//!
//! **Why build it yourself?**
//! Standard `BinaryHeap` timers are O(log N). When you have millions of connections, O(log N) adds up.
//! Hashed Wheel Timers are O(1) (amortized) but have lower precision (tick size).
//! You'll learn about "Hierarchical Timing" and handling "Ticks" efficiently.

use std::collections::HashMap;
use std::time::Duration;

// =========================================================================================
// Architecture
// =========================================================================================
//
// Components:
// - `Wheel`: A circular buffer of `Buckets`.
// - `Bucket`: A list of tasks scheduled for that time slot.
// - `Task`: Represents a scheduled action.
//
// Mechanics:
// - `tick_duration`: The time covered by one bucket (e.g., 100ms).
// - `wheel_size`: Number of buckets (e.g., 512).
// - `current_tick`: The current index of the wheel pointer.
//
// Scheduling:
// - Calculate `ticks_in_future = delay / tick_duration`.
// - `rounds = ticks_in_future / wheel_size`.
// - `bucket = (current_tick + ticks_in_future) % wheel_size`.
//
// Execution:
// - On every `tick()`, advance `current_tick`.
// - Inspect the bucket at `current_tick`.
// - If task.rounds > 0, decrement rounds.
// - If task.rounds == 0, execute and remove.
//
// Tradeoffs:
// - Precision is limited to `tick_duration`.
// - Long timeouts require many `rounds` checks (unless Hierarchical).

pub type TaskId = u64;

struct TaskEntry {
    rounds: usize,
    callback: Box<dyn Fn() + Send + Sync>,
}

pub struct HashedWheelTimer {
    tick_duration: Duration,
    wheel: Vec<Vec<TaskId>>,
    tasks: HashMap<TaskId, TaskEntry>,
    current_tick: usize,
    next_id: TaskId,
    residual_time: Duration,
}

impl HashedWheelTimer {
    /// Creates a new Hashed Wheel Timer.
    ///
    /// # Arguments
    /// * `tick_duration` - The resolution of the timer.
    /// * `wheel_size` - Number of buckets. Must be power of 2 for efficiency (though we use % here).
    #[must_use]
    pub fn new(tick_duration: Duration, wheel_size: usize) -> Self {
        // Guard degenerate configuration. A zero tick_duration would cause a
        // divide-by-zero in schedule() (and an infinite loop in tick()); a
        // zero wheel_size would cause a divide/modulo-by-zero. Clamp both to a
        // sane positive minimum.
        let tick_duration = if tick_duration.is_zero() {
            Duration::from_nanos(1)
        } else {
            tick_duration
        };
        let wheel_size = wheel_size.max(1);

        let mut wheel = Vec::with_capacity(wheel_size);
        for _ in 0..wheel_size {
            wheel.push(Vec::new());
        }

        Self {
            tick_duration,
            wheel,
            tasks: HashMap::new(),
            current_tick: 0,
            next_id: 0,
            residual_time: Duration::from_nanos(0),
        }
    }

    /// Schedules a task to be executed after `delay`.
    /// Returns a `TaskId` that can be used to cancel the task.
    pub fn schedule<F>(&mut self, delay: Duration, callback: F) -> TaskId
    where
        F: Fn() + Send + Sync + 'static,
    {
        let mut ticks = (delay.as_nanos() / self.tick_duration.as_nanos()) as usize;
        ticks = ticks.saturating_sub(1);

        let wheel_size = self.wheel.len();
        let rounds = ticks / wheel_size;
        let bucket_idx = (self.current_tick + ticks) % wheel_size;

        let id = self.next_id;
        self.next_id += 1;

        let entry = TaskEntry {
            rounds,
            callback: Box::new(callback),
        };

        self.tasks.insert(id, entry);
        self.wheel[bucket_idx].push(id);

        id
    }

    /// Cancels a scheduled task.
    /// Returns `true` if the task was found and canceled.
    pub fn cancel(&mut self, id: TaskId) -> bool {
        // We just remove from `tasks`.
        // The ID will remain in `wheel` but lookup will fail on execution, effectively ignoring it.
        // This is "Lazy Cancellation".
        self.tasks.remove(&id).is_some()
    }

    /// Advances the timer by the given duration.
    /// Should be called periodically.
    pub fn tick(&mut self, dt: Duration) {
        self.residual_time += dt;

        while self.residual_time >= self.tick_duration {
            self.residual_time -= self.tick_duration;
            self.process_current_bucket();
            self.current_tick = (self.current_tick + 1) % self.wheel.len();
        }
    }

    fn process_current_bucket(&mut self) {
        let bucket_idx = self.current_tick;
        // We need to mutate `self.tasks` and `self.wheel`.
        // We can extract the bucket indices first.

        // Take the bucket out (replace with empty) to avoid borrow checker issues
        // while iterating.
        let entry_ids = std::mem::take(&mut self.wheel[bucket_idx]);

        // We need to know which IDs to KEEP in the bucket, and which to REMOVE from tasks.
        let mut still_pending = Vec::new();

        for id in entry_ids {
            let should_run = if let Some(entry) = self.tasks.get_mut(&id) {
                if entry.rounds > 0 {
                    entry.rounds -= 1;
                    still_pending.push(id);
                    false
                } else {
                    true
                }
            } else {
                false // Canceled
            };

            if should_run {
                // Remove and execute
                if let Some(entry) = self.tasks.remove(&id) {
                    (entry.callback)();
                }
            }
        }

        // Put back pending tasks
        self.wheel[bucket_idx] = still_pending;
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `tokio-timer` (old): Used a hierarchical wheel. New Tokio uses a 64-level wheel.
//
// Missing vs. Production:
// - **Hierarchy**: This is a single-level wheel. Max timeout is `wheel_size * tick_duration`.
//   Wait, we use `rounds`, so max timeout is infinite, but efficiency drops for very long timeouts.
//   Hierarchical wheels avoid the O(N) `rounds` check.
// - **Thread Safety**: This struct is not thread-safe itself. It requires external synchronization (Mutex).
//   Production timers are often internal to an Event Loop (single threaded access).

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn test_schedule_execution() {
        let mut timer = HashedWheelTimer::new(Duration::from_millis(10), 8);
        let counter = Arc::new(AtomicUsize::new(0));
        let c = counter.clone();

        timer.schedule(Duration::from_millis(20), move || {
            c.fetch_add(1, Ordering::SeqCst);
        });

        // Tick 10ms (1 tick)
        timer.tick(Duration::from_millis(10));
        assert_eq!(counter.load(Ordering::SeqCst), 0);

        // Tick 10ms (2 ticks total) -> Should fire
        timer.tick(Duration::from_millis(10));
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_rounds() {
        // Wheel size 4, tick 10ms.
        // Delay 50ms = 5 ticks.
        // Index: 0 -> 1 -> 2 -> 3 -> 0 (Round 1) -> 1 (Fire)
        let mut timer = HashedWheelTimer::new(Duration::from_millis(10), 4);
        let counter = Arc::new(AtomicUsize::new(0));
        let c = counter.clone();

        timer.schedule(Duration::from_millis(50), move || {
            c.fetch_add(1, Ordering::SeqCst);
        });

        // 4 ticks (40ms) - Full rotation
        timer.tick(Duration::from_millis(40));
        assert_eq!(counter.load(Ordering::SeqCst), 0);

        // 1 more tick (50ms)
        timer.tick(Duration::from_millis(10));
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_cancellation() {
        let mut timer = HashedWheelTimer::new(Duration::from_millis(10), 8);
        let counter = Arc::new(AtomicUsize::new(0));
        let c = counter.clone();

        let id = timer.schedule(Duration::from_millis(10), move || {
            c.fetch_add(1, Ordering::SeqCst);
        });

        timer.cancel(id);

        timer.tick(Duration::from_millis(20));
        assert_eq!(counter.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn test_degenerate_config_does_not_panic() {
        // Regression: tick_duration == ZERO caused a divide-by-zero in
        // schedule(); wheel_size == 0 caused a divide/modulo-by-zero. Both
        // must be clamped by the constructor.
        let mut timer = HashedWheelTimer::new(Duration::ZERO, 0);
        let counter = Arc::new(AtomicUsize::new(0));
        let c = counter.clone();

        // schedule() must not panic despite the degenerate config.
        timer.schedule(Duration::from_millis(5), move || {
            c.fetch_add(1, Ordering::SeqCst);
        });

        // tick() must terminate (no infinite loop) and eventually fire.
        timer.tick(Duration::from_millis(10));
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }
}
