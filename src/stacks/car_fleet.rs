//! # 853. Car Fleet
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/car-fleet/>
//!
//! There are `n` cars going to the same destination along a one-lane road. The destination is `target` miles away.
//!
//! You are given two integer arrays `position` and `speed`, both of length `n`, where `position[i]` is the position of the `ith` car and `speed[i]` is the speed of the `ith` car (in miles per hour).
//!
//! A car can never pass another car ahead of it, but it can catch up to it and drive bumper to bumper at the same speed.
//! The faster car will slow down to match the slower car's speed. The distance between these two cars is ignored (i.e., they are assumed to have the same position).
//!
//! A car fleet is some non-empty set of cars driving at the same position and same speed. Note that a single car is also a car fleet.
//!
//! Return the number of car fleets that will arrive at the destination.
//!
//! This problem demonstrates evaluating fleet formation using a straightforward monotonic stack versus an O(1) space optimized iterator `fold` approach. It also highlights Rust's explicit handling of floating-point non-total ordering (`Ord` vs `PartialOrd`), as time to target is a float.
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

/// Brute Force / Standard Stack approach
///
/// Time: O(N log N) - Sorting the cars by position takes O(N log N).
/// Space: O(N) - Storing the cars array and the stack.
///
/// We pair up the cars' positions and speeds, sort them by position in descending order
/// (closest to target first), and calculate the time it takes for each car to reach the target.
/// If a car behind takes less or equal time to reach the target than the car fleet in front,
/// it catches up and joins the fleet. Otherwise, it forms a new fleet.
#[must_use]
pub fn car_fleet_stack(target: i32, position: Vec<i32>, speed: Vec<i32>) -> i32 {
    let n = position.len();
    if n == 0 {
        return 0;
    }

    // Pair positions and speeds
    let mut cars: Vec<(i32, i32)> = position.into_iter().zip(speed).collect();

    // Sort by position descending (closest to target first)
    // GOTCHA: We must reverse the default ascending sort to process cars from front to back.
    cars.sort_unstable_by(|a, b| b.0.cmp(&a.0));

    let mut stack: Vec<f64> = Vec::with_capacity(n);

    for (p, s) in cars {
        let time = f64::from(target - p) / f64::from(s);

        // If stack is empty or this car takes longer than the fleet at the top of the stack,
        // it forms a new fleet.
        if stack.is_empty() || time > *stack.last().unwrap() {
            stack.push(time);
        }
    }

    stack.len() as i32
}

/// Optimized approach: O(1) Space Iterator Fold
///
/// Time: O(N log N) - Sorting still dominates.
/// Space: O(N) - We still need an array to sort, but we eliminate the O(N) stack.
///
/// Instead of pushing times to a stack, we only care about the time of the *slowest* fleet
/// currently leading. We can use an iterator `fold` to keep track of the maximum time seen so far
/// and count how many times this maximum increases.
#[must_use]
pub fn car_fleet_optimized(target: i32, position: Vec<i32>, speed: Vec<i32>) -> i32 {
    let n = position.len();
    if n == 0 {
        return 0;
    }

    let mut cars: Vec<(i32, i32)> = position.into_iter().zip(speed).collect();
    cars.sort_unstable_by(|a, b| b.0.cmp(&a.0));

    // RUST INSIGHT: fold is perfect for accumulating state over an iterator.
    // Our state is a tuple: (number of fleets, time of the slowest fleet leading so far)
    let (fleets, _) =
        cars.into_iter()
            .map(|(p, s)| f64::from(target - p) / f64::from(s))
            .fold((0, 0.0), |(count, max_time), time| {
                // RUST INSIGHT: f64 only implements PartialOrd, not Ord, due to NaN.
                // However, we know our times are well-defined positive floats, so standard > is fine.
                if time > max_time {
                    // New fleet formed
                    (count + 1, time)
                } else {
                    // Car catches up to the current fleet, no new fleet
                    (count, max_time)
                }
            });

    fleets
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
    fn test_happy_path() {
        let target = 12;
        let position = vec![10, 8, 0, 5, 3];
        let speed = vec![2, 4, 1, 1, 3];
        assert_eq!(car_fleet_stack(target, position.clone(), speed.clone()), 3);
        assert_eq!(car_fleet_optimized(target, position, speed), 3);
    }

    #[test]
    fn test_single_car() {
        let target = 10;
        let position = vec![3];
        let speed = vec![3];
        assert_eq!(car_fleet_stack(target, position.clone(), speed.clone()), 1);
        assert_eq!(car_fleet_optimized(target, position, speed), 1);
    }

    #[test]
    fn test_all_same_speed_different_positions() {
        let target = 100;
        let position = vec![0, 2, 4];
        let speed = vec![4, 2, 1];
        // Cars:
        // P=4, S=1 -> time = 96
        // P=2, S=2 -> time = 49 (catches up to P=4? No, it's faster, so it arrives in 49. Wait, P=4 is at 4, P=2 is at 2.
        // Let's trace:
        // 1. (4, 1) -> time = 96
        // 2. (2, 2) -> time = 49. 49 <= 96, so it catches up to the fleet at (4,1). Fleet 1.
        // 3. (0, 4) -> time = 25. 25 <= 96 (the fleet's speed is now 1), catches up. Fleet 1.
        assert_eq!(car_fleet_stack(target, position.clone(), speed.clone()), 1);
        assert_eq!(car_fleet_optimized(target, position, speed), 1);
    }

    #[test]
    fn test_stress_boundary() {
        // Cars arriving at the exact same time
        let target = 10;
        let position = vec![6, 8];
        let speed = vec![2, 1];
        // (8, 1) -> time = 2
        // (6, 2) -> time = 2. 2 <= 2, catches up. 1 fleet.
        assert_eq!(car_fleet_stack(target, position.clone(), speed.clone()), 1);
        assert_eq!(car_fleet_optimized(target, position, speed), 1);
    }
}
