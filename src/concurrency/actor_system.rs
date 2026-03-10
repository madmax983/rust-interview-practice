//! # Actor System Implementation
//!
//! A lightweight, thread-based Actor System from scratch.
//!
//! **Replaces Crates:** `actix`, `bastion`, `ractor`
//!
//! **Real-world Usage:**
//! - High-concurrency web servers (Actix-web is built on Actix).
//! - Distributed systems and clustering (Erlang/OTP model).
//! - Managing stateful connections (e.g., each WebSocket connection is an actor).
//!
//! **Why build it yourself?**
//! The Actor Model is a fundamental approach to concurrency that avoids shared mutable state (and thus locks).
//! Building one teaches you how to decouple state from execution using message passing (`mpsc` channels),
//! how to manage background threads that act as event loops, and how to use trait objects to dispatch
//! heterogeneous messages to a strongly-typed state machine.

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure:
//
//      [Sender (Addr)]  ─────►  [mpsc::Receiver]
//                                      │
//                                      ▼
//                           ┌────────────────────┐
//                           │ Thread (Event Loop)│
//                           │                    │
//                           │   ┌────────────┐   │
//                           │   │   Actor    │   │
//                           │   │  (State)   │   │
//                           │   └────────────┘   │
//                           └────────────────────┘
//
// Invariants:
// 1. An Actor's state is strictly isolated. It can only be mutated by the thread running its event loop.
// 2. Messages are processed sequentially in the order they are received (FIFO).
// 3. The Actor thread terminates gracefully when all external handles (`Addr`) are dropped.
//
// Complexity:
// ┌───────────────┬─────────────┬─────────────┐
// │ Operation     │ Time        │ Space       │
// ├───────────────┼─────────────┼─────────────┤
// │ Send Msg      │ O(1)        │ O(1)        │
// │ Process Msg   │ O(Handler)  │ O(1)        │
// └───────────────┴─────────────┴─────────────┘
//
// Design Decisions:
// - **Concurrency Model**: One OS thread per Actor.
//   - *Tradeoff*: Simple to implement, but doesn't scale to millions of actors due to OS thread overhead.
//   - *Alternative*: Production systems (like Actix) multiplex many actors over a small thread pool using `async`/`await`.
// - **Message Dispatch**: Boxed trait objects (`Box<dyn Envelope>`) over a single channel.
//   - *Tradeoff*: Requires an allocation per message.
//   - *Alternative*: Typed channels per message type (complex routing) or `Any` downcasting.

use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

/// A marker trait for messages that an Actor can receive.
/// Must be Send to cross thread boundaries, and 'static because they are processed asynchronously.
pub trait Message: Send + 'static {
    /// The type returned when this message is handled.
    type Result: Send + 'static;
}

/// Context trait for actors to control their lifecycle.
pub trait ActorContext {
    /// Stop the actor.
    fn stop(&mut self);
}

/// A concrete implementation of ActorContext.
pub struct ContextImpl {
    running: bool,
}

impl ActorContext for ContextImpl {
    fn stop(&mut self) {
        self.running = false;
    }
}

