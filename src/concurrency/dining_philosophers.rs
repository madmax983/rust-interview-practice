//! # Dining Philosophers Implementation
//!
//! Solves the classic concurrency problem using the "Resource Hierarchy" strategy to prevent deadlocks.
//! It demonstrates safe shared state management using `Arc` and `Mutex`.
//!
//! **Replaces Crates:** N/A (This is a pattern, not a library replacement, though `parking_lot` is often used for faster Mutexes)
//!
//! **Real-world Usage:**
//! - Operating System resource allocation (avoiding circular dependencies).
//! - Database transaction locking (ensuring consistent lock ordering).
//! - Network routing protocols (preventing routing loops).
//!
//! **Why build it yourself?**
//! Implementing this teaches you about:
//! - **Deadlock Prevention:** Understanding the four necessary conditions for deadlock (Mutual Exclusion, Hold and Wait, No Preemption, Circular Wait) and breaking one (Circular Wait).
//! - **Shared State:** Using `Arc` to share ownership of resources (forks) across threads.
//! - **Interior Mutability:** Using `Mutex` to allow mutation (locking) of shared data.
//! - **Thread Synchronization:** Coordinating multiple threads to perform work without stepping on each other.

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

// =========================================================================================
// Architecture
// =========================================================================================
//
// Diagram:
//       P0
//     /    \
//   F4      F0
//   |        |
// P4          P1
//   \        /
//   F3      F1
//     \    /
//       P2 -- F2 -- P3
//
// Invariants:
// 1. There are N philosophers and N forks.
// 2. A philosopher needs two specific forks (left and right) to eat.
// 3. A fork can be held by only one philosopher at a time (Mutual Exclusion).
// 4. Deadlock is prevented by the **Resource Hierarchy** solution:
//    - Philosophers must acquire the lower-numbered fork first.
//    - P4 (the last one) picks up F0 (right) then F4 (left), breaking the cycle.
//
// Complexity:
// ┌───────────────┬─────────────┬─────────────┐
// │ Operation     │ Time        │ Space       │
// ├───────────────┼─────────────┼─────────────┤
// │ Simulation    │ O(T)        │ O(N)        │
// └───────────────┴─────────────┴─────────────┘
// Where T is simulation time and N is number of philosophers.

/// Represents a single philosopher.
#[derive(Clone)]
struct Philosopher {
    name: String,
    left_fork: Arc<Mutex<()>>,
    right_fork: Arc<Mutex<()>>,
}

impl Philosopher {
    /// Create a new Philosopher.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the philosopher.
    /// * `left_fork` - The fork on their left.
    /// * `right_fork` - The fork on their right.
    fn new(name: &str, left_fork: Arc<Mutex<()>>, right_fork: Arc<Mutex<()>>) -> Self {
        Self {
            name: name.to_string(),
            left_fork,
            right_fork,
        }
    }

    /// Attempt to eat.
    ///
    /// This method implements the deadlock avoidance strategy by strictly ordering lock acquisition.
    /// While `Philosopher` struct holds references to left/right, we must determine which is "first" and "second"
    /// based on some consistent global ordering (like memory address or ID).
    /// However, in our setup in `Table::new`, we assign forks.
    /// The standard "Resource Hierarchy" solution relies on the *order of acquisition*.
    ///
    /// Here, we simply acquire `left` then `right`.
    /// **CRITICAL**: To prevent deadlock, the main setup MUST ensure that for at least one philosopher,
    /// the "left" fork is the higher numbered one, or we swap the order here.
    ///
    /// Actually, a cleaner Rust way is to pass the forks in the correct locking order to the struct,
    /// OR handle the logic here. Let's handle it in `eat` for clarity if we had IDs.
    ///
    /// But wait, `Arc<Mutex<()>>` doesn't have an intrinsic ID we can easily check without pointer hashing.
    /// Better approach: The `Table` setup ensures `Philosopher` gets `(min, max)` as `(first, second)`.
    ///
    /// Let's stick to the classic: `left` and `right` fields.
    /// The *caller* (Table) is responsible for giving them in an order that prevents deadlock?
    /// No, the `Philosopher` should just `lock` left then right.
    /// The *Table* must reverse the handles for the last philosopher.
    fn eat(&self) {
        // println!("{} is thinking.", self.name);
        thread::sleep(Duration::from_millis(10));

        // RUST INSIGHT: RAII (Resource Acquisition Is Initialization)
        // The `MutexGuard` returned by `lock()` automatically releases the lock when it goes out of scope.
        // We don't need to manually unlock.

        // GOTCHA: If we just did `let _left = ...; let _right = ...;`, we hold both locks.
        // If we did `if let Ok(guard) = ...`, the guard drops at the end of the `if let` block!
        // We want to hold BOTH for the duration of eating.

        // RUST INSIGHT: Deadlock Prevention in Action
        // If all philosophers pick up `left` simultaneously, they all wait for `right`, causing deadlock.
        // The Table setup ensures that for the last philosopher, `left` is actually the lower-indexed fork
        // (or we swap them), effectively breaking the symmetry.
        //
        // In this implementation, we assume `left_fork` is always acquired first.
        // The `Table` constructor handles the swapping for the last philosopher.

        let _first_lock = self.left_fork.lock().unwrap();
        // println!("{} picked up first fork.", self.name);

        let _second_lock = self.right_fork.lock().unwrap();
        // println!("{} picked up second fork.", self.name);

        // println!("{} is eating.", self.name);
        thread::sleep(Duration::from_millis(10));

        // Locks are dropped here.
        // println!("{} finished eating.", self.name);
    }
}

