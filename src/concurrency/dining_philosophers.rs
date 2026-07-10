//! # 1226. The Dining Philosophers
//!
//! Five silent philosophers sit at a round table with bowls of spaghetti. Forks are placed between each pair of adjacent philosophers.
//! Each philosopher must alternately think and eat. However, a philosopher can only eat spaghetti when they have both left and right forks.
//! Each fork can be held by only one philosopher and so a philosopher can use the fork only if it is not being used by another philosopher.
//!
//! After an individual philosopher finishes eating, they need to put down both forks so that the forks become available to others.
//! A philosopher can take the fork on their right or the one on their left as they become available, but cannot start eating before getting both forks.
//!
//! Eating is not limited by the remaining amounts of spaghetti or stomach space; an infinite supply and an infinite demand are assumed.
//!
//! Problem Link: <https://leetcode.com/problems/the-dining-philosophers/>
//!
//! **Why this matters in Rust:**
//! This problem is a classic example of **deadlock** scenarios in concurrent programming. In Rust, the type system (`Mutex`, `Arc`)
//! prevents data races at compile time, but it does *not* prevent logical errors like deadlocks.
//! By solving this problem, we learn how to manage shared resources safely and avoid circular waits using a consistent resource ordering strategy.
//! It also demonstrates RAII (Resource Acquisition Is Initialization) via `MutexGuard`, which automatically releases locks when they go out of scope.

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

// =========================================================================================
// Approach
// =========================================================================================
//
// To prevent deadlock, we must break the "Circular Wait" condition.
// A common strategy is **Resource Hierarchy** (or Resource Ordering):
//
// 1. Assign a partial order to resources (forks have IDs 0..4).
// 2. Require that all processes (philosophers) request resources in increasing order of enumeration.
//
// For a philosopher sitting between fork `i` and `(i+1)%5`:
// - Their "left" fork is `i`.
// - Their "right" fork is `(i+1)%5`.
//
// Instead of always picking "left" then "right", they must pick `min(left, right)` then `max(left, right)`.
// This breaks the symmetry:
// - Philosophers 0-3 pick their left fork (lower ID) first.
// - Philosopher 4 (who has forks 4 and 0) picks fork 0 (right, lower ID) first.
//
// This ensures that fork 4 is requested only after fork 0 is held by P4, or fork 3 is held by P3.
// Since P0 also needs fork 0, P4 and P0 compete for fork 0 first, preventing the cycle where everyone holds one fork and waits for the next.

/// A fork on the table.
/// We use a Mutex to represent exclusive access. The `()` indicates we only care about the lock itself, not the data protected.
type Fork = Mutex<()>;

/// Represents a philosopher with a unique ID and references to their left and right forks.
struct Philosopher {
    _id: usize,
    first_fork: Arc<Fork>,
    second_fork: Arc<Fork>,
}

impl Philosopher {
    const fn new(id: usize, first_fork: Arc<Fork>, second_fork: Arc<Fork>) -> Self {
        Self {
            _id: id,
            first_fork,
            second_fork,
        }
    }

    /// Simulate the process of eating.
    fn eat(&self) {
        // println!("Philosopher {} is thinking.", self._id);
        thread::sleep(Duration::from_millis(1));

        // println!("Philosopher {} is hungry.", self._id);

        // Lock the first fork
        // RUST INSIGHT: RAII (Resource Acquisition Is Initialization)
        // The lock is acquired when `lock()` returns successfully.
        // It is automatically released when `_first_guard` goes out of scope (at the end of the function).
        // This prevents "forgetting to unlock" bugs common in C/C++.
        let _first_guard = self.first_fork.lock().unwrap();

        // GOTCHA: Mutex Poisoning
        // `lock()` returns a `LockResult`. If the thread holding the lock previously panicked,
        // the mutex is "poisoned". We verify this with `unwrap()`.
        // In a robust system, we might handle the poisoned error, but here we propagate the panic.

        // println!("Philosopher {} picked up first fork.", self._id);

        // Lock the second fork
        let _second_guard = self.second_fork.lock().unwrap();
        // println!("Philosopher {} picked up second fork.", self._id);

        // println!("Philosopher {} is eating.", self._id);
        thread::sleep(Duration::from_millis(1));

        // println!("Philosopher {} put down forks.", self._id);

        // Locks dropped here.
    }
}

