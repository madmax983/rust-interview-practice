**[O(1) node_count for ConsistentHashRing]**
**Learning:** Recomputing unique items across a virtualized mapping by allocating a `HashSet` per call can be a hidden bottleneck, especially when the ring size scales up.
**Action:** When a metric like "unique physical nodes" is required from a ring of virtual nodes, explicitly track the count via integer state (`physical_node_count`) during insertions and removals to enable $O(1)$ zero-allocation reads. Always remember to `.saturating_sub(1)` when decrementing to prevent theoretical underflows.
