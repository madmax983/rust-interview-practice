**[Avoiding Vec clones for Immutable Graph Iteration]
**Learning:** RefCell allows multiple simultaneous immutable borrows. It is safe to hold `&node.borrow().neighbors` directly across a recursive call, provided we only ever use `borrow()` (read access) on those original graph nodes and never `borrow_mut()` them during the traversal.
**Action:** Always verify if a `Vec` inside a `RefCell` is actually being mutated during traversal before defensively cloning it to appease the borrow checker.

**[Replacing contains+clone+remove with take in HashSets]
**Learning:** When needing to conditionally remove and consume a dynamically allocated type (like `Vec<u8>` or `String`) from a `HashSet`, using `.contains(&val)` followed by `.clone()` and `.remove(&val)` is extremely inefficient.
**Action:** Use `if let Some(owned_val) = set.take(&val)` instead to extract the owned value in a single zero-allocation lookup.
