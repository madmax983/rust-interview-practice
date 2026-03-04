//! # Pub/Sub Event Bus Implementation
//!
//! A high-performance, concurrent Publish-Subscribe (Pub/Sub) event bus
//! that allows publishers to broadcast messages to multiple subscribers based on topics.
//!
//! **Replaces Crates:** `bus`, `tokio::sync::broadcast` (synchronous subset)
//!
//! **Real-world Usage:**
//! - Event-driven architectures (decoupling services).
//! - UI State management (React/Redux, game engines).
//! - System metrics aggregation.
//!
//! **Why build it yourself?**
//! Implementing a Pub/Sub bus teaches you about the Observer pattern in concurrent Rust.
//! You must handle dynamic subscriptions (adding/removing listeners), avoid deadlocks when
//! firing events from within an event handler, and manage the lifetimes of subscribers safely
//! using weak references or explicit unsubscription.

use std::collections::HashMap;
use std::hash::Hash;
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::sync::{Arc, Mutex, RwLock, Weak};

// =========================================================================================
// Architecture
// =========================================================================================
//
// Components:
// - `EventBus`: Central registry mapping Topics to a list of Subscribers.
// - `Subscriber`: Receives messages via an `mpsc::SyncSender`.
// - `Topic`: The routing key.
// - `Message`: The payload.
//
// Data Structure:
//
//      EventBus
//      ├── RwLock<HashMap<Topic, Vec<Weak<SyncSender<Message>>>>>
//
// Invariants:
// 1. **No Deadlocks**: Publishing an event must not hold a lock that prevents a subscriber from
//    subscribing to a new topic or publishing another event (reentrancy).
// 2. **Memory Leaks**: When a subscriber drops its `Receiver`, the EventBus must clean up the
//    stale `Sender`. We use `Weak` pointers so the bus doesn't keep subscribers alive artificially.
// 3. **Bounded Queues**: We use `sync_channel` to prevent slow consumers from OOMing the publisher.
//
// Complexity:
// ┌───────────────┬─────────────┬─────────────┐
// │ Operation     │ Time        │ Space       │
// ├───────────────┼─────────────┼─────────────┤
// │ Subscribe     │ O(1)        │ O(1)        │
// │ Publish       │ O(S)*       │ O(S)        │
// └───────────────┴─────────────┴─────────────┘
// * S = number of subscribers to the topic.
//
// Design Decisions:
// - **Weak Pointers**: The bus holds `Weak<SyncSender>`. The subscriber owns the `Arc<SyncSender>`.
//   When the subscriber drops the `Arc`, the `Weak` pointer becomes invalid, and the bus cleans it up on the next publish.
// - **RwLock**: Optimizes for the common case where publishing (iterating the map) is frequent,
//   and subscribing (mutating the map) is infrequent. We use a read lock for publishing.
//   Wait, iterating and calling `send` needs a read lock on the map, but we might need to remove dead weak pointers.
//   To avoid upgrading locks during publish, we can defer cleanup or use interior mutability on the vector.
//   For simplicity and safety, `publish` will read, collect valid senders, then send outside the lock.
//   A separate cleanup step or periodic sweep can remove dead weak pointers.
//   Actually, `RwLock::write` on the vector is needed to prune dead weak pointers.
//   Let's use `RwLock<HashMap<T, Mutex<Vec<Weak<SyncSender<M>>>>>>` to allow concurrent publishing
//   across different topics, and localized cleanup.

/// A concurrent Pub/Sub trait.
/// Defines the standard interface for publishing and subscribing to topics.
pub trait PubSub<T, M> {
    type Subscription;

    /// Subscribes to a specific topic, returning a handle to receive messages.
    fn subscribe(&self, topic: T) -> Self::Subscription;

    /// Publishes a message to all subscribers of the given topic.
    /// Returns the number of successful deliveries.
    fn publish(&self, topic: &T, message: M) -> usize;
}

/// Represents a subscriber's connection to the bus.
/// Dropping this struct will automatically unsubscribe the listener.
pub struct Subscription<M> {
    receiver: Receiver<M>,
    _sender: Arc<SyncSender<M>>, // Keeps the Weak reference alive in the bus
}

impl<M> Subscription<M> {
    /// Wait for the next message on this subscription.
    pub fn recv(&self) -> Result<M, std::sync::mpsc::RecvError> {
        self.receiver.recv()
    }

