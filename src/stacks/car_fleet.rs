//! # 853. Car Fleet
//!
//! Link: <https://leetcode.com/problems/car-fleet/>
//!
//! There are `n` cars at given miles away from the starting mile 0, traveling to reach the mile `target`.
//! You are given two integer array `position` and `speed`, both of length `n`, where `position[i]` is the
//! starting mile of the `i`th car and `speed[i]` is the speed of the `i`th car in miles per hour.
//!
//! A car cannot pass another car ahead of it. It can only catch up to it and then drive at the same speed
//! at the position of the car ahead of it. A car fleet is some non-empty set of cars driving at the same
//! position and same speed. Note that a single car is also a car fleet.
//!
//! Return the number of car fleets that will arrive at the destination.
//!
//! ## Why This Matters in Rust
//!
//! This problem perfectly demonstrates the Monotonic Stack pattern and how Rust's robust type system enforces
//! explicit handling of floating-point numbers. Unlike Python or C++, Rust's `f64` does not implement the `Ord`
//! trait because floats have `NaN`, which breaks the total ordering property. This forces developers to
//! explicitly handle potential `NaN`s when sorting or comparing floats, eliminating a whole class of silent bugs.
//!
//! ## Approach
//!
//! We need to find how many fleets arrive at the target. A car catches up to a car ahead of it if its time
//! to reach the target is less than or equal to the time of the car ahead.
//!
//! 1.  **Pair and Sort**: First, we pair each car's position and speed. Then, we sort the cars by their starting
//!     position in descending order (closest to the target first). This allows us to process cars from front to back.
//! 2.  **Calculate Time**: The time to reach the target is `(target - position) as f64 / speed as f64`.
//! 3.  **Monotonic Stack / Fold**: As we iterate through the sorted cars, we compare each car's time to reach the target
//!     with the slowest time of the current fleet. If the current car takes longer, it forms a new fleet, and its time
//!     becomes the new benchmark. If it's faster or equal, it catches up and joins the existing fleet.

/// Monotonic Stack Approach
///
/// This approach explicitly uses a `Vec` as a stack to keep track of the arrival times of the fleets.
///
/// Time: O(N log N) - Sorting the cars dominates the runtime.
/// Space: O(N) - We store the combined array and the stack of times.
///
/// # Gotcha
/// If we needed to sort by `f64` time, we couldn't just use `.sort()`. We'd need `.sort_by(|a, b| a.partial_cmp(b).unwrap())`.
/// Luckily, we can sort by `position` which is an integer and naturally implements `Ord`.
#[must_use]
pub fn car_fleet_stack(target: i32, position: Vec<i32>, speed: Vec<i32>) -> i32 {
    let n = position.len();
    if n == 0 {
        return 0;
    }

    // Pair position and speed, then sort by position in descending order.
    // RUST INSIGHT: `Vec::into_iter().zip()` is a zero-cost abstraction for combining two arrays.
    let mut cars: Vec<(i32, i32)> = position.into_iter().zip(speed).collect();

    // Sort descending by position
    cars.sort_unstable_by(|a, b| b.0.cmp(&a.0));

    let mut stack: Vec<f64> = Vec::with_capacity(n);

    for (pos, spd) in cars {
        let time = (target - pos) as f64 / spd as f64;

        // RUST INSIGHT: Rust forces us to handle `last()` safely via Option.
        // We compare using `*stack.last().unwrap()`.
        // However, we only care if the current time is strictly greater than the top of the stack.
        // If the stack is empty, or the time is greater, we push.

        let mut form_new_fleet = false;
        if let Some(&top_time) = stack.last() {
            if time > top_time {
                form_new_fleet = true;
            }
        } else {
            form_new_fleet = true;
        }

        if form_new_fleet {
            stack.push(time);
        }
    }

    stack.len() as i32
}

