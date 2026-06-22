//! # 853. Car Fleet
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/car-fleet/>
//!
//! There are `n` cars going to the same destination along a one-lane road. The destination is `target` miles away.
//! You are given two integer array `position` and `speed`, both of length `n`.
//!
//! A car can never pass another car ahead of it, but it can catch up to it and drive bumper to bumper at the same speed.
//! The faster car will slow down to match the slower car's speed. The distance between these two cars is ignored.
//! A car fleet is some non-empty set of cars driving at the same position and same speed.
//!
//! Return the number of car fleets that will arrive at the destination.
//!
//! This problem naturally fits a monotonic stack or greedy fold approach. In Rust, it provides an excellent
//! opportunity to practice tuple sorting, functional iterator pipelines, and handling floating point comparisons
//! which don't implement the `Ord` trait (only `PartialOrd`).
//!
//! ## Examples
//!
//! ```
//! // Omitted for brevity, see tests.
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

/// Stack-based approach.
///
/// Time: O(N log N) dominated by sorting.
/// Space: O(N) for the cars array and the stack.
///
/// RUST INSIGHT: We map the inputs into an array of `(position, time_to_target)` tuples.
/// The `time_to_target` is a `f64`. We sort by position in reverse order, and then use
/// a monotonic stack to keep track of the fleets. If a car's time to reach the target is
/// less than or equal to the car ahead of it (which is at the top of the stack), it will
/// join that fleet, so we don't push it to the stack.
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature uses owned Vec
pub fn car_fleet_stack(target: i32, position: Vec<i32>, speed: Vec<i32>) -> i32 {
    let mut cars: Vec<(i32, f64)> = position
        .into_iter()
        .zip(speed)
        .map(|(p, s)| (p, f64::from(target - p) / f64::from(s)))
        .collect();

    // Sort cars by position in descending order (closest to target first)
    // RUST INSIGHT: `f64` doesn't implement `Ord`, only `PartialOrd`, due to NaN.
    // Sorting by `i32` position is perfectly fine and safe.
    cars.sort_unstable_by(|a, b| b.0.cmp(&a.0));

    let mut stack: Vec<f64> = Vec::with_capacity(cars.len());

    for (_, time) in cars {
        if stack.is_empty() || time > *stack.last().unwrap() {
            stack.push(time);
        }
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    {
        stack.len() as i32
    }
}

/// Optimized iterator approach using `fold`.
///
/// Time: O(N log N) dominated by sorting.
/// Space: O(N) for the cars array. Stack space is optimized to O(1).
///
/// RUST INSIGHT: We don't actually need to store the entire stack. We only care about
/// the total count of fleets and the arrival time of the slowest fleet currently leading.
/// This perfectly matches the `fold` pattern on iterators.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn car_fleet_optimized(target: i32, position: Vec<i32>, speed: Vec<i32>) -> i32 {
    let mut cars: Vec<(i32, f64)> = position
        .into_iter()
        .zip(speed)
        .map(|(p, s)| (p, f64::from(target - p) / f64::from(s)))
        .collect();

    cars.sort_unstable_by(|a, b| b.0.cmp(&a.0));

    cars.into_iter()
        .fold((0, 0.0), |(fleets, max_time), (_, time)| {
            if time > max_time {
                // A new fleet forms because this car takes longer than the fleet ahead
                (fleets + 1, time)
            } else {
                // This car catches up to the fleet ahead
                (fleets, max_time)
            }
        })
        .0
}

/// Main entry point
#[must_use]
pub fn car_fleet(target: i32, position: Vec<i32>, speed: Vec<i32>) -> i32 {
    car_fleet_optimized(target, position, speed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stack_happy_path() {
        let target = 12;
        let position = vec![10, 8, 0, 5, 3];
        let speed = vec![2, 4, 1, 1, 3];
        assert_eq!(car_fleet_stack(target, position, speed), 3);
    }

    #[test]
    fn test_optimized_happy_path() {
        let target = 12;
        let position = vec![10, 8, 0, 5, 3];
        let speed = vec![2, 4, 1, 1, 3];
        assert_eq!(car_fleet_optimized(target, position, speed), 3);
    }

    #[test]
    fn test_edge_cases() {
        // Single car
        assert_eq!(car_fleet(10, vec![3], vec![3]), 1);

        // Empty (not strictly in constraints, but good to handle if possible,
        // though `car_fleet` handles it naturally)
        assert_eq!(car_fleet(10, vec![], vec![]), 0);

        // Two cars matching speed and catching up perfectly
        assert_eq!(car_fleet(10, vec![0, 4], vec![2, 1]), 1);
    }

    #[test]
    fn test_stress() {
        // Many cars at same pos/speed logic is excluded by unique position constraint,
        // but we can test increasing speed from back to front.
        let target = 100;
        let position = vec![0, 20, 40, 60, 80];
        let speed = vec![10, 8, 6, 4, 2];
        // times: 10, 10, 10, 10, 10
        // They all catch up to the slowest one at the front
        assert_eq!(car_fleet(target, position, speed), 1);
    }
}
