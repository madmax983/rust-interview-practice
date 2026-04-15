//! # 735. Asteroid Collision
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/asteroid-collision/>
//!
//! We are given an array `asteroids` of integers representing asteroids in a row.
//! For each asteroid, the absolute value represents its size, and the sign represents its
//! direction (positive meaning right, negative meaning left). Each asteroid moves at the same speed.
//!
//! Find out the state of the asteroids after all collisions. If two asteroids meet, the smaller
//! one will explode. If both are the same size, both will explode. Two asteroids moving in the
//! same direction will never meet.
//!
//! This problem perfectly demonstrates Rust's `Vec` acting as a stack, and more importantly,
//! how building custom enums and using `std::cmp::Ordering` can eliminate confusing nested logic
//! associated with tracking signs and magnitudes.

use std::cmp::Ordering;

/// Brute force approach: Repeatedly scan and resolve adjacent collisions.
///
/// Time: O(N²) - In the worst case, we might scan the array O(N) times to resolve collisions.
/// Space: O(N) - Uses a temporary array to build the next state.
///
/// This approach literally simulates the problem by scanning for adjacent colliding pairs
/// (a positive asteroid followed immediately by a negative one), resolving the collision,
/// and building a new array for the next pass until no more collisions occur.
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
pub fn asteroid_collision_brute_force(mut asteroids: Vec<i32>) -> Vec<i32> {
    loop {
        let mut next_state = Vec::new();
        let mut i = 0;
        let mut collided_this_pass = false;

        while i < asteroids.len() {
            // Check if we have an adjacent collision pair
            if i + 1 < asteroids.len() && asteroids[i] > 0 && asteroids[i + 1] < 0 {
                collided_this_pass = true;
                let left = asteroids[i];
                let right_abs = asteroids[i + 1].abs();

                match left.cmp(&right_abs) {
                    Ordering::Greater => {
                        // Left wins, append left, skip right
                        next_state.push(asteroids[i]);
                    }
                    Ordering::Less => {
                        // Right wins, skip left, append right
                        next_state.push(asteroids[i + 1]);
                    }
                    Ordering::Equal => {
                        // Both explode, skip both
                    }
                }
                i += 2; // We processed both
            } else {
                // No immediate collision here, push current and move on
                next_state.push(asteroids[i]);
                i += 1;
            }
        }

        asteroids = next_state;

        if !collided_this_pass {
            break;
        }
    }

    asteroids
}

/// Optimized approach: Single pass using a Stack.
/// Time: O(N) - Each asteroid is pushed and popped at most once.
/// Space: O(N) - The stack can grow up to the size of the input.
///
/// We iterate through the asteroids and maintain a stack of the survivors.
/// A collision only happens if the top of the stack is moving right (>0)
/// and the incoming asteroid is moving left (<0). We use a `while` loop
/// to resolve cascading collisions.
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
#[allow(clippy::missing_panics_doc)] // Unwraps are safe due to is_empty check
pub fn asteroid_collision_optimized(asteroids: Vec<i32>) -> Vec<i32> {
    // GOTCHA: Pre-allocate capacity if possible. We might keep all asteroids.
    let mut stack = Vec::with_capacity(asteroids.len());

    for ast in asteroids {
        let mut alive = true;

        // While there is a collision scenario: stack top is right (+), current is left (-)
        while alive && !stack.is_empty() && *stack.last().unwrap() > 0 && ast < 0 {
            let top: i32 = *stack.last().unwrap();
            let ast_abs = ast.abs();

            match top.cmp(&ast_abs) {
                Ordering::Less => {
                    // Top explodes, incoming still alive to check next
                    stack.pop();
                }
                Ordering::Equal => {
                    // Both explode
                    stack.pop();
                    alive = false;
                }
                Ordering::Greater => {
                    // Incoming explodes
                    alive = false;
                }
            }
        }

        if alive {
            stack.push(ast);
        }
    }

    stack
}

/// A strongly-typed representation of an Asteroid to clarify logic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Asteroid {
    Left(i32),
    Right(i32),
}

impl Asteroid {
    const fn new(val: i32) -> Self {
        if val < 0 {
            Self::Left(val.abs())
        } else {
            Self::Right(val)
        }
    }

    const fn to_i32(self) -> i32 {
        match self {
            Self::Left(size) => -size,
            Self::Right(size) => size,
        }
    }
}

