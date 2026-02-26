//! # Load Balancer Implementation
//!
//! A software load balancer that distributes traffic across multiple backend servers using various algorithms.
//!
//! **Replaces Crates:** `tower-balance`, `p2c` (Power of Two Choices)
//!
//! **Real-world Usage:**
//! - Nginx (Round Robin, Least Conn).
//! - AWS ALB (Application Load Balancer).
//! - Kubernetes Services (kube-proxy).
//!
//! **Why build it yourself?**
//! You learn how distinct algorithms affect traffic distribution.
//! Round Robin is simple but ignores server load.
//! Least Connections requires tracking state (active requests).
//! Weighted Round Robin handles heterogeneous hardware.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

// =========================================================================================
// Architecture
// =========================================================================================
//
// Components:
// - `Backend`: A trait representing a server node.
// - `LoadBalancer`: The main struct managing the list of backends and the selection strategy.
// - `Strategy`: Enum defining the algorithm (RoundRobin, Random, LeastConnections).
//
// Concurrency:
// - `AtomicUsize` for Round Robin index (lock-free).
// - `Mutex` for backend list modification (adding/removing servers).
//
// Invariants:
// 1. Selection must always return a valid backend if the list is non-empty.
// 2. Load distribution should match the expected statistical properties of the strategy.

pub trait Backend: Send + Sync {
    fn id(&self) -> String;
    fn weight(&self) -> usize; // For Weighted Round Robin
    fn active_connections(&self) -> usize; // For Least Connections
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Strategy {
    RoundRobin,
    WeightedRoundRobin,
    LeastConnections,
}

pub struct LoadBalancer<B> {
    backends: Arc<Mutex<Vec<B>>>,
    strategy: Strategy,
    // State for Round Robin
    rr_index: AtomicUsize,
}

impl<B: Backend> LoadBalancer<B> {
    pub fn new(strategy: Strategy) -> Self {
        Self {
            backends: Arc::new(Mutex::new(Vec::new())),
            strategy,
            rr_index: AtomicUsize::new(0),
        }
    }

    pub fn add_backend(&self, backend: B) {
        let mut backends = self.backends.lock().unwrap();
        backends.push(backend);
    }

    pub fn remove_backend(&self, id: &str) {
        let mut backends = self.backends.lock().unwrap();
        backends.retain(|b| b.id() != id);
    }

    /// Selects a backend according to the strategy.
    /// Returns `None` if no backends are available.
    pub fn next(&self) -> Option<B>
    where
        B: Clone, // We return a clone (or Arc) of the backend to the caller
    {
        let backends = self.backends.lock().unwrap();
        if backends.is_empty() {
            return None;
        }

        match self.strategy {
            Strategy::RoundRobin => {
                let idx = self.rr_index.fetch_add(1, Ordering::Relaxed);
                let backend = &backends[idx % backends.len()];
                Some(backend.clone())
            }
            Strategy::LeastConnections => {
                // O(N) scan to find min
                backends
                    .iter()
                    .min_by_key(|b| b.active_connections())
                    .cloned()
            }
            Strategy::WeightedRoundRobin => {
                // Basic implementation: Expand vector logic or probabilistic?
                // Nginx-style "smooth weighted round-robin" is complex.
                // Here we implement a simple probabilistic approach or a flat expansion.
                // Simple approach: Current Index iterates 0..Sum(Weights).
                // But that requires maintaining a virtual array.
                // Let's implement a deterministic logic:
                // Iterate through backends, subtract weight from a counter? No.

                // Let's use the GCD method or just simple iteration if we assume mostly equal weights.
                // For "From Scratch" simplicity, let's just pick based on `(idx % sum_weights)` mapped to ranges.
                // This is O(N) to search the range.

                let total_weight: usize = backends.iter().map(|b| b.weight()).sum();
                if total_weight == 0 {
                    return backends.first().cloned();
                }

                let idx = self.rr_index.fetch_add(1, Ordering::Relaxed) % total_weight;

                let mut current = 0;
                for backend in backends.iter() {
                    current += backend.weight();
                    if idx < current {
                        return Some(backend.clone());
                    }
                }
                // Should not happen
                backends.last().cloned()
            }
        }
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `tower::balance`: Highly composable, integrates with Service trait, supports P2C (Power of Two Choices).
//
// Missing vs. Production:
// - **Health Checks**: We assume all backends in the list are healthy. Real LBs actively probe /health.
// - **Dynamic Weights**: Weights are fixed in `Backend` trait.
// - **Zero-Copy**: We clone the backend. Production systems yields references or handles (Arc).

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug)]
    struct MockBackend {
        id: String,
        weight: usize,
        conns: usize,
    }

    impl MockBackend {
        fn new(id: &str, weight: usize, conns: usize) -> Self {
            Self {
                id: id.to_string(),
                weight,
                conns,
            }
        }
    }

    impl Backend for MockBackend {
        fn id(&self) -> String {
            self.id.clone()
        }
        fn weight(&self) -> usize {
            self.weight
        }
        fn active_connections(&self) -> usize {
            self.conns
        }
    }

    #[test]
    fn test_round_robin() {
        let lb = LoadBalancer::new(Strategy::RoundRobin);
        lb.add_backend(MockBackend::new("A", 1, 0));
        lb.add_backend(MockBackend::new("B", 1, 0));

        let b1 = lb.next().unwrap();
        let b2 = lb.next().unwrap();
        let b3 = lb.next().unwrap();

        assert_eq!(b1.id, "A");
        assert_eq!(b2.id, "B");
        assert_eq!(b3.id, "A");
    }

    #[test]
    fn test_least_connections() {
        let lb = LoadBalancer::new(Strategy::LeastConnections);
        lb.add_backend(MockBackend::new("A", 1, 10));
        lb.add_backend(MockBackend::new("B", 1, 2)); // Least
        lb.add_backend(MockBackend::new("C", 1, 20));

        let b = lb.next().unwrap();
        assert_eq!(b.id, "B");
    }

    #[test]
    fn test_weighted_round_robin() {
        let lb = LoadBalancer::new(Strategy::WeightedRoundRobin);
        // A: 3, B: 1. Total 4.
        // Ranges: A=[0,1,2], B=[3]
        lb.add_backend(MockBackend::new("A", 3, 0));
        lb.add_backend(MockBackend::new("B", 1, 0));

        // Sequence should be A, A, A, B, A, A, A, B ...
        let results: Vec<String> = (0..8).map(|_| lb.next().unwrap().id).collect();

        // Count occurrences
        let count_a = results.iter().filter(|&id| id == "A").count();
        let count_b = results.iter().filter(|&id| id == "B").count();

        assert_eq!(count_a, 6);
        assert_eq!(count_b, 2);
    }

    #[test]
    fn test_empty() {
        let lb: LoadBalancer<MockBackend> = LoadBalancer::new(Strategy::RoundRobin);
        assert!(lb.next().is_none());
    }
}
