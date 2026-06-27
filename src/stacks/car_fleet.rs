//! # 853. Car Fleet
//!
//! Difficulty: Medium
//! Link: https://leetcode.com/problems/car-fleet/
//!
//! Why this matters in Rust: This problem highlights Rust's explicit handling of
//! floating-point non-total ordering (`Ord` vs `PartialOrd`), and demonstrates
//! evaluating fleet formation using a straightforward monotonic stack versus an
//! O(1) space optimized iterator `fold` approach.
//!
//! ## Approach
//!
//! We start by pairing each car's position and speed, then sorting the cars by
//! their initial position in descending order (closest to target first).
//! We calculate the time it takes for each car to reach the target: `(target - position) / speed`.
//!
//! A car catches up to the car ahead of it (forming a fleet) if its arrival time
//! is less than or equal to the car ahead's arrival time.
//!
//! Two solutions are provided:
//! 1. `car_fleet_stack`: Uses a monotonic stack to keep track of the arrival times of fleets.
//! 2. `car_fleet_optimized`: Uses an iterator `fold` without allocating a stack,
//!    achieving O(1) extra space after the initial sorting allocation.
//!
//! Time Complexity: O(N log N) for sorting.
//! Space Complexity: O(N) to store the combined position and speed array. The optimized
//! solution uses O(1) additional space.
//!
//! ## Alternative Approaches
//!
//! * In C++ or Java, one might use an array of pairs and a standard sorting function.
//!   Rust's `f32` does not implement `Ord` (only `PartialOrd`) because of `NaN` values,
//!   which requires careful handling when comparing floating point times, highlighting
//!   Rust's strict safety around edge cases.

/// Straightforward stack approach
pub fn car_fleet_stack(target: i32, position: Vec<i32>, speed: Vec<i32>) -> i32 {
    let mut cars: Vec<(i32, i32)> = position.into_iter().zip(speed).collect();

    // Sort cars by position in descending order
    // RUST INSIGHT: We sort descending so we process the car closest to the target first.
    cars.sort_unstable_by(|a, b| b.0.cmp(&a.0));

    let mut stack: Vec<f32> = Vec::new();

    for (p, s) in cars {
        let time = (target - p) as f32 / s as f32;

        // If the stack has a car ahead of us (last in stack) and our time is <= their time,
        // we catch up and form a fleet. We don't push our time because the fleet's time
        // is determined by the slower car ahead.
        // GOTCHA: Floating point comparisons require care, but here valid inputs guarantee no NaNs.
        if stack.is_empty() || time > *stack.last().unwrap() {
            stack.push(time);
        }
    }

    stack.len() as i32
}

/// Optimized O(1) auxiliary space approach (after sorting)
pub fn car_fleet_optimized(target: i32, position: Vec<i32>, speed: Vec<i32>) -> i32 {
    let mut cars: Vec<(i32, i32)> = position.into_iter().zip(speed.into_iter()).collect();
    cars.sort_unstable_by(|a, b| b.0.cmp(&a.0));

    // RUST INSIGHT: Using `fold` allows us to maintain state (fleets count, max time)
    // cleanly without explicit mutable variables outside the loop.
    let (fleets, _) = cars
        .into_iter()
        .fold((0, f32::NEG_INFINITY), |(count, max_time), (p, s)| {
            let time = (target - p) as f32 / s as f32;
            if time > max_time {
                (count + 1, time)
            } else {
                (count, max_time)
            }
        });

    fleets
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_car_fleet_happy_path() {
        assert_eq!(
            car_fleet_stack(12, vec![10, 8, 0, 5, 3], vec![2, 4, 1, 1, 3]),
            3
        );
        assert_eq!(
            car_fleet_optimized(12, vec![10, 8, 0, 5, 3], vec![2, 4, 1, 1, 3]),
            3
        );
    }

    #[test]
    fn test_car_fleet_edge_cases() {
        // Only one car
        assert_eq!(car_fleet_stack(10, vec![3], vec![3]), 1);
        assert_eq!(car_fleet_optimized(10, vec![3], vec![3]), 1);

        // Empty case
        assert_eq!(car_fleet_stack(10, vec![], vec![]), 0);
        assert_eq!(car_fleet_optimized(10, vec![], vec![]), 0);
    }

    #[test]
    fn test_car_fleet_stress() {
        // Cars that arrive at exactly the same time, starting from different positions
        assert_eq!(car_fleet_stack(100, vec![0, 20, 40], vec![10, 8, 6]), 1);
        assert_eq!(car_fleet_optimized(100, vec![0, 20, 40], vec![10, 8, 6]), 1);

        // Already at target (time 0)
        assert_eq!(car_fleet_stack(10, vec![10, 8], vec![2, 2]), 2);
        assert_eq!(car_fleet_optimized(10, vec![10, 8], vec![2, 2]), 2);
    }
}
