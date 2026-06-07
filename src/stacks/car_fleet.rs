//! # 853. Car Fleet
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/car-fleet/>
//!
//! There are `n` cars going to the same destination along a one-lane road. The destination is `target` miles away.
//! You are given two integer array `position` and `speed`, both of length `n`, where `position[i]` is the position
//! of the `i`th car and `speed[i]` is the speed of the `i`th car (in miles per hour).
//!
//! A car can never pass another car ahead of it, but it can catch up to it and drive bumper to bumper at the same speed.
//! The faster car will slow down to match the slower car's speed. The distance between these two cars is ignored.
//! A car fleet is some non-empty set of cars driving together.
//!
//! Return the number of car fleets that will arrive at the destination.
//!
//! This problem matters in Rust because it demonstrates how to combine multiple data vectors,
//! sort them using custom comparators or iterators, and safely handle floating-point logic
//! without running afoul of Rust's strict `Ord` and `PartialOrd` requirements for floats.

/// Straightforward Approach: Using an Explicit Stack
///
/// We zip `position` and `speed` together, sort them by position descending (closest to target first).
/// Then we calculate the time each car needs to reach the target.
/// We iterate through these times and simulate a stack.
///
/// Time: O(n log n) - sorting dominates the runtime.
/// Space: O(n) - we create a combined vector of pairs and an explicit stack of times.
///
/// # Rust Insight
/// We use `.into_iter().zip()` to iterate two `Vec`s simultaneously.
/// `f32` (or `f64`) in Rust does not implement `Ord` because `NaN != NaN`.
/// This prevents us from using standard `.sort()` if we were sorting by times directly.
/// Here we sort by `position` which is `i32` and perfectly comparable.
#[must_use]
pub fn car_fleet_stack(target: i32, position: Vec<i32>, speed: Vec<i32>) -> i32 {
    if position.is_empty() {
        return 0;
    }

    // Combine position and speed, then sort by position descending.
    // GOTCHA: We must sort by position descending so we process the car closest to the target first.
    let mut cars: Vec<(i32, i32)> = position.into_iter().zip(speed).collect();
    cars.sort_unstable_by(|a, b| b.0.cmp(&a.0));

    let mut stack: Vec<f64> = Vec::with_capacity(cars.len());

    for (pos, spd) in cars {
        let time = f64::from(target - pos) / f64::from(spd);
        stack.push(time);

        let n = stack.len();
        // If the current car arrives faster than or at the same time as the car ahead of it,
        // it catches up and forms a fleet. We keep the slower time (the car ahead).
        if n >= 2 && stack[n - 1] <= stack[n - 2] {
            stack.pop();
        }
    }

    // The number of distinct fleets is the size of the stack.
    stack.len() as i32
}

/// Optimized Approach: O(1) Extra Space (Iterators & Fold)
///
/// Instead of a literal `Vec` stack, we can keep track of the number of fleets and the time
/// of the slowest car ahead of us. If the current car takes strictly more time, it forms a new fleet.
///
/// Time: O(n log n) - still requires sorting.
/// Space: O(n) - to zip and sort. We avoid the `O(n)` stack vector, but we still allocate for sorting.
///
/// # Rust Insight
/// We can use a `fold` to elegantly thread state (fleet count and max time) through the iterator.
/// Rust's zero-cost abstractions allow this iterator chain to compile down to extremely efficient loops.
#[must_use]
pub fn car_fleet_optimized(target: i32, position: Vec<i32>, speed: Vec<i32>) -> i32 {
    if position.is_empty() {
        return 0;
    }

    let mut cars: Vec<(i32, i32)> = position.into_iter().zip(speed).collect();
    cars.sort_unstable_by(|a, b| b.0.cmp(&a.0));

    let (fleets, _) = cars.into_iter().fold((0, 0.0), |(fleet_count, max_time), (pos, spd)| {
        let time = f64::from(target - pos) / f64::from(spd);
        // If this car needs strictly more time, it cannot catch the fleet ahead
        if time > max_time {
            (fleet_count + 1, time)
        } else {
            // It caught up, so it's part of the same fleet (max_time remains unchanged)
            (fleet_count, max_time)
        }
    });

    fleets
}

/// Default implementation
#[must_use]
pub fn car_fleet(target: i32, position: Vec<i32>, speed: Vec<i32>) -> i32 {
    car_fleet_optimized(target, position, speed)
}

// Alternative approaches
//
// 1. Array with indices tracking instead of creating a combined tuple vector:
//    You could create an array of indices `[0..n]`, sort the indices by `position[i]`,
//    and then iterate using the sorted indices. This avoids copying `position` and `speed`
//    values but still requires O(n) space for the indices array.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_happy_path() {
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
    fn test_stress_fast_cars_behind() {
        let target = 100;
        let position = vec![0, 2, 4];
        let speed = vec![4, 2, 1];
        // Cars at 4 (speed 1), 2 (speed 2), 0 (speed 4)
        // Times:
        // pos 4: (100-4)/1 = 96
        // pos 2: (100-2)/2 = 49 -> Catches car at 4
        // pos 0: (100-0)/4 = 25 -> Catches car at 2
        // All form a single fleet
        assert_eq!(car_fleet_stack(target, position.clone(), speed.clone()), 1);
        assert_eq!(car_fleet_optimized(target, position, speed), 1);
    }
}
