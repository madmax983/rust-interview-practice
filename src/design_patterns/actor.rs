#![cfg(feature = "async-parallel")]

//! # Actor Pattern (Channel-Based)
//!
//! Replaces: **Shared State Concurrency** (Mutex/RwLock), **Active Object Pattern**
//!
//! Real Rust usage: `actix`, `tokio::sync::mpsc`, `bastion`
//!
//! ## Why this pattern exists in Rust
//! Rust's "share XOR mutate" rule makes traditional shared-state concurrency (locks) painful.
//! Message passing allows one task to "own" the state and mutate it freely, while others
//! communicate via immutable messages. This aligns perfectly with Rust's ownership model.
//!
//! ## Architecture
//!
//! ```text
//! [ Client A ] --(Message)--> [ MPSC Channel ] --(Recv)--> [ Actor (Loop) ]
//! [ Client B ]                                               |
//!                                                            v
//!                                                      [ Mutable State ]
//! ```
//!
//! **Invariants:**
//! - Only the actor accesses the state (no locks needed).
//! - Messages are processed sequentially (linearizability).
//!
//! ## When to use
//! - When state management is complex (e.g., state machines).
//! - When you want to isolate failure (if actor panics, it can be restarted - though less automatic in raw tokio).
//! - To avoid deadlocks common in complex lock hierarchies.

use std::collections::HashMap;
use tokio::sync::{mpsc, oneshot};

// ============================================================================
// The Messages
// ============================================================================

// OWNERSHIP INSIGHT: Messages transfer ownership of data to the actor.
// `oneshot::Sender` transfers the capability to reply back to the caller.
#[derive(Debug)]
enum Message {
    Store {
        key: String,
        value: String,
    },
    Fetch {
        key: String,
        respond_to: oneshot::Sender<Option<String>>,
    },
}

// ============================================================================
// The Actor (Logic & State)
// ============================================================================

struct DatabaseActor {
    receiver: mpsc::Receiver<Message>,
    data: HashMap<String, String>,
}

impl DatabaseActor {
    fn new(receiver: mpsc::Receiver<Message>) -> Self {
        Self {
            receiver,
            data: HashMap::new(),
        }
    }

    async fn run(&mut self) {
        // Process messages sequentially
        while let Some(msg) = self.receiver.recv().await {
            match msg {
                Message::Store { key, value } => {
                    self.data.insert(key, value);
                }
                Message::Fetch { key, respond_to } => {
                    let value = self.data.get(&key).cloned();
                    // Ignore errors if receiver dropped
                    let _ = respond_to.send(value);
                }
            }
        }
        // Channel closed, actor exits
    }
}

// ============================================================================
// The Handle (Public API)
// ============================================================================

// Cloneable handle to communicate with the actor
#[derive(Clone)]
pub struct DatabaseHandle {
    sender: mpsc::Sender<Message>,
}

impl DatabaseHandle {
    /// Spawns the actor and returns a handle to it.
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel(32);

        // Spawn the actor task
        tokio::spawn(async move {
            let mut actor = DatabaseActor::new(receiver);
            actor.run().await;
        });

        Self { sender }
    }

    pub async fn store(&self, key: String, value: String) {
        let msg = Message::Store { key, value };
        // In a real app, we might handle closed channel error (actor died)
        let _ = self.sender.send(msg).await;
    }

    pub async fn fetch(&self, key: String) -> Option<String> {
        let (send_reply, recv_reply) = oneshot::channel();
        let msg = Message::Fetch {
            key,
            respond_to: send_reply,
        };

        if self.sender.send(msg).await.is_err() {
            return None; // Actor died
        }

        // Await the reply
        recv_reply.await.ok().flatten()
    }
}

impl Default for DatabaseHandle {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Footer
// ============================================================================
//
// Tradeoffs:
// - Latency: Message passing involves serialization/allocation overhead compared to `Mutex`.
// - Backpressure: `mpsc::channel` has a bound (here 32). If actor is slow, callers wait.
//   This is a feature (prevents OOM), but must be tuned.
//
// Comparison to `Arc<Mutex<HashMap>>`:
// - Mutex is faster for simple reads/writes (low contention).
// - Actor is better if writes trigger complex logic (IO, notifications) or if you want to
//   structure the app as communicating processes.

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_actor_store_fetch() {
        let db = DatabaseHandle::new();

        db.store("key1".to_string(), "value1".to_string()).await;

        let result = db.fetch("key1".to_string()).await;
        assert_eq!(result, Some("value1".to_string()));

        let missing = db.fetch("key2".to_string()).await;
        assert_eq!(missing, None);
    }

    #[tokio::test]
    async fn test_concurrent_access() {
        let db = DatabaseHandle::new();

        let db1 = db.clone();
        let t1 = tokio::spawn(async move {
            for i in 0..100 {
                db1.store(format!("k{}", i), format!("v{}", i)).await;
            }
        });

        let db2 = db.clone();
        let t2 = tokio::spawn(async move {
            // Read back
            // We can't guarantee k99 is written yet, but we can write others
            db2.store("shared".to_string(), "locked".to_string()).await;
        });

        let _ = tokio::join!(t1, t2);

        let val = db.fetch("shared".to_string()).await;
        assert_eq!(val, Some("locked".to_string()));

        let val99 = db.fetch("k99".to_string()).await;
        assert_eq!(val99, Some("v99".to_string()));
    }
}