/// A stateful component that runs in its own thread and processes messages.
pub trait Actor: Send + Sized + 'static {
    /// Context passed to the actor upon startup or message handling.
    /// Can be used to stop the actor, schedule future messages, etc.
    type Context: ActorContext;

    /// Called before the actor starts processing messages.
    fn started(&mut self, _ctx: &mut Self::Context) {}

    /// Called after the actor stops processing messages (e.g., when the channel closes).
    fn stopped(&mut self, _ctx: &mut Self::Context) {}

    /// Creates the context for the actor.
    fn create_context() -> Self::Context;

    /// Checks if the context indicates the actor should keep running.
    fn is_running(ctx: &Self::Context) -> bool;

    /// Spawns the actor on a new background thread, returning its Address.
    fn start(mut self) -> Addr<Self> {
        let (tx, rx) = mpsc::channel();
        let addr = Addr { sender: tx };

        // PRODUCTION NOTE: In a real system like actix, actors don't necessarily get their
        // own dedicated OS thread. Instead, they are multiplexed via `async`/`await` on a
        // fixed-size thread pool (like Tokio's) to scale to millions of actors.
        thread::spawn(move || {
            let mut ctx = Self::create_context();
            self.started(&mut ctx);

            while Self::is_running(&ctx) {
                // GOTCHA: `rx.recv()` blocks the thread. If the actor needs to handle timeouts,
                // you would need to use `recv_timeout` and incorporate timer management into the context.
                match rx.recv() {
                    Ok(mut envelope) => {
                        // RUST INSIGHT: `Envelope` hides the specific message type from the channel,
                        // allowing us to send different message types down the same mpsc::Sender
                        // using dynamic dispatch (trait objects).
                        envelope.handle(&mut self, &mut ctx);
                    }
                    Err(_) => {
                        // Sender side dropped. We should stop.
                        ctx.stop();
                    }
                }
            }

            self.stopped(&mut ctx);
        });

        addr
    }
}

/// A trait defining how an Actor handles a specific Message type.
pub trait Handler<M: Message>: Actor {
    /// Process the message and return a Result.
    fn handle(&mut self, msg: M, ctx: &mut Self::Context) -> M::Result;
}

// Internal trait to allow boxing of messages. We can't use `Box<dyn Message>` directly
// because the channel needs to know how to call the right `handle` method on the Actor.
// We use a pattern called "Message Envelope" (or type erasure).
trait Envelope<A: Actor>: Send {
    fn handle(&mut self, actor: &mut A, ctx: &mut A::Context);
}

// A concrete envelope that holds the specific message and (optionally) a channel to send the reply.
struct MessageEnvelope<M: Message> {
    msg: Option<M>, // Option allows taking ownership in `handle`
    reply_tx: Option<Sender<M::Result>>,
}

impl<A, M> Envelope<A> for MessageEnvelope<M>
where
    A: Actor + Handler<M>,
    M: Message,
{
    fn handle(&mut self, actor: &mut A, ctx: &mut A::Context) {
        if let Some(msg) = self.msg.take() {
            let result = actor.handle(msg, ctx);
            if let Some(tx) = self.reply_tx.take() {
                // Ignore send errors; the caller might have dropped the receiver (fire-and-forget).
                let _ = tx.send(result);
            }
        }
    }
}

/// A reference/handle to a running Actor.
/// Used to send messages to the Actor.
pub struct Addr<A: Actor> {
    sender: Sender<Box<dyn Envelope<A>>>,
}

impl<A: Actor> Clone for Addr<A> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
        }
    }
}

// SendError type analogous to mpsc::SendError
#[derive(Debug, PartialEq, Eq)]
pub enum SendError {
    Closed,
}

impl<A: Actor> Addr<A> {
    /// Sends a message to the actor asynchronously (fire-and-forget).
    pub fn do_send<M>(&self, msg: M)
    where
        M: Message,
        A: Handler<M>,
    {
        let envelope: Box<dyn Envelope<A>> = Box::new(MessageEnvelope {
            msg: Some(msg),
            reply_tx: None,
        });

        // If the actor is stopped (channel closed), the message is dropped.
        let _ = self.sender.send(envelope);
    }