/// Optimal approach: Using a Stack with strongly-typed states.
/// Time: O(N) - Each asteroid is pushed and popped at most once.
/// Space: O(N) - The stack can grow up to the size of the input.
///
/// This approach solves the same logic as the optimized version but translates the
/// raw integers into an `Asteroid` enum. This separates the concept of "direction"
/// from "magnitude", making the collision resolution logic explicit and eliminating
/// implicit integer sign-checking bugs.
///
/// In Python/Java, creating a small custom class for this might add overhead or
/// verbosity. In Rust, enums are zero-cost abstractions, so this improves readability
/// without impacting performance.
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
pub fn asteroid_collision_optimal(asteroids: Vec<i32>) -> Vec<i32> {
    let mut stack: Vec<Asteroid> = Vec::with_capacity(asteroids.len());

    for val in asteroids {
        let current = Asteroid::new(val);
        let mut alive = true;

        while alive {
            // RUST INSIGHT: Pattern matching gracefully extracts the state.
            // We only care about the collision scenario: Right vs Left.
            match (stack.last(), current) {
                (Some(&Asteroid::Right(top_size)), Asteroid::Left(curr_size)) => {
                    // RUST INSIGHT: `std::cmp::Ordering` makes size comparisons exhaustive.
                    match top_size.cmp(&curr_size) {
                        Ordering::Less => {
                            // Top is smaller and explodes. Current continues.
                            stack.pop();
                        }
                        Ordering::Equal => {
                            // Both are equal and explode.
                            stack.pop();
                            alive = false;
                        }
                        Ordering::Greater => {
                            // Top is larger, current explodes.
                            alive = false;
                        }
                    }
                }
                _ => {
                    // No collision scenario (Left vs Left, Right vs Right, Left vs Right)
                    break;
                }
            }
        }

        if alive {
            stack.push(current);
        }
    }

    // Convert back to raw i32
    stack.into_iter().map(Asteroid::to_i32).collect()
}

/// Main entry point - uses optimal solution
#[must_use]
pub fn asteroid_collision(asteroids: Vec<i32>) -> Vec<i32> {
    asteroid_collision_optimal(asteroids)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_happy_path() {
        let input = vec![5, 10, -5];
        let expected = vec![5, 10];
        assert_eq!(asteroid_collision_brute_force(input.clone()), expected);
        assert_eq!(asteroid_collision_optimized(input.clone()), expected);
        assert_eq!(asteroid_collision_optimal(input.clone()), expected);
        assert_eq!(asteroid_collision(input), expected);
    }

    #[test]
    fn test_equal_collision() {
        let input = vec![8, -8];
        let expected: Vec<i32> = vec![];
        assert_eq!(asteroid_collision_brute_force(input.clone()), expected);
        assert_eq!(asteroid_collision_optimized(input.clone()), expected);
        assert_eq!(asteroid_collision_optimal(input.clone()), expected);
    }

    #[test]
    fn test_cascading_collision() {
        let input = vec![10, 2, -5];
        let expected = vec![10];
        assert_eq!(asteroid_collision_brute_force(input.clone()), expected);
        assert_eq!(asteroid_collision_optimized(input.clone()), expected);
        assert_eq!(asteroid_collision_optimal(input.clone()), expected);
    }

    #[test]
    fn test_no_collision_diverging() {
        // -2 goes left, 1 goes right, 2 goes right. They never meet.
        let input = vec![-2, 1, 2];
        let expected = vec![-2, 1, 2];
        assert_eq!(asteroid_collision_brute_force(input.clone()), expected);
        assert_eq!(asteroid_collision_optimized(input.clone()), expected);
        assert_eq!(asteroid_collision_optimal(input.clone()), expected);
    }

    #[test]
    fn test_stress_massive_cascade() {
        let input = vec![1, 2, 3, 4, 5, -10];
        let expected = vec![-10];
        assert_eq!(asteroid_collision_brute_force(input.clone()), expected);
        assert_eq!(asteroid_collision_optimized(input.clone()), expected);
        assert_eq!(asteroid_collision_optimal(input.clone()), expected);
    }
}

// Alternative approaches:
// - Two pointer approach: Instead of an explicit stack `Vec`, you can simulate a stack using
//   a read pointer and a write pointer in-place on the original `asteroids` array to achieve
//   O(1) extra space. However, modifying a `Vec` in-place while iterating is often less
//   idiomatic in Rust than building a new stack due to borrowing rules, and `Vec::retain`
//   doesn't work well for resolving cascading look-backs.