pub struct Table {
    forks: Vec<Arc<Fork>>,
}

impl Table {
    /// Create a new table with `n` philosophers.
    #[must_use]
    pub fn new(n: usize) -> Self {
        let forks = (0..n).map(|_| Arc::new(Mutex::new(()))).collect();

        Self { forks }
    }

    /// Run the simulation. Each philosopher eats `meals_per_philosopher` times.
    ///
    /// # Panics
    ///
    /// Panics if there are fewer than 2 philosophers, or if a philosopher
    /// thread panics (e.g. a fork `Mutex` is poisoned).
    pub fn dine(&self, meals_per_philosopher: usize) {
        let (tx, rx) = std::sync::mpsc::channel();
        let num_philosophers = self.forks.len();

        // Handle edge case: need at least 2 philosophers/forks to avoid self-deadlock on single mutex
        assert!(
            num_philosophers >= 2,
            "Dining Philosophers requires at least 2 philosophers to avoid self-deadlock on a single fork."
        );

        let mut handles = vec![];

        for i in 0..num_philosophers {
            // Identify left and right forks
            let left_fork_idx = i;
            let right_fork_idx = (i + 1) % num_philosophers;

            // RUST INSIGHT: Deadlock Prevention via Resource Ordering
            // We enforce a global order on lock acquisition.
            // Always acquire the fork with the lower index first.
            // This breaks the circular dependency chain.
            let (first_fork, second_fork) = if left_fork_idx < right_fork_idx {
                (
                    Arc::clone(&self.forks[left_fork_idx]),
                    Arc::clone(&self.forks[right_fork_idx]),
                )
            } else {
                (
                    Arc::clone(&self.forks[right_fork_idx]),
                    Arc::clone(&self.forks[left_fork_idx]),
                )
            };

            let philosopher = Philosopher::new(i, first_fork, second_fork);
            let tx = tx.clone();

            handles.push(thread::spawn(move || {
                for _ in 0..meals_per_philosopher {
                    philosopher.eat();
                }
                tx.send(i).unwrap();
            }));
        }

        // Drop our sender so we can wait for all threads to finish via rx
        drop(tx);

        // Wait for all philosophers to report completion
        for _ in 0..num_philosophers {
            rx.recv().unwrap();
        }

        // Ensure all threads are joined
        for h in handles {
            h.join().unwrap();
        }
    }
}

// =========================================================================================
// Alternative Approaches
// =========================================================================================
//
// 1. **Arbitrator (Waiter) Solution**:
//    - Introduce a waiter (a Semaphore initialized to N-1).
//    - A philosopher must acquire a permit from the waiter before picking up forks.
//    - This limits concurrency slightly but guarantees deadlock freedom.
//
// 2. **Chandy/Misra Solution**:
//    - Uses dirty/clean tokens to allow arbitrary large systems to resolve conflict without a central arbitrator.
//
// 3. **Try-Lock with Backoff**:
//    - Philosophers pick up left fork.
//    - Try to pick up right fork. If failed, put down left fork and wait (random backoff).
//    - Vulnerable to "livelock" (everyone picks up left, puts down left, repeats in sync).

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dining_philosophers_no_deadlock() {
        let table = Table::new(5);
        // If deadlock occurs, this will hang forever.
        // Rust's test harness usually has a default timeout, but for a learning repo,
        // successful completion of this function proves no deadlock occurred during these iterations.
        table.dine(10);
    }

    #[test]
    fn test_two_philosophers() {
        // Minimal case
        let table = Table::new(2);
        table.dine(10);
    }

    #[test]
    #[should_panic(expected = "requires at least 2 philosophers")]
    fn test_single_philosopher_panic() {
        // A single philosopher would try to lock the same mutex twice (re-entrant lock),
        // which causes a deadlock in Rust's `std::sync::Mutex`.
        // Our implementation panics proactively to avoid hanging the test suite.
        let table = Table::new(1);
        table.dine(1);
    }

    #[test]
    fn test_stress_dining() {
        let table = Table::new(10);
        // Higher contention
        table.dine(50);
    }
}
