**[SlotMap Retention Bottleneck]**
**Learning:** The `retain` method in `SlotMap` uses `O(N)` key lookup inside a loop (`self.slots.get_mut`), and then potentially calls `remove` which also does a lookup. This is extremely inefficient since it's already iterating over the indices. It also instantiates temporary keys for checking, and potentially mutates the array concurrently via nested calls.
**Action:** Refactor `retain` to directly access and mutate the slot array elements during iteration, avoiding redundant lookups and method calls.

**[SlotMap Retention Bottleneck]**
**Learning:** The `retain` method in `SlotMap` used redundant `O(1)` key lookups inside a loop (`self.slots.get_mut`), and then potentially called `remove` which also did a lookup. This is extremely inefficient due to bounds checking and redundant memory accesses.
**Action:** Refactor `retain` to directly access and mutate the slot array elements during iteration, avoiding redundant lookups and method calls, providing a constant-factor speedup.