/// Optimized Approach: O(1) Space Iterator Fold
///
/// Instead of pushing times onto a stack, we only ever need to track the `max_time` seen so far
/// and the `fleet_count`. This eliminates the need for the stack entirely.
///
/// Time: O(N log N) - Still requires sorting.
/// Space: O(N) - For the combined array (though we could potentially avoid this with an index array).
///
/// # Rust Insight
/// Using `fold` allows us to express the state transition declaratively. The state is a tuple:
/// `(fleet_count, max_time_seen)`.
#[must_use]
pub fn car_fleet_optimal(target: i32, position: Vec<i32>, speed: Vec<i32>) -> i32 {
    if position.is_empty() {
        return 0;
    }

    let mut cars: Vec<(i32, i32)> = position.into_iter().zip(speed).collect();

    // Sort ascending by position
    cars.sort_unstable_by_key(|&(p, _)| p);

    // We process from right to left (closest to target first) using `.into_iter().rev()`.
    let (fleets, _) = cars.into_iter().rev().fold(
        (0, 0.0_f64),
        |(fleet_count, max_time_seen), (pos, spd)| {
            let time = (target - pos) as f64 / spd as f64;
            // GOTCHA: Floating point comparison directly with operators is safe here because we know
            // neither value is NaN, but Rust will complain if you try to `cmp` f64 directly.
            if time > max_time_seen {
                // Forms a new fleet, update max time
                (fleet_count + 1, time)
            } else {
                // Joins the existing fleet, count and max time remain unchanged
                (fleet_count, max_time_seen)
            }
        },
    );

    fleets
}

/// Main entry point.
///
/// We default to the optimal O(1) extra space solution as it is idiomatic and clean.
#[must_use]
pub fn car_fleet(target: i32, position: Vec<i32>, speed: Vec<i32>) -> i32 {
    car_fleet_optimal(target, position, speed)
}

// -----------------------------------------------------------------------------------------
// Alternative Approaches
// -----------------------------------------------------------------------------------------
//
// 1. **Index Array Sorting**:
//    Instead of `zip` and `collect` into a new `Vec<(i32, i32)>`, we could create a `Vec<usize>`
//    of indices `(0..n).collect()`, sort that index array by referencing `position[i]`, and then
//    iterate over the indices. This reduces allocation overhead if the structs are very large,
//    though for simple integers, it's virtually the same.
//
// 2. **Map/BTreeMap**:
//    If we knew positions were unique, we could insert them into a `BTreeMap<i32, i32>`
//    (position -> speed). `BTreeMap` is naturally sorted, so iterating backwards would give us
//    the correct order. This is O(N log N) space and time, but avoids explicit sorting.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_car_fleet_happy_path() {
        let target = 12;
        let position = vec![10, 8, 0, 5, 3];
        let speed = vec![2, 4, 1, 1, 3];
        // Expected: 3 fleets
        assert_eq!(car_fleet_stack(target, position.clone(), speed.clone()), 3);
        assert_eq!(car_fleet_optimal(target, position, speed), 3);
    }

    #[test]
    fn test_car_fleet_single_car() {
        let target = 10;
        let position = vec![3];
        let speed = vec![3];
        assert_eq!(car_fleet_stack(target, position.clone(), speed.clone()), 1);
        assert_eq!(car_fleet_optimal(target, position, speed), 1);
    }

    #[test]
    fn test_car_fleet_empty() {
        let target = 10;
        let position = vec![];
        let speed = vec![];
        assert_eq!(car_fleet_stack(target, position.clone(), speed.clone()), 0);
        assert_eq!(car_fleet_optimal(target, position, speed), 0);
    }

    #[test]
    fn test_car_fleet_no_catch_up() {
        let target = 100;
        let position = vec![0, 2, 4];
        let speed = vec![1, 2, 4]; // The car in front is fastest, car behind is slowest
        // Since 100 > 49 > 24, each forms its own fleet (no catch up).
        assert_eq!(car_fleet_stack(target, position.clone(), speed.clone()), 3);
        assert_eq!(car_fleet_optimal(target, position, speed), 3);
    }

    #[test]
    fn test_car_fleet_all_catch_up() {
        let target = 10;
        let position = vec![2, 4];
        let speed = vec![3, 2];
        // 2.66 < 3.0, car at 2 catches up to car at 4. Fleet count = 1
        assert_eq!(car_fleet_stack(target, position.clone(), speed.clone()), 1);
        assert_eq!(car_fleet_optimal(target, position, speed), 1);
    }
}
