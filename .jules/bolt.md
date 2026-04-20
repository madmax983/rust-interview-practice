**[Optimizing Combinations Capacity]
**Learning:** Initializing `Vec::new()` for dynamically sized but mathematically determinable collections results in multiple heap reallocations.
**Action:** Always consider pre-calculating the exact capacity (e.g. `C(n, k)` for combinations) and using `Vec::with_capacity(capacity)` to eliminate reallocation overhead entirely, turning array building into a zero-allocation operation during execution.
