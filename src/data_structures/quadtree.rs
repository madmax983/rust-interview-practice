//! # Quadtree Implementation
//!
//! A tree data structure in which each internal node has exactly four children.
//! Used to partition a two-dimensional space by recursively subdividing it into four quadrants.
//!
//! **Replaces Crates:** `quadtree`, `rstar` (conceptually), `spatial-partitioning`
//!
//! **Real-world Usage:**
//! - Image compression (storing uniform color regions).
//! - Spatial indexing (finding all points within a range).
//! - Collision detection (broad-phase physics).
//! - Sparse data storage (spreadsheets).
//!
//! **Why build it yourself?**
//! It teaches recursive spatial subdivision and AABB (Axis-Aligned Bounding Box) intersection logic.
//! You learn how to handle the tradeoff between tree depth and bucket capacity.

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure:
// - `Point`: (x, f32), (y, f32)
// - `AABB`: Center (x, y) and Half-Dimension (w, h).
// - `Quadtree`:
//   - `boundary`: AABB
//   - `capacity`: Max points before splitting
//   - `points`: Vec<Point> (only if leaf)
//   - `divided`: bool
//   - `children`: Option<Box<[Quadtree; 4]>> (NW, NE, SW, SE)
//
// Invariants:
// 1. Points are stored only in leaf nodes (or we store in all nodes, but leaves is standard).
//    *Wait, standard PR Quadtree stores points in leaves. Region Quadtree might differ.*
//    *We will implement a bucket-based Region Quadtree: Points accumulate until capacity, then split.*
// 2. A node is either a leaf (no children) or internal (4 children).
// 3. All points in a node are contained within its boundary.
//
// Complexity:
// ┌───────────────┬─────────────┬─────────────┐
// │ Operation     │ Time        │ Space       │
// ├───────────────┼─────────────┼─────────────┤
// │ Insert        │ O(log N)    │ O(N)        │
// │ Query (Range) │ O(N) worst  │ O(N)        │
// │               │ O(log N) avg│             │
// └───────────────┴─────────────┴─────────────┘

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    #[must_use] 
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AABB {
    pub center: Point,
    pub half_dimension: f32, // Square regions for simplicity (width/2)
}

impl AABB {
    #[must_use] 
    pub const fn new(center: Point, half_dimension: f32) -> Self {
        Self {
            center,
            half_dimension,
        }
    }

    #[must_use] 
    pub fn contains(&self, p: &Point) -> bool {
        p.x >= self.center.x - self.half_dimension
            && p.x <= self.center.x + self.half_dimension
            && p.y >= self.center.y - self.half_dimension
            && p.y <= self.center.y + self.half_dimension
    }

    #[must_use] 
    pub fn intersects(&self, other: &Self) -> bool {
        // Separating Axis Theorem (simplified for AABB)
        let dx = (self.center.x - other.center.x).abs();
        let dy = (self.center.y - other.center.y).abs();
        let sum_dim = self.half_dimension + other.half_dimension;

        dx <= sum_dim && dy <= sum_dim
    }
}

/// Maximum subdivision depth. Prevents infinite recursion when more than
/// `capacity` coincident (identical) points are inserted: at this depth a
/// leaf is allowed to hold more than `capacity` points instead of splitting.
const MAX_DEPTH: usize = 24;

pub struct Quadtree {
    boundary: AABB,
    capacity: usize,
    points: Vec<Point>,
    divided: bool,
    depth: usize,
    // Children: NW, NE, SW, SE
    children: Option<Box<[Self; 4]>>,
}

impl Quadtree {
    #[must_use] 
    pub const fn new(boundary: AABB, capacity: usize) -> Self {
        Self::with_depth(boundary, capacity, 0)
    }

    const fn with_depth(boundary: AABB, capacity: usize, depth: usize) -> Self {
        Self {
            boundary,
            capacity,
            points: Vec::new(),
            divided: false,
            depth,
            children: None,
        }
    }

    /// Insert a point into the Quadtree.
    /// Returns true if successful, false if point is out of bounds.
    pub fn insert(&mut self, p: Point) -> bool {
        if !self.boundary.contains(&p) {
            return false;
        }

        if !self.divided && (self.points.len() < self.capacity || self.depth >= MAX_DEPTH) {
            // Leaf has room, OR we've hit the max subdivision depth: at max
            // depth we stop splitting and let this leaf hold >capacity points.
            // This is what allows coincident (identical) points to terminate.
            self.points.push(p);
            return true;
        }

        if !self.divided {
            self.subdivide();
        }

        // Push to children
        // We use if-else chain because a point on the boundary might belong to multiple?
        // Standard convention: belong to the first one that accepts it (usually consistent boundary rules).
        // Since children cover the space completely, at least one will accept.

        let children = self.children.as_mut().unwrap();

        if children[0].insert(p) {
            return true;
        }
        if children[1].insert(p) {
            return true;
        }
        if children[2].insert(p) {
            return true;
        }
        if children[3].insert(p) {
            return true;
        }

        // Should be unreachable if boundary check passed and subdivide works
        false
    }

