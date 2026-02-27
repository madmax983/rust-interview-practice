//! # Binary Heap Implementation
//!
//! # Header
//!
//! *   **Problem Name**: Binary Heap (Priority Queue)
//! *   **Difficulty**: Medium
//! *   **Link**: <https://en.wikipedia.org/wiki/Binary_heap>
//! *   **Why this matters in Rust**: While `std::collections::BinaryHeap` exists, understanding the array-based tree representation and sift-up/sift-down operations is crucial for implementing graph algorithms like Dijkstra or Prim.
//!
//! # Architecture
//!
//! A Binary Heap is a complete binary tree where each node is greater than or equal to its children (Max-Heap) or less than or equal (Min-Heap).
//! It is efficiently stored in a flat array.
//!
//! **Indices:**
//! For a node at index `i`:
//! *   Parent: `(i - 1) / 2`
//! *   Left Child: `2 * i + 1`
//! *   Right Child: `2 * i + 2`
//!
//! **Invariants:**
//! 1.  **Shape Property**: A complete binary tree (all levels filled except possibly the last, which is filled from left to right).
//! 2.  **Heap Property**: `parent >= children` (for Max-Heap).
//!
//! **Complexity:**
//!
//! | Operation | Time | Space |
//! | :--- | :--- | :--- |
//! | Push | O(log N) | O(1) |
//! | Pop | O(log N) | O(1) |
//! | Peek | O(1) | O(1) |
//!
//! # Rust Insight
//!
//! *   **Generics**: We use `T: Ord` to allow any comparable type.
//! *   **Zero-Cost Abstraction**: Array indices calculations are usually optimized away or very cheap.
//! *   **Vec**: Dynamic resizing comes for free.

/// A Max-Heap implementation.
pub struct BinaryHeap<T> {
    data: Vec<T>,
}

impl<T: Ord> Default for BinaryHeap<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Ord> BinaryHeap<T> {
    pub fn new() -> Self {
        Self { data: Vec::new() }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            data: Vec::with_capacity(capacity),
        }
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Pushes an item onto the heap.
    pub fn push(&mut self, item: T) {
        self.data.push(item);
        self.sift_up(self.data.len() - 1);
    }

    /// Removes the greatest item from the heap.
    pub fn pop(&mut self) -> Option<T> {
        if self.data.is_empty() {
            return None;
        }
        // Swap root with last element
        let last_idx = self.data.len() - 1;
        self.data.swap(0, last_idx);
        let result = self.data.pop();

        if !self.data.is_empty() {
            self.sift_down(0);
        }

        result
    }

    /// Returns a reference to the greatest item.
    pub fn peek(&self) -> Option<&T> {
        self.data.first()
    }

    fn sift_up(&mut self, mut idx: usize) {
        while idx > 0 {
            let parent_idx = (idx - 1) / 2;
            if self.data[idx] > self.data[parent_idx] {
                self.data.swap(idx, parent_idx);
                idx = parent_idx;
            } else {
                break;
            }
        }
    }

    fn sift_down(&mut self, mut idx: usize) {
        let len = self.data.len();
        loop {
            let left_child = 2 * idx + 1;
            let right_child = 2 * idx + 2;
            let mut largest = idx;

            if left_child < len && self.data[left_child] > self.data[largest] {
                largest = left_child;
            }

            if right_child < len && self.data[right_child] > self.data[largest] {
                largest = right_child;
            }

            if largest != idx {
                self.data.swap(idx, largest);
                idx = largest;
            } else {
                break;
            }
        }
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `std::collections::BinaryHeap`: Standard library implementation is robust and optimized.
//   It uses `sift_down_to_bottom` optimization (hole implementation) which reduces comparisons.
//
// Missing vs. Production:
// - **Min-Heap**: This is a Max-Heap. To get a Min-Heap, users must wrap types in `Reverse<T>`.
//   Production crates often allow configuring the comparator.
// - **Heapify**: Creating a heap from a `Vec` in O(N) (Floyd's algorithm) is not implemented here (we just rely on push).

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_push_pop() {
        let mut heap = BinaryHeap::new();
        heap.push(1);
        heap.push(5);
        heap.push(2);

        assert_eq!(heap.pop(), Some(5));
        assert_eq!(heap.pop(), Some(2));
        assert_eq!(heap.pop(), Some(1));
        assert_eq!(heap.pop(), None);
    }

    #[test]
    fn test_heap_property() {
        let mut heap = BinaryHeap::new();
        let data = vec![10, 5, 20, 2, 8, 15];
        for &x in &data {
            heap.push(x);
        }

        let mut sorted = Vec::new();
        while let Some(x) = heap.pop() {
            sorted.push(x);
        }

        assert_eq!(sorted, vec![20, 15, 10, 8, 5, 2]);
    }

    #[test]
    fn test_peek() {
        let mut heap = BinaryHeap::new();
        heap.push(10);
        assert_eq!(heap.peek(), Some(&10));
        heap.push(20);
        assert_eq!(heap.peek(), Some(&20));
        heap.pop();
        assert_eq!(heap.peek(), Some(&10));
    }

    #[test]
    fn test_empty() {
        let mut heap: BinaryHeap<i32> = BinaryHeap::new();
        assert!(heap.is_empty());
        assert_eq!(heap.peek(), None);
        assert_eq!(heap.pop(), None);
    }
}
