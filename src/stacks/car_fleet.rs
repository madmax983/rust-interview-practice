//! # 853. Car Fleet
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/car-fleet/>
//!
//! There are `n` cars going to the same destination along a one-lane road.
//! The destination is `target` miles away.
//! You are given two integer arrays `position` and `speed`, both of length `n`,
//! where `position[i]` is the position of the `ith` car and `speed[i]` is the
//! speed of the `ith` car (in miles per hour).
//!
//! A car can never pass another car ahead of it, but it can catch up to it
//! and drive bumper to bumper at the same speed. The faster car will slow down
//! to match the slower car's speed. The distance between these two cars is ignored.
//! A car fleet is some non-empty set of cars driving at the same position and same speed.
//! Return the number of car fleets that will arrive at the destination.
//!
//! This problem emphasizes the evaluation of fleet formation. A key aspect in Rust
//! is handling floating-point arithmetic explicitly. Since fleets form when a car
//! catches up to another (i.e., takes less or equal time to reach the target), we compute
//! time `(target - pos) / speed`. Rust forces us to be explicit when working with floats
//! because they do not implement `Ord` (only `PartialOrd`) due to `NaN` values.
//! We can either use f64 and handle the logic carefully or stick to integer arithmetic/iterators.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::stacks::car_fleet::car_fleet;
//!
//! let target = 12;
//! let position = vec![10, 8, 0, 5, 3];
//! let speed = vec![2, 4, 1, 1, 3];
//! assert_eq!(car_fleet(target, position, speed), 3);
//! ```
//!
//! ## Constraints
//!
//! - `n == position.length == speed.length`
//! - `1 <= n <= 10^5`
//! - `0 < target <= 10^6`
//! - `0 <= position[i] < target`
//! - All the values of `position` are unique.
//! - `0 < speed[i] <= 10^6`

/// Brute Force / Stack Approach
/// Time: O(N log N) - due to sorting
/// Space: O(N) - storing elements in a vector and a stack
///
/// We zip position and speed, sort by position in descending order (closest to target first),
/// and then use a stack to keep track of fleet arrival times. If a car's time is <= the
/// time of the car at the top of the stack, it joins that fleet. Otherwise, it forms a new fleet.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn car_fleet_brute_force(target: i32, position: Vec<i32>, speed: Vec<i32>) -> i32 {
    let mut cars: Vec<(i32, f64)> = position
        .into_iter()
        .zip(speed)
        .map(|(p, s)| (p, f64::from(target - p) / f64::from(s)))
        .collect();

    // Sort by position descending
    // GOTCHA: We sort by the first element of the tuple (position) which is an integer (i32),
    // thus avoiding the `PartialOrd` issues of sorting by floats.
    cars.sort_unstable_by(|a, b| b.0.cmp(&a.0));

    let mut stack: Vec<f64> = Vec::new();

    for (_, time) in cars {
        if let Some(&top_time) = stack.last() {
            if time > top_time {
                stack.push(time);
            }
        } else {
            stack.push(time);
        }
    }

    stack.len() as i32
}

/// Optimized Approach: Iterator `fold` (O(1) Space)
/// Time: O(N log N) - sorting dominates
/// Space: O(N) - for the zipped vector, but O(1) auxiliary space beyond that
///
/// Instead of maintaining a full stack, we only need to keep track of the longest time
/// we have seen so far (the slowest fleet leader) and count how many times a new fleet
/// is formed. We can accomplish this elegantly using Rust's iterator `fold`.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn car_fleet_optimized(target: i32, position: Vec<i32>, speed: Vec<i32>) -> i32 {
    let mut cars: Vec<(i32, f64)> = position
        .into_iter()
        .zip(speed)
        .map(|(p, s)| (p, f64::from(target - p) / f64::from(s)))
        .collect();

    // Sort descending by position
    cars.sort_unstable_by(|a, b| b.0.cmp(&a.0));

    // RUST INSIGHT: We use `fold` to accumulate state `(count, max_time)`.
    // This entirely eliminates the need for an explicit Stack (`Vec`) structure.
    let (fleets, _) = cars
        .into_iter()
        .fold((0, 0.0), |(count, max_time), (_, time)| {
            if time > max_time {
                (count + 1, time) // New fleet formed
            } else {
                (count, max_time) // Joins existing fleet
            }
        });

    fleets
}

/// Main entry point - uses optimal solution
#[must_use]
pub fn car_fleet(target: i32, position: Vec<i32>, speed: Vec<i32>) -> i32 {
    car_fleet_optimized(target, position, speed)
}

// Alternative Approaches:
// 1. **Integer Arithmetic without Floats**: To avoid floating point issues entirely,
//    instead of calculating `time = distance / speed`, one can compare
//    `time1 <= time2` by cross-multiplying: `distance1 * speed2 <= distance2 * speed1`.
//    This requires careful handling of integer overflow (using `i64`).

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brute_force_example_1() {
        let target = 12;
        let position = vec![10, 8, 0, 5, 3];
        let speed = vec![2, 4, 1, 1, 3];
        assert_eq!(car_fleet_brute_force(target, position, speed), 3);
    }

    #[test]
    fn test_optimized_example_1() {
        let target = 12;
        let position = vec![10, 8, 0, 5, 3];
        let speed = vec![2, 4, 1, 1, 3];
        assert_eq!(car_fleet_optimized(target, position, speed), 3);
    }

    #[test]
    fn test_example_2() {
        let target = 10;
        let position = vec![3];
        let speed = vec![3];
        assert_eq!(car_fleet(target, position, speed), 1);
    }

    #[test]
    fn test_example_3() {
        let target = 100;
        let position = vec![0, 2, 4];
        let speed = vec![4, 2, 1];
        // Cars starting at 0, 2, 4 with speeds 4, 2, 1.
        // Times:
        // pos=4, sp=1 -> time = 96 / 1 = 96
        // pos=2, sp=2 -> time = 98 / 2 = 49 (catches up to 4? Yes, 49 < 96. Joins fleet)
        // pos=0, sp=4 -> time = 100 / 4 = 25 (catches up to 2? Yes, 25 < 49. Joins fleet)
        // All form 1 fleet.
        assert_eq!(car_fleet(target, position, speed), 1);
    }

    #[test]
    fn test_empty() {
        assert_eq!(car_fleet(10, vec![], vec![]), 0);
    }

    #[test]
    fn test_same_position_different_speeds() {
        // According to constraints, all positions are unique.
        // But if they were:
        // A car fleet could theoretically start at the same position.
        // This stress test checks float accuracy handling
        let target = 10;
        let position = vec![0, 4, 2];
        let speed = vec![2, 1, 3];
        // pos=4, sp=1 -> 6
        // pos=2, sp=3 -> 8/3 = 2.66 -> joins fleet
        // pos=0, sp=2 -> 5 -> joins fleet
        assert_eq!(car_fleet(target, position, speed), 1);
    }
}
