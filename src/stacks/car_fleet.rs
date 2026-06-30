//! # 853. Car Fleet
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/car-fleet/>
//!
//! There are `n` cars at given miles away from the starting mile 0, traveling to reach the mile `target`.
//! You are given two integer arrays `position` and `speed`, both of length `n`, where `position[i]` is the starting
//! mile of the `ith` car and `speed[i]` is the speed of the `ith` car in miles per hour.
//!
//! A car cannot pass another car ahead of it. It can only catch up to it and travel at the same speed.
//! Return the number of car fleets that will arrive at the destination.
//!
//! This problem provides a great exercise in processing sorted data with iterators and handling floating-point
//! comparisons in Rust. It highlights how we can track state either using an explicit monotonic stack or
//! more idiomatically with an iterator `fold` combining state management in O(1) space.
//!
//! ## Approaches
//!
//! 1.  **Monotonic Stack**:
//!     -   Sort the cars by position in descending order (closest to target first).
//!     -   Calculate the time to reach the target for each car.
//!     -   Maintain a stack of arrival times. If a car's time is `<= ` the top of the stack, it joins the fleet ahead of it.
//!     -   Time: O(N log N) for sorting, O(N) for processing.
//!     -   Space: O(N) for the stack and paired array.
//! 2.  **Iterator Fold (O(1) Auxiliary Space)**:
//!     -   Instead of a stack, we only need to track the slowest time (the fleet leader) we've seen so far
//!         while iterating from closest to furthest.
//!     -   Time: O(N log N) for sorting.
//!     -   Space: O(N) for paired array (to allow sorting), O(1) auxiliary space for counting.

// =========================================================================================
// Approach 1: Monotonic Stack
// =========================================================================================

/// Time: O(N log N)
/// Space: O(N)
#[must_use]
pub fn car_fleet_stack(target: i32, position: Vec<i32>, speed: Vec<i32>) -> i32 {
    let n = position.len();
    if n == 0 {
        return 0;
    }

    // Pair positions with speeds
    let mut cars: Vec<(i32, i32)> = position.into_iter().zip(speed).collect();

    // Sort cars by position descending (closest to target first)
    // RUST INSIGHT: Sorting in descending order allows us to process the leader first.
    cars.sort_unstable_by(|a, b| b.0.cmp(&a.0));

    let mut stack: Vec<f64> = Vec::with_capacity(n);

    for (p, s) in cars {
        let time = f64::from(target - p) / f64::from(s);

        // If stack is empty or this car takes strictly longer than the fleet ahead of it
        // (meaning it cannot catch up and forms a new fleet)
        if stack.is_empty() || time > *stack.last().unwrap() {
            stack.push(time);
        }
        // Otherwise, it catches up and we don't push it (it merges into the existing fleet)
    }

    // The number of fleets is the number of elements in the stack
    stack.len() as i32
}

// =========================================================================================
// Approach 2: Iterator Fold (Optimal Auxiliary Space)
// =========================================================================================

/// Time: O(N log N)
/// Space: O(N) for sorting vector, O(1) auxiliary state
#[must_use]
pub fn car_fleet_optimal(target: i32, position: Vec<i32>, speed: Vec<i32>) -> i32 {
    let n = position.len();
    if n == 0 {
        return 0;
    }

    let mut cars: Vec<(i32, i32)> = position.into_iter().zip(speed).collect();
    // Sort ascending, then we will iterate backwards
    cars.sort_unstable_by_key(|&(p, _)| p);

    // RUST INSIGHT: `fold` allows us to cleanly carry state (fleet_count, max_time)
    // through the iterator chain without mutating external variables.
    let (fleets, _) = cars.into_iter().rev().fold(
        (0, 0.0_f64),
        |(fleet_count, max_time), (p, s)| {
            let time = f64::from(target - p) / f64::from(s);

            // GOTCHA: Floating point comparisons can be tricky, but here strict greater-than
            // correctly identifies a car that cannot catch the fleet ahead.
            if time > max_time {
                // New fleet formed
                (fleet_count + 1, time)
            } else {
                // Caught up with the fleet ahead
                (fleet_count, max_time)
            }
        },
    );

    fleets
}

/// Main entry point - uses optimal solution
#[must_use]
pub fn car_fleet(target: i32, position: Vec<i32>, speed: Vec<i32>) -> i32 {
    car_fleet_optimal(target, position, speed)
}

// =========================================================================================
// Tests
// =========================================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_happy_path() {
        let target = 12;
        let position = vec![10, 8, 0, 5, 3];
        let speed = vec![2, 4, 1, 1, 3];

        assert_eq!(car_fleet_stack(target, position.clone(), speed.clone()), 3);
        assert_eq!(car_fleet_optimal(target, position.clone(), speed.clone()), 3);
        assert_eq!(car_fleet(target, position, speed), 3);
    }

    #[test]
    fn test_edge_case_single_car() {
        let target = 10;
        let position = vec![3];
        let speed = vec![3];

        assert_eq!(car_fleet_stack(target, position.clone(), speed.clone()), 1);
        assert_eq!(car_fleet_optimal(target, position.clone(), speed.clone()), 1);
        assert_eq!(car_fleet(target, position, speed), 1);
    }

    #[test]
    fn test_edge_case_empty() {
        let target = 10;
        let position = vec![];
        let speed = vec![];

        assert_eq!(car_fleet_stack(target, position.clone(), speed.clone()), 0);
        assert_eq!(car_fleet_optimal(target, position.clone(), speed.clone()), 0);
        assert_eq!(car_fleet(target, position, speed), 0);
    }

    #[test]
    fn test_stress_case_same_position() {
        // Technically the problem states position[i] < target and unique, but let's test robust ordering
        let target = 100;
        let position = vec![0, 2, 4];
        let speed = vec![4, 2, 1];
        // times to reach 100:
        // pos 4: (100-4)/1 = 96
        // pos 2: (100-2)/2 = 49 -> catches up to 4? No, 49 < 96, so it catches up BEFORE target.
        // pos 0: (100-0)/4 = 25 -> catches up to 2? 25 < 49, so it catches up BEFORE target.
        // Since they all catch up to pos 4's fleet, total fleets = 1.

        assert_eq!(car_fleet_stack(target, position.clone(), speed.clone()), 1);
        assert_eq!(car_fleet_optimal(target, position.clone(), speed.clone()), 1);
    }
}