    /// Tries to receive the next message without blocking.
    pub fn try_recv(&self) -> Result<M, std::sync::mpsc::TryRecvError> {
        self.receiver.try_recv()
    }
}

/// The main Event Bus.
pub struct EventBus<T, M> {
    // Map of Topic -> List of Weak Senders
    // The inner Mutex allows pruning dead subscribers while only holding a read lock on the HashMap.
    subscribers: RwLock<HashMap<T, Mutex<Vec<Weak<SyncSender<M>>>>>>,
    capacity: usize,
}

impl<T, M> EventBus<T, M>
where
    T: Hash + Eq + Clone,
    M: Clone + Send + 'static,
{
    /// Creates a new Event Bus.
    /// `capacity` is the bounded size of the channel for each subscriber.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            subscribers: RwLock::new(HashMap::new()),
            capacity,
        }
    }

    /// Forces a sweep of dead subscriptions across all topics.
    /// Normally handled automatically during `publish`, but can be called manually
    /// if topics are rarely published to.
    pub fn clean_dead_subscriptions(&self) {
        let map = self.subscribers.read().unwrap();
        for subs in map.values() {
            let mut list = subs.lock().unwrap();
            list.retain(|weak| weak.upgrade().is_some());
        }
    }
}

impl<T, M> PubSub<T, M> for EventBus<T, M>
where
    T: Hash + Eq + Clone,
    M: Clone + Send + 'static,
{
    type Subscription = Subscription<M>;

    /// Subscribes to a specific topic.
    /// Returns a `Subscription` handle. The subscriber receives messages via this handle.
    fn subscribe(&self, topic: T) -> Self::Subscription {
        let (sender, receiver) = sync_channel(self.capacity);
        let arc_sender = Arc::new(sender);
        let weak_sender = Arc::downgrade(&arc_sender);

        // 1. Try to get read lock to see if topic exists
        let map = self.subscribers.read().unwrap();
        if let Some(subs) = map.get(&topic) {
            let mut list = subs.lock().unwrap();
            list.push(weak_sender);
            return Subscription {
                receiver,
                _sender: arc_sender,
            };
        }
        drop(map);

        // 2. Topic doesn't exist, get write lock to insert
        let mut map = self.subscribers.write().unwrap();
        // Double check in case another thread inserted it while we waited for write lock
        let subs = map.entry(topic).or_insert_with(|| Mutex::new(Vec::new()));
        let mut list = subs.lock().unwrap();
        list.push(weak_sender);

        Subscription {
            receiver,
            _sender: arc_sender,
        }
    }

    /// Publishes a message to all subscribers of the given topic.
    /// Returns the number of successful deliveries.
    fn publish(&self, topic: &T, message: M) -> usize {
        let map = self.subscribers.read().unwrap();

        let subs = match map.get(topic) {
            Some(subs) => subs,
            None => return 0, // No subscribers for topic
        };

        let mut list = subs.lock().unwrap();
        let mut delivered = 0;
        let mut to_remove = Vec::new();

        for (i, weak_sender) in list.iter().enumerate() {
            if let Some(sender) = weak_sender.upgrade() {
                // RUST INSIGHT: We clone the message for each subscriber.
                // If the message is large, `M` should be an `Arc<ActualData>` to avoid deep copies.
                // The `try_send` prevents a slow subscriber from blocking the publisher.
                // If their queue is full, we drop the message (standard Pub/Sub behavior for slow consumers).
                if sender.try_send(message.clone()).is_ok() {
                    delivered += 1;
                }
            } else {
                // The subscriber dropped their Subscription handle.
                // Mark for removal.
                to_remove.push(i);
            }
        }

        // Clean up dead weak pointers
        // Iterate backwards to safely remove by index without shifting issues
        for idx in to_remove.into_iter().rev() {
            list.remove(idx);
        }

        delivered
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `tokio::sync::broadcast`: Async, heavily optimized, supports multi-producer multi-consumer (MPMC)
//   on a single stream with lag detection.
// - `bus`: Synchronous, lock-free ring buffer under the hood, but strictly single-producer.
//
// Missing vs. Production:
// - **Lag Detection**: If a subscriber's channel fills up, `try_send` fails silently here.
//   Production buses return a `Lagged` error to the subscriber so they know they missed events.
// - **Wildcard Routing**: Does not support topics like `user.*` or hierarchical routing (like RabbitMQ/MQTT).
//
// Next Steps:
// 1. Add `Lagged` error reporting.
// 2. Implement hierarchical topic matching (e.g., using a Trie instead of HashMap).
//
// Benchmarking Note:
// To benchmark the EventBus, measure publish throughput using `criterion` across N threads.
// 1. Spawn `S` subscribers on different topics.
// 2. Spawn `P` publishers sending messages in a tight loop.
// 3. Compare latency with and without `sync_channel` blocking by varying capacity.

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[derive(Clone, Debug, PartialEq, Eq, Hash)]
    enum Topic {
        System,
        User,
    }

    #[derive(Clone, Debug, PartialEq)]
    struct Payload(String);

    #[test]
    fn test_pub_sub_basic() {
        let bus = EventBus::new(10);

        let sub1 = bus.subscribe(Topic::System);
        let sub2 = bus.subscribe(Topic::System);
        let sub3 = bus.subscribe(Topic::User);

        assert_eq!(bus.publish(&Topic::System, Payload("Hello".into())), 2);
        assert_eq!(bus.publish(&Topic::User, Payload("World".into())), 1);

        assert_eq!(sub1.try_recv().unwrap(), Payload("Hello".into()));
        assert_eq!(sub2.try_recv().unwrap(), Payload("Hello".into()));
        assert_eq!(sub3.try_recv().unwrap(), Payload("World".into()));
    }

    #[test]
    fn test_auto_unsubscribe_on_drop() {
        let bus = EventBus::new(10);

        let sub1 = bus.subscribe(Topic::System);
        {
            let _sub2 = bus.subscribe(Topic::System);
            assert_eq!(bus.publish(&Topic::System, Payload("A".into())), 2);
        } // _sub2 drops here

        // Next publish should automatically prune _sub2 and only deliver to sub1
        assert_eq!(bus.publish(&Topic::System, Payload("B".into())), 1);

        assert_eq!(sub1.try_recv().unwrap(), Payload("A".into()));
        assert_eq!(sub1.try_recv().unwrap(), Payload("B".into()));

        // Verify pruning happened
        let map = bus.subscribers.read().unwrap();
        let subs = map.get(&Topic::System).unwrap();
        let list = subs.lock().unwrap();
        assert_eq!(list.len(), 1);
    }

    #[test]
    fn test_slow_consumer() {
        // Channel capacity 1
        let bus = EventBus::new(1);
        let sub1 = bus.subscribe(Topic::System);

        assert_eq!(bus.publish(&Topic::System, Payload("1".into())), 1);
        // This should fail to deliver to sub1 because queue is full (capacity 1)
        // Publisher should not block.
        assert_eq!(bus.publish(&Topic::System, Payload("2".into())), 0);

        // Sub1 only gets the first message
        assert_eq!(sub1.try_recv().unwrap(), Payload("1".into()));
        assert!(sub1.try_recv().is_err()); // Queue empty
    }

    #[test]
    fn test_multithreaded_publish() {
        let bus = Arc::new(EventBus::new(100));
        let sub = bus.subscribe(Topic::System);

        let mut handles = vec![];
        for i in 0..10 {
            let bus_clone = bus.clone();
            handles.push(thread::spawn(move || {
                bus_clone.publish(&Topic::System, Payload(i.to_string()));
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        // We should have received all 10 messages
        let mut count = 0;
        while sub.try_recv().is_ok() {
            count += 1;
        }
        assert_eq!(count, 10);
    }

    #[test]
    fn test_multithreaded_subscribe() {
        let bus = Arc::new(EventBus::new(10));

        let mut handles = vec![];
        for _ in 0..10 {
            let bus_clone = bus.clone();
            handles.push(thread::spawn(move || {
                let sub = bus_clone.subscribe(Topic::System);
                // Hold subscription for a moment
                thread::sleep(Duration::from_millis(10));
                // It drops when thread ends
                let _ = sub;
            }));
        }

        // While threads are subscribing/dropping, publish messages
        for _ in 0..10 {
            bus.publish(&Topic::System, Payload("Spam".into()));
            thread::sleep(Duration::from_millis(2));
        }

        for h in handles {
            h.join().unwrap();
        }

        // Force cleanup
        bus.clean_dead_subscriptions();

        let map = bus.subscribers.read().unwrap();
        let subs = map.get(&Topic::System).unwrap();
        let list = subs.lock().unwrap();
        assert_eq!(list.len(), 0);
    }
}
