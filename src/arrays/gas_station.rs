//! # 134. Gas Station
//!
//! There are `n` gas stations along a circular route, where the amount of gas at the `i`th station is `gas[i]`.
//! You have a car with an unlimited gas tank and it costs `cost[i]` of gas to travel from the `i`th station to its next `(i + 1)`th station.
//! You begin the journey with an empty tank at one of the gas stations.
//!
//! Given two integer arrays `gas` and `cost`, return the starting gas station's index if you can travel around the circuit once in the clockwise direction, otherwise return `-1`. If there exists a solution, it is guaranteed to be unique.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::arrays::gas_station::can_complete_circuit;
//!
//! let gas = vec![1, 2, 3, 4, 5];
//! let cost = vec![3, 4, 5, 1, 2];
//! assert_eq!(can_complete_circuit(gas, cost), 3);
//!
//! let gas = vec![2, 3, 4];
//! let cost = vec![3, 4, 3];
//! assert_eq!(can_complete_circuit(gas, cost), -1);
//! ```
//!
//! ## Constraints
//!
//! - `n == gas.length == cost.length`
//! - `1 <= n <= 10^5`
//! - `0 <= gas[i], cost[i] <= 10^4`
//!
//! ---
//!
//! **Why this matters in Rust:**
//! This problem perfectly demonstrates the power of zero-cost iterator combinators (`zip`, `fold`, `enumerate`) in Rust. It shows how we can accumulate state purely functionally without mutable external bindings or complex index math, turning a historically confusing two-pointer problem into a clean data pipeline.

/// Brute force approach: Simulate the journey from every starting station.
///
/// Time: O(N^2) - in the worst case, we simulate a nearly full loop from every station.
/// Space: O(1) - constant extra space used.
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
#[allow(clippy::cast_possible_truncation)]
#[allow(clippy::cast_possible_wrap)]
pub fn can_complete_circuit_brute_force(gas: Vec<i32>, cost: Vec<i32>) -> i32 {
    let n = gas.len();

    // RUST INSIGHT: We use `0..n` as a clean way to generate starting indices.
    for start in 0..n {
        let mut tank = 0;
        let mut can_complete = true;

        for step in 0..n {
            let current = (start + step) % n; // Wrap around the circular array
            tank += gas[current] - cost[current];

            if tank < 0 {
                can_complete = false;
                break; // We can't reach the next station
            }
        }

        if can_complete {
            return start as i32;
        }
    }

    -1
}

/// Optimized approach: Single pass imperative
///
/// **Algorithm Insight:**
/// 1. If the total gas is less than the total cost, completing the circuit is mathematically impossible.
/// 2. If it is possible, then there must be a valid starting point. If starting at `A` fails at `B`, then starting anywhere between `A` and `B` will also fail at or before `B`. Thus, our next valid start attempt must be `B + 1`.
///
/// Time: O(N) - single pass through the array.
/// Space: O(1) - only tracking integer counters.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
#[allow(clippy::cast_possible_truncation)]
#[allow(clippy::cast_possible_wrap)]
pub fn can_complete_circuit_optimized(gas: Vec<i32>, cost: Vec<i32>) -> i32 {
    let mut total_surplus = 0;
    let mut current_surplus = 0;
    let mut start_index = 0;

    for i in 0..gas.len() {
        let diff = gas[i] - cost[i];
        total_surplus += diff;
        current_surplus += diff;

        // If our current tank goes negative, we can't reach station i+1 from our current start.
        if current_surplus < 0 {
            // Reset start index to the next station
            start_index = i + 1;
            // Reset current tank
            current_surplus = 0;
        }
    }

    // GOTCHA: Don't forget to check if the total trip is even possible.
    if total_surplus >= 0 {
        start_index as i32
    } else {
        -1
    }
}

/// Optimal approach: Functional iterator pipeline
///
/// Uses standard Rust iterator traits to achieve the same O(N) time and O(1) space,
/// but demonstrates how to manage state entirely functionally using `fold`.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
#[allow(clippy::cast_possible_truncation)]
#[allow(clippy::cast_possible_wrap)]
pub fn can_complete_circuit_optimal(gas: Vec<i32>, cost: Vec<i32>) -> i32 {
    // RUST INSIGHT: `fold` takes an initial state tuple: (total_surplus, current_surplus, start_index).
    // The `enumerate` combinator gives us the index `i` along with the values.
    // The `zip` combines the gas and cost arrays into a single iterator without any allocation.
    let (total_surplus, _, start_index) = gas.iter().zip(cost.iter()).enumerate().fold(
        (0, 0, 0),
        |(total, curr, start), (i, (&g, &c))| {
            let diff = g - c;
            let next_total = total + diff;
            let next_curr = curr + diff;

            if next_curr < 0 {
                // If we run out of gas, next start point is i+1, and reset current tank.
                (next_total, 0, i + 1)
            } else {
                (next_total, next_curr, start)
            }
        },
    );

    if total_surplus >= 0 {
        start_index as i32
    } else {
        -1
    }
}

/// Main entry point - uses optimal solution
#[must_use]
pub fn can_complete_circuit(gas: Vec<i32>, cost: Vec<i32>) -> i32 {
    can_complete_circuit_optimal(gas, cost)
}

// =========================================================================================
// Alternative Approaches
// =========================================================================================
// 1. **Prefix Sum Minima**: Another O(N) approach involves tracking the prefix sum of (gas - cost). The starting index is the one immediately following the absolute minimum point of the prefix sum. This is mathematically elegant but slightly less intuitive to explain in an interview setting than the "reset when negative" logic.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brute_force_happy_path() {
        assert_eq!(
            can_complete_circuit_brute_force(vec![1, 2, 3, 4, 5], vec![3, 4, 5, 1, 2]),
            3
        );
    }

    #[test]
    fn test_brute_force_impossible() {
        assert_eq!(
            can_complete_circuit_brute_force(vec![2, 3, 4], vec![3, 4, 3]),
            -1
        );
    }

    #[test]
    fn test_optimized_happy_path() {
        assert_eq!(
            can_complete_circuit_optimized(vec![1, 2, 3, 4, 5], vec![3, 4, 5, 1, 2]),
            3
        );
    }

    #[test]
    fn test_optimized_impossible() {
        assert_eq!(
            can_complete_circuit_optimized(vec![2, 3, 4], vec![3, 4, 3]),
            -1
        );
    }

    #[test]
    fn test_optimal_happy_path() {
        assert_eq!(
            can_complete_circuit_optimal(vec![1, 2, 3, 4, 5], vec![3, 4, 5, 1, 2]),
            3
        );
    }

    #[test]
    fn test_optimal_impossible() {
        assert_eq!(
            can_complete_circuit_optimal(vec![2, 3, 4], vec![3, 4, 3]),
            -1
        );
    }

    #[test]
    fn test_all_approaches_single_element_possible() {
        let gas = vec![2];
        let cost = vec![2];
        assert_eq!(
            can_complete_circuit_brute_force(gas.clone(), cost.clone()),
            0
        );
        assert_eq!(can_complete_circuit_optimized(gas.clone(), cost.clone()), 0);
        assert_eq!(can_complete_circuit_optimal(gas, cost), 0);
    }

    #[test]
    fn test_all_approaches_single_element_impossible() {
        let gas = vec![1];
        let cost = vec![2];
        assert_eq!(
            can_complete_circuit_brute_force(gas.clone(), cost.clone()),
            -1
        );
        assert_eq!(
            can_complete_circuit_optimized(gas.clone(), cost.clone()),
            -1
        );
        assert_eq!(can_complete_circuit_optimal(gas, cost), -1);
    }
}
