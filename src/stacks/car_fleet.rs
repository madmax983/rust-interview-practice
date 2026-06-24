//! # 853. Car Fleet
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/car-fleet/>
//!
//! There are `n` cars going to the same destination along a one-lane road. The destination is `target` miles away.
//! You are given two integer arrays `position` and `speed`, both of length `n`.
//! A car fleet is some non-empty set of cars driving at the same position and same speed. Note that a single car is also a car fleet.
//! If a car catches up to a car fleet right at the destination point, it will still be considered as one car fleet.
//! Return the number of car fleets that will arrive at the destination.
//!
//! This problem demonstrates sorting by multiple keys and comparing non-integer mathematical boundaries. It highlights
//! the subtle difference between `Ord` and `PartialOrd` in Rust, particularly when dealing with floating point calculations,
//! and provides an excellent opportunity to contrast a straightforward monotonic stack approach with an O(1) space
//! optimized iterator `fold` approach.

/// Brute Force / Stack Approach
///
/// Time: O(n log n) - Sorting the cars dominates the time complexity.
/// Space: O(n) - We allocate a `Vec` for pairing and another for the monotonic stack.
///
/// This approach pairs the position and speed, sorts them by position in descending order (closest to target first),
/// and then calculates the time it takes for each car to reach the target. We maintain a monotonic stack of arrival times.
/// If a car's time is <= the time of the car in front of it (top of stack), it joins that fleet. Otherwise, it forms a new fleet.
#[must_use]
pub fn car_fleet_stack(target: i32, position: Vec<i32>, speed: Vec<i32>) -> i32 {
    if position.is_empty() {
        return 0;
    }

    // Pair up position and speed, and calculate time to target
    let mut cars: Vec<(i32, f64)> = position
        .into_iter()
        .zip(speed.into_iter())
        // RUST INSIGHT: Notice we cast to f64 to accurately represent arrival time
        .map(|(p, s)| (p, f64::from(target - p) / f64::from(s)))
        .collect();

    // Sort by position descending (closest to target first)
    // RUST INSIGHT: `f64` doesn't implement `Ord`, so we sort by the integer `position` only,
    // which is perfectly fine since positions are unique.
    cars.sort_unstable_by(|a, b| b.0.cmp(&a.0));

    let mut stack: Vec<f64> = Vec::new();

    for (_, time) in cars {
        // If the stack is empty, or this car takes longer to reach the target than the car in front of it,
        // it cannot catch up, so it forms a new fleet.
        // GOTCHA: We use a monotonic stack. If `time <= stack.last()`, it joins the existing fleet
        // and we don't push it. We only push when it forms a *new* slower fleet.
        if stack.is_empty() || time > *stack.last().unwrap() {
            stack.push(time);
        }
    }

    // The number of distinct fleets is the size of the stack
    stack.len() as i32
}

/// Optimized Approach: O(1) Space with Iterator Fold
///
/// Time: O(n log n) - Sorting still dominates.
/// Space: O(n) - Still need a `Vec` to sort the pairs, but we eliminate the O(n) stack.
///
/// Instead of a stack, we can use an iterator and fold. Since we process from closest to target
/// backwards, we only need to keep track of the *slowest* fleet ahead of us (the maximum time seen so far)
/// and the total count of fleets.
#[must_use]
pub fn car_fleet_optimized(target: i32, position: Vec<i32>, speed: Vec<i32>) -> i32 {
    let mut cars: Vec<(i32, i32)> = position.into_iter().zip(speed).collect();

    // Sort descending by position
    cars.sort_unstable_by(|a, b| b.0.cmp(&a.0));

    // RUST INSIGHT: `fold` allows us to maintain state (count, max_time) across the iteration
    // without explicit mutable variables outside the loop.
    let (fleets, _) = cars.into_iter().fold((0, 0.0_f64), |(count, max_time), (p, s)| {
        let time = f64::from(target - p) / f64::from(s);

        // If this car's time is strictly greater than the maximum time seen so far,
        // it means it cannot catch up to any fleet ahead of it, thus forming a new fleet.
        if time > max_time {
            (count + 1, time)
        } else {
            // Otherwise, it catches up and joins the existing fleet, so max_time remains the same
            (count, max_time)
        }
    });

    fleets
}

/// Main entry point delegating to the optimized approach.
#[must_use]
pub fn car_fleet(target: i32, position: Vec<i32>, speed: Vec<i32>) -> i32 {
    car_fleet_optimized(target, position, speed)
}

// Alternative Approaches:
// 1. Array of size Target: If `target` is small, you could use an array of size `target` to store speeds
//    at each position, avoiding the O(n log n) sort, turning it into O(target) time. However, this is
//    O(target) space and fails if `target` is very large.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_happy_path_multiple_fleets() {
        let target = 12;
        let position = vec![10, 8, 0, 5, 3];
        let speed = vec![2, 4, 1, 1, 3];
        assert_eq!(car_fleet_stack(target, position.clone(), speed.clone()), 3);
        assert_eq!(car_fleet_optimized(target, position, speed), 3);
    }

    #[test]
    fn test_edge_case_single_car() {
        let target = 10;
        let position = vec![3];
        let speed = vec![3];
        assert_eq!(car_fleet_stack(target, position.clone(), speed.clone()), 1);
        assert_eq!(car_fleet_optimized(target, position, speed), 1);
    }

    #[test]
    fn test_edge_case_empty() {
        let target = 10;
        let position = vec![];
        let speed = vec![];
        assert_eq!(car_fleet_stack(target, position.clone(), speed.clone()), 0);
        assert_eq!(car_fleet_optimized(target, position, speed), 0);
    }

    #[test]
    fn test_stress_boundary_floating_point_precision() {
        // Cars that will arrive at the exact same fractional time
        let target = 100;
        let position = vec![10, 20, 30];
        let speed = vec![9, 8, 7];
        // Time = 90/9 = 10, 80/8 = 10, 70/7 = 10
        // They all catch up at exactly the same time, forming 1 fleet.
        assert_eq!(car_fleet_stack(target, position.clone(), speed.clone()), 1);
        assert_eq!(car_fleet_optimized(target, position, speed), 1);
    }
}