    fn subdivide(&mut self) {
        let x = self.boundary.center.x;
        let y = self.boundary.center.y;
        let hd = self.boundary.half_dimension / 2.0;

        let d = self.depth + 1;
        let nw = Self::with_depth(AABB::new(Point::new(x - hd, y + hd), hd), self.capacity, d);
        let ne = Self::with_depth(AABB::new(Point::new(x + hd, y + hd), hd), self.capacity, d);
        let sw = Self::with_depth(AABB::new(Point::new(x - hd, y - hd), hd), self.capacity, d);
        let se = Self::with_depth(AABB::new(Point::new(x + hd, y - hd), hd), self.capacity, d);

        self.children = Some(Box::new([nw, ne, sw, se]));
        self.divided = true;

        // Redistribution: Move existing points to children
        // RUST INSIGHT: We need to drain points from self.points to avoid cloning.
        while let Some(p) = self.points.pop() {
            let children = self.children.as_mut().unwrap();
            if children[0].insert(p) {
                continue;
            }
            if children[1].insert(p) {
                continue;
            }
            if children[2].insert(p) {
                continue;
            }
            if children[3].insert(p) {
                continue;
            }
        }
    }

    /// Query points within a given range (AABB).
    #[must_use] 
    pub fn query(&self, range: &AABB) -> Vec<Point> {
        let mut results = Vec::new();
        self.query_recursive(range, &mut results);
        results
    }

    fn query_recursive(&self, range: &AABB, results: &mut Vec<Point>) {
        if !self.boundary.intersects(range) {
            return;
        }

        for p in &self.points {
            if range.contains(p) {
                results.push(*p);
            }
        }

        if self.divided {
            let children = self.children.as_ref().unwrap();
            for child in children.iter() {
                child.query_recursive(range, results);
            }
        }
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `rstar`: Uses R-Trees (better for generalized rectangles, not just points).
// - `quadtree`: Similar implementation.
//
// Missing vs. Production:
// - **Generic Payload**: We only store `Point`. Real Quadtrees store `(Point, Data)`.
// - **Removal**: Removing points and merging quadrants (coalescing) is complex.
// - **Iterators**: We return `Vec<Point>`, allocating memory. Iterators would be zero-allocation traversal.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aabb_contains() {
        let box_ = AABB::new(Point::new(0.0, 0.0), 10.0);
        assert!(box_.contains(&Point::new(5.0, 5.0)));
        assert!(box_.contains(&Point::new(10.0, 10.0))); // Boundary inclusive
        assert!(!box_.contains(&Point::new(11.0, 0.0)));
    }

    #[test]
    fn test_aabb_intersects() {
        let b1 = AABB::new(Point::new(0.0, 0.0), 10.0); // -10..10
        let b2 = AABB::new(Point::new(15.0, 0.0), 10.0); // 5..25 (Overlaps 5..10)
        let b3 = AABB::new(Point::new(25.0, 0.0), 1.0); // 24..26 (No overlap)

        assert!(b1.intersects(&b2));
        assert!(!b1.intersects(&b3));
    }

    #[test]
    fn test_quadtree_insert_subdivide() {
        let boundary = AABB::new(Point::new(0.0, 0.0), 100.0);
        let mut qt = Quadtree::new(boundary, 4); // Capacity 4

        // Insert 4 points (should stay in root)
        qt.insert(Point::new(1.0, 1.0));
        qt.insert(Point::new(2.0, 2.0));
        qt.insert(Point::new(-1.0, -1.0));
        qt.insert(Point::new(-2.0, -2.0));

        assert!(!qt.divided);
        assert_eq!(qt.points.len(), 4);

        // Insert 5th point (should trigger split)
        qt.insert(Point::new(50.0, 50.0));

        assert!(qt.divided);
        // Points moved to children, root points empty
        assert_eq!(qt.points.len(), 0);

        // Check children (indirectly)
        let query_all = qt.query(&boundary);
        assert_eq!(query_all.len(), 5);
    }

    #[test]
    fn test_coincident_points_terminate() {
        // Regression: inserting more coincident (identical) points than
        // capacity must not recurse forever / overflow the stack.
        let boundary = AABB::new(Point::new(0.0, 0.0), 100.0);
        let mut qt = Quadtree::new(boundary, 1); // Capacity 1

        let p = Point::new(5.0, 5.0);
        assert!(qt.insert(p));
        assert!(qt.insert(p));
        assert!(qt.insert(p)); // 3rd identical point, capacity 1

        // A range query covering the point must find all three.
        let range = AABB::new(Point::new(5.0, 5.0), 1.0);
        let found = qt.query(&range);
        assert_eq!(found.len(), 3);
        assert!(found.iter().all(|q| *q == p));
    }

    #[test]
    fn test_quadtree_query() {
        let boundary = AABB::new(Point::new(0.0, 0.0), 100.0);
        let mut qt = Quadtree::new(boundary, 1); // Capacity 1 to force deep split

        qt.insert(Point::new(10.0, 10.0));
        qt.insert(Point::new(20.0, 20.0));
        qt.insert(Point::new(-10.0, -10.0));

        // Query top-right quadrant
        let range = AABB::new(Point::new(50.0, 50.0), 50.0); // 0..100
        let found = qt.query(&range);

        assert_eq!(found.len(), 2);
        assert!(found.contains(&Point::new(10.0, 10.0)));
        assert!(found.contains(&Point::new(20.0, 20.0)));
        assert!(!found.contains(&Point::new(-10.0, -10.0)));
    }
}