    /// Sends a message and returns a Receiver that will yield the result.
    /// In a fully async system (like Actix), this returns a Future.
    pub fn send<M>(&self, msg: M) -> Result<Receiver<M::Result>, SendError>
    where
        M: Message,
        A: Handler<M>,
    {
        let (tx, rx) = mpsc::channel();
        let envelope: Box<dyn Envelope<A>> = Box::new(MessageEnvelope {
            msg: Some(msg),
            reply_tx: Some(tx),
        });

        match self.sender.send(envelope) {
            Ok(_) => Ok(rx),
            Err(_) => Err(SendError::Closed),
        }
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `actix`: Highly optimized, async-first, uses a runtime (tokio) to multiplex actors, supports actor supervision and clustering.
// - `bastion`: Focuses on fault-tolerance and supervisor trees (Erlang style).
//
// Missing vs. Production:
// - **Async/Await**: This implementation uses blocking OS threads.
// - **Supervision**: No automatic restarts if an actor panics.
// - **Clustering**: No distributed messaging between nodes.
//
// Next Steps:
// 1. Implement a `Supervisor` to restart actors on panic.
// 2. Port to an async runtime for M:N threading.
//
// Benchmarking Note:
// To benchmark this implementation, create a loop sending thousands of messages (e.g. `Increment`)
// to an actor using `std::time::Instant` to measure total duration, ensuring `black_box` isn't
// needed as message processing forces actual execution within the actor's thread. Compare
// single-threaded vs multi-threaded sender configurations using `criterion`.

#[cfg(test)]
mod tests {
    use super::*;

    struct CounterActor {
        count: i32,
    }

    impl Actor for CounterActor {
        type Context = ContextImpl;

        fn started(&mut self, _ctx: &mut Self::Context) {
            println!("Counter started");
        }

        fn stopped(&mut self, _ctx: &mut Self::Context) {
            println!("Counter stopped with value: {}", self.count);
        }

        fn create_context() -> Self::Context {
            ContextImpl { running: true }
        }

        fn is_running(ctx: &Self::Context) -> bool {
            ctx.running
        }
    }

    struct Increment(i32);
    impl Message for Increment {
        type Result = ();
    }

    impl Handler<Increment> for CounterActor {
        fn handle(
            &mut self,
            msg: Increment,
            _ctx: &mut Self::Context,
        ) -> <Increment as Message>::Result {
            self.count += msg.0;
        }
    }

    struct GetCount;
    impl Message for GetCount {
        type Result = i32;
    }

    impl Handler<GetCount> for CounterActor {
        fn handle(
            &mut self,
            _msg: GetCount,
            _ctx: &mut Self::Context,
        ) -> <GetCount as Message>::Result {
            self.count
        }
    }

    struct StopActor;
    impl Message for StopActor {
        type Result = ();
    }

    impl Handler<StopActor> for CounterActor {
        fn handle(
            &mut self,
            _msg: StopActor,
            ctx: &mut Self::Context,
        ) -> <StopActor as Message>::Result {
            ctx.stop();
        }
    }

    #[test]
    fn test_actor_basic_messaging() {
        let actor = CounterActor { count: 0 };
        let addr = actor.start();

        // Send messages (fire and forget)
        addr.do_send(Increment(5));
        addr.do_send(Increment(10));

        // Get result
        let rx = addr.send(GetCount).unwrap();
        let result = rx.recv().unwrap();

        assert_eq!(result, 15);
    }

    #[test]
    fn test_actor_stop() {
        let actor = CounterActor { count: 0 };
        let addr = actor.start();

        addr.do_send(Increment(1));
        addr.do_send(StopActor);

        // This message should be dropped because the actor is stopped
        // (or rather, it might be sent but not processed depending on race condition)
        // Wait for actor to process StopActor
        thread::sleep(std::time::Duration::from_millis(50));

        // Attempting to send a message requiring a reply will fail if the channel is closed
        let res = addr.send(GetCount);
        assert_eq!(res.is_err(), true);
        if let Err(e) = res {
            assert_eq!(e, SendError::Closed);
        }
    }

    #[test]
    fn test_concurrent_senders() {
        let actor = CounterActor { count: 0 };
        let addr = actor.start();

        let mut handles = vec![];

        for _ in 0..10 {
            let addr_clone = addr.clone();
            handles.push(thread::spawn(move || {
                for _ in 0..100 {
                    addr_clone.do_send(Increment(1));
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        let rx = addr.send(GetCount).unwrap();
        assert_eq!(rx.recv().unwrap(), 1000);
    }
}
