//! # 853. Car Fleet
//!
//! **Difficulty:** Medium
//!
//! Given `n` cars going to the same destination along a one-lane road, return the number of car fleets that will arrive at the destination.
//!
//! [LeetCode Problem 853](https://leetcode.com/problems/car-fleet/)
//!
//! ## Why This Matters in Rust
//!
//! This problem highlights Rust's explicit handling of non-total ordering (`Ord` vs `PartialOrd`), specifically when dealing with floating-point numbers (`f32`/`f64`). It demonstrates how to build and manipulate monotonic stacks safely, and how to rewrite a space-heavy stack algorithm into an elegant, O(1) space iterator chain using `fold`.

/// Monotonic Stack Approach
///
/// We first pair each car's position and speed, then sort the cars by their starting position in descending order
/// (closest to the target first). We calculate the time it takes for each car to reach the target if it were driving alone.
///
/// We maintain a stack of arrival times. For each car, if its arrival time is less than or equal to the time at the top of
/// the stack (the fleet immediately ahead of it), it will catch up and join that fleet. We represent the fleet by the time
/// of the slower car (which takes longer), so we just drop the current car's time. If it takes strictly longer, it forms a
/// new fleet, and we push its time onto the stack.
///
/// Time Complexity: O(N log N) - We sort the cars by position.
/// Space Complexity: O(N) - We store the zipped pairs and use a stack that could grow to size N.
#[must_use]
pub fn car_fleet(target: i32, position: Vec<i32>, speed: Vec<i32>) -> i32 {
    let mut cars: Vec<(i32, i32)> = position.into_iter().zip(speed).collect();

    // Sort by position in descending order so we process cars closest to the target first.
    // RUST INSIGHT: `sort_unstable_by` is typically faster and uses less memory than `sort` when stable sorting isn't needed.
    // We are sorting by `i32` which implements `Ord`, avoiding the need to sort by `f64` which only implements `PartialOrd`.
    // If we wanted to sort by `f64`, we'd have to handle `NaN` explicitly (e.g. `a.partial_cmp(b).unwrap()`).
    cars.sort_unstable_by(|a, b| b.0.cmp(&a.0));

    // The stack will store the arrival times of the fleets.
    let mut stack: Vec<f64> = Vec::with_capacity(cars.len());

    for (pos, spd) in cars {
        let time = f64::from(target - pos) / f64::from(spd);

        stack.push(time);
        let len = stack.len();

        // If the current car arrives faster or at the same time as the fleet ahead of it,
        // it joins that fleet. The fleet's arrival time is dictated by the slower car ahead.
        // GOTCHA: Direct floating-point equality (==) can be risky due to precision, but `<=`
        // is safe and correct here because of how fleets form bottlenecks.
        if len >= 2 && stack[len - 1] <= stack[len - 2] {
            stack.pop();
        }
    }

    stack.len() as i32
}

/// Optimized Iterator Approach
///
/// Instead of using a stack to keep track of every fleet's time, we only need to track the
/// number of fleets and the time of the slowest fleet we've encountered so far.
/// We can process the sorted cars using a functional iterator chain with `fold`.
///
/// Time Complexity: O(N log N) - For sorting.
/// Space Complexity: O(N) - For the zipped pairs (though O(1) auxiliary space during evaluation).
#[must_use]
pub fn car_fleet_optimized(target: i32, position: Vec<i32>, speed: Vec<i32>) -> i32 {
    let mut cars: Vec<(i32, i32)> = position.into_iter().zip(speed).collect();
    cars.sort_unstable_by(|a, b| b.0.cmp(&a.0));

    // RUST INSIGHT: Iterator adapters like `map` and `fold` compile down to efficient loops.
    // This eliminates the heap allocation overhead of maintaining a separate `Vec` as a stack.
    cars.into_iter()
        .map(|(p, s)| f64::from(target - p) / f64::from(s))
        .fold((0, 0.0f64), |(fleets, max_time), time| {
            // If this car takes strictly longer than the current bottleneck fleet,
            // it forms a new fleet and becomes the new bottleneck.
            if time > max_time {
                (fleets + 1, time)
            } else {
                (fleets, max_time)
            }
        })
        .0
}

// Alternative Approaches:
// 1. Cross-Multiplication (Integer Arithmetic): Instead of using floating-point division, we could compare
//    times using cross-multiplication with `i64` to avoid `f64` precision quirks entirely.
//    `time1 <= time2` is equivalent to `dist1 * speed2 <= dist2 * speed1`.
// 2. Index Array: Rather than zipping and collecting pairs into a new `Vec`, allocate a `Vec<usize>` of
//    indices `0..n`, sort the indices by `position[i]`, and iterate. This saves pair allocation overhead
//    but has worse cache locality due to random memory access.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_happy_path() {
        let target = 12;
        let position = vec![10, 8, 0, 5, 3];
        let speed = vec![2, 4, 1, 1, 3];
        assert_eq!(car_fleet(target, position.clone(), speed.clone()), 3);
        assert_eq!(car_fleet_optimized(target, position, speed), 3);
    }

    #[test]
    fn test_edge_case_single_car() {
        let target = 10;
        let position = vec![3];
        let speed = vec![3];
        assert_eq!(car_fleet(target, position.clone(), speed.clone()), 1);
        assert_eq!(car_fleet_optimized(target, position, speed), 1);
    }

    #[test]
    fn test_edge_case_empty() {
        let target = 10;
        let position = vec![];
        let speed = vec![];
        assert_eq!(car_fleet(target, position.clone(), speed.clone()), 0);
        assert_eq!(car_fleet_optimized(target, position, speed), 0);
    }

    #[test]
    fn test_stress_boundary_case() {
        // Many cars at different positions but fast cars behind slow cars,
        // all merging into a single fleet at the target.
        let target = 100;
        let position = vec![0, 20, 40, 60, 80];
        let speed = vec![10, 8, 6, 4, 2];
        // times:
        // pos=80, spd=2 -> time = 10
        // pos=60, spd=4 -> time = 10
        // pos=40, spd=6 -> time = 10
        // pos=20, spd=8 -> time = 10
        // pos=0, spd=10 -> time = 10
        // They all have time = 10, so they all catch up and form exactly 1 fleet.
        assert_eq!(car_fleet(target, position.clone(), speed.clone()), 1);
        assert_eq!(car_fleet_optimized(target, position, speed), 1);
    }

    #[test]
    fn test_no_catching_up() {
        // Slower cars behind faster cars.
        let target = 10;
        let position = vec![6, 8];
        let speed = vec![3, 2];
        // 8 -> (10-8)/2 = 1.0
        // 6 -> (10-6)/3 = 1.333
        // 6 takes longer than 8, so it forms a new fleet. Total = 2.
        assert_eq!(car_fleet(target, position.clone(), speed.clone()), 2);
        assert_eq!(car_fleet_optimized(target, position, speed), 2);
    }
}