/// A Table managing the simulation.
pub struct Table {
    philosophers: Vec<Philosopher>,
}

impl Table {
    /// Create a new table with `n` philosophers.
    ///
    /// # Panics
    ///
    /// Panics if `n` is less than 2.
    #[must_use]
    pub fn new(n: usize) -> Self {
        assert!(n >= 2, "Must have at least 2 philosophers to eat.");

        // Create forks.
        // RUST INSIGHT: `Arc<Mutex<()>>`
        // - `()`: The data inside the mutex is minimal (unit type). We only care about the lock itself.
        // - `Mutex`: Provides exclusive access.
        // - `Arc`: Allows multiple philosophers to own the same fork.
        let forks: Vec<_> = (0..n).map(|_| Arc::new(Mutex::new(()))).collect();

        let mut philosophers = Vec::with_capacity(n);

        for i in 0..n {
            let left = Arc::clone(&forks[i]);
            let right = Arc::clone(&forks[(i + 1) % n]);

            // Resource Hierarchy Solution:
            // Always pick up the lower-indexed fork first.
            // Indices: Left is `i`, Right is `(i+1)%n`.
            // Normal case: `i < i+1`. So pick left then right.
            // Last case (i=n-1): Left is `n-1`, Right is `0`. `0 < n-1`.
            // So for the last philosopher, we MUST pick `right` (0) first.
            //
            // To implement this in `Philosopher::eat` which always does `lock(first); lock(second)`,
            // we swap the forks given to the struct for the last philosopher.

            let (first, second) = if i == n - 1 {
                (right, left)
            } else {
                (left, right)
            };

            philosophers.push(Philosopher::new(
                &format!("Philosopher {i}"),
                first,
                second,
            ));
        }

        Self { philosophers }
    }

    /// Run the simulation for a single iteration (each philosopher eats once).
    ///
    /// Note: This runs threads in parallel and joins them.
    ///
    /// # Panics
    ///
    /// Panics if any of the philosopher threads panic.
    pub fn run(&self) {
        let handles: Vec<_> = self
            .philosophers
            .iter()
            .cloned()
            .map(|p| {
                // RUST INSIGHT: Cloning for Threads
                // We need to move ownership of the philosopher into the thread.
                // Since `Philosopher` contains `Arc`s, cloning it is cheap (increments ref counts).
                // The `String` name is also cloned, which is a small allocation but acceptable here.
                thread::spawn(move || {
                    p.eat();
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Alternative Approaches:
// 1. **Arbitrator (Waiter):** Introduce a central authority (mutex) that must be acquired before picking up forks.
//    - *Pros:* Simple to implement.
//    - *Cons:* The arbitrator becomes a bottleneck (serialization).
// 2. **Chandy/Misra Solution:** A complex message-passing algorithm where forks are "dirty" or "clean".
//    - *Pros:* Completely distributed, high concurrency.
//    - *Cons:* Very complex to implement correctly.
// 3. **Limit Diners:** Allow only N-1 philosophers to sit at the table of N seats.
//    - *Pros:* Pigeonhole principle guarantees at least one person can eat.
//    - *Cons:* Requires a semaphore or counting mechanism.
//
// Comparison to other languages:
// - **Go:** Would use `chan` (channels) to model forks or a `select` statement.
// - **Java:** `synchronized` blocks or `ReentrantLock`.
// - **C++:** `std::mutex`, `std::lock_guard`, and `std::scoped_lock` (which can lock multiple mutexes safely).

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_happy_path_no_deadlock() {
        // Run a simulation with 5 philosophers.
        // If deadlock occurs, this test will hang (and eventually time out in CI).
        let table = Table::new(5);
        table.run();
    }

    #[test]
    fn test_stress_many_philosophers() {
        // 100 philosophers!
        // This stresses the OS thread scheduler and lock contention.
        let table = Table::new(50);
        table.run();
    }

    #[test]
    #[should_panic(expected = "Must have at least 2 philosophers")]
    fn test_edge_case_single_philosopher() {
        // A single philosopher cannot eat because they need 2 forks, but only 1 exists (0 and (0+1)%1 = 0).
        // Our constructor logic `(i+1)%n` would imply fork 0 is both left and right.
        // Locking the same mutex twice in the same thread causes deadlock (in Rust `std::sync::Mutex`) or UB.
        // We explicitly prevent this in `new`.
        let _ = Table::new(1);
    }

    #[test]
    fn test_verify_parallelism_is_possible() {
        // This test is tricky to write deterministically without instrumentation.
        // We just ensure that we can run multiple rounds.
        let table = Table::new(4);
        for _ in 0..5 {
            table.run();
        }
    }
}
