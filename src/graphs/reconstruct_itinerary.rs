//! # 332. Reconstruct Itinerary
//!
//! Link: <https://leetcode.com/problems/reconstruct-itinerary/>
//!
//! You are given a list of airline `tickets` where `tickets[i] = [from_i, to_i]` represent the departure and the arrival airports of one flight.
//! Reconstruct the itinerary in order and return it.
//!
//! All of the tickets belong to a man who departs from "JFK", thus, the itinerary must begin with "JFK".
//! If there are multiple valid itineraries, you should return the itinerary that has the smallest lexical order when read as a single string.
//! You may assume all tickets form at least one valid itinerary. You must use all the tickets once and only once.
//!
//! ## Why this matters in Rust
//! This problem is an excellent study in:
//! - **Graph Mutation during Traversal**: Removing edges while traversing (Eulerian path) is tricky with Rust's ownership rules.
//! - **Recursion vs Iteration**: Choosing between recursive DFS (cleaner) and iterative DFS (stack-safe).
//! - **Sorting & Borrowing**: Handling sorting of adjacency lists to meet lexical requirements.
//!
//! ## Rust Insight: Mutation inside Recursion
//! In many languages, you'd pass a mutable map to the recursive function. In Rust, this works too, but you must be careful not to hold
//! an immutable reference (like an iterator) while trying to mutate the map.
//!
//! ## Approaches
//!
//! ### Approach 1: Hierholzer's Algorithm (DFS)
//! The problem asks for an Eulerian Path (or Circuit) in a directed multigraph.
//! 1. Build an adjacency list: `HashMap<String, Vec<String>>`.
//! 2. Sort destination lists in *reverse* lexical order so we can efficiently `pop()` the smallest lexical element from the end (O(1)).
//! 3. Start DFS from "JFK".
//! 4. In DFS(u):
//!    - While `u` has outgoing edges, `pop` the edge `u -> v` and recursively call DFS(v).
//!    - After the loop (when `u` has no more unused outgoing edges), push `u` to the result list.
//! 5. The result list will be the path in reverse order. Reverse it to get the itinerary.
//!
//! Complexity:
//! - Time: O(E log E) due to sorting edges. DFS itself is O(E).
//! - Space: O(E) for adjacency list and recursion stack.

use std::collections::HashMap;

/// Finds the itinerary with the smallest lexical order.
///
/// Note: The problem guarantees a valid itinerary exists starting from "JFK".
#[must_use]
pub fn find_itinerary(tickets: Vec<Vec<String>>) -> Vec<String> {
    // Build adjacency list
    // Map: From -> List of To
    // We use a Vec and sort it in reverse order so we can pop from the back (O(1))
    // which corresponds to taking the smallest lexical element first.
    let mut adj: HashMap<String, Vec<String>> = HashMap::new();

    for ticket in tickets {
        // RUST INSIGHT: `entry` API
        // Efficiently handles the "insert if not present, then modify" pattern.
        // We consume the input `tickets` to avoid unnecessary cloning.
        let mut iter = ticket.into_iter();
        let from = iter.next().expect("Ticket must have a departure");
        let to = iter.next().expect("Ticket must have a destination");

        adj.entry(from).or_default().push(to);
    }

    // Sort neighbors in reverse lexical order
    for neighbors in adj.values_mut() {
        neighbors.sort_unstable_by(|a, b| b.cmp(a));
    }

    let mut route = Vec::new();

    // We use a helper function to perform the DFS.
    // We pass the adjacency list as mutable so we can consume edges.
    dfs("JFK", &mut adj, &mut route);

    // The route is constructed in reverse order (post-order traversal)
    route.reverse();
    route
}

fn dfs(u: &str, adj: &mut HashMap<String, Vec<String>>, route: &mut Vec<String>) {
    // RUST INSIGHT: While-let loop with mutation
    // We need to keep popping edges from u's neighbor list until empty.
    // We cannot iterate over `adj.get(u)` directly because we need to mutate `adj` inside the loop (recursive call).
    // Instead, we peer into the map, pop a value if it exists, and then recurse.

    // GOTCHA: We cannot hold a reference to `adj` (like `adj.get_mut(u)`) across the recursive call `dfs`.
    // The borrow checker would complain that we have a mutable borrow of `adj` (the entry)
    // and then try to borrow `adj` again in the recursive call.
    // FIX: We only access `adj` momentarily to pop, then release the borrow before recursing.

    loop {
        // Attempt to pop the next destination.
        // We use a scoped block or just a simple statement to ensure the mutable borrow of `adj` ends immediately.
        let next_dest = if let Some(neighbors) = adj.get_mut(u) {
            neighbors.pop()
        } else {
            None
        };

        if let Some(v) = next_dest {
            dfs(&v, adj, route);
        } else {
            // No more outgoing edges from u (or u not in map)
            break;
        }
    }

    // Add to route *after* visiting all children (Post-order)
    route.push(u.to_string());
}

// =========================================================================================
// Alternative Approaches
// =========================================================================================
//
// 1. Iterative DFS with Stack:
//    Instead of recursion, use an explicit stack. This avoids stack overflow issues for very
//    large graphs (though not a concern for the constraint of 300 edges).
//
// 2. BinaryHeap (Min-Heap):
//    Use `HashMap<String, BinaryHeap<Reverse<String>>>` to always get the lexically smallest
//    neighbor. Useful if edges are added dynamically. For a static graph, sorting once (as done here)
//    is generally more efficient.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_itinerary() {
        let tickets = vec![
            vec!["MUC".to_string(), "LHR".to_string()],
            vec!["JFK".to_string(), "MUC".to_string()],
            vec!["SFO".to_string(), "SJC".to_string()],
            vec!["LHR".to_string(), "SFO".to_string()],
        ];
        let expected = vec![
            "JFK".to_string(),
            "MUC".to_string(),
            "LHR".to_string(),
            "SFO".to_string(),
            "SJC".to_string(),
        ];
        assert_eq!(find_itinerary(tickets), expected);
    }

    #[test]
    fn test_lexical_order_tie() {
        // JFK -> SFO
        // JFK -> ATL
        // SFO -> ATL
        // ATL -> JFK
        // ATL -> SFO
        // Must choose ATL before SFO from JFK.
        // Route: JFK -> ATL -> JFK -> SFO -> ATL -> SFO
        let tickets = vec![
            vec!["JFK".to_string(), "SFO".to_string()],
            vec!["JFK".to_string(), "ATL".to_string()],
            vec!["SFO".to_string(), "ATL".to_string()],
            vec!["ATL".to_string(), "JFK".to_string()],
            vec!["ATL".to_string(), "SFO".to_string()],
        ];
        let expected = vec![
            "JFK".to_string(),
            "ATL".to_string(),
            "JFK".to_string(),
            "SFO".to_string(),
            "ATL".to_string(),
            "SFO".to_string(),
        ];
        assert_eq!(find_itinerary(tickets), expected);
    }

    #[test]
    fn test_cycle() {
        // JFK -> KUL -> JFK
        let tickets = vec![
            vec!["JFK".to_string(), "KUL".to_string()],
            vec!["KUL".to_string(), "JFK".to_string()],
        ];
        let expected = vec!["JFK".to_string(), "KUL".to_string(), "JFK".to_string()];
        assert_eq!(find_itinerary(tickets), expected);
    }

    #[test]
    fn test_single_trip() {
        let tickets = vec![vec!["JFK".to_string(), "LGA".to_string()]];
        let expected = vec!["JFK".to_string(), "LGA".to_string()];
        assert_eq!(find_itinerary(tickets), expected);
    }
}
