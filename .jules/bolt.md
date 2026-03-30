**[Longest Common Prefix Allocation Removal]
**Learning:** By using `.into_iter()` on an owned `Vec<String>`, you can take ownership of the first `String` and repeatedly mutate it in place using `.truncate()`. This safely prevents any heap allocations for the result while building the prefix, without running into borrow checker issues compared to using references or intermediate slices.
**Action:** Whenever folding an iterator of owned strings to produce an owned result, look for opportunities to mutate the first owned item in place via `truncate` or similar methods instead of creating new heap-allocated strings.

**[Spiral Matrix Collection Allocation Removal]**
**Learning:** Custom complex iterators rarely implement `size_hint` or `ExactSizeIterator` correctly. Calling `.collect()` on such iterators causes multiple intermediate memory reallocations as the buffer grows.
**Action:** When the exact target size is known beforehand (like `m * n` for a matrix), pre-allocate the target `Vec` using `Vec::with_capacity(capacity)` and use `.extend()` to avoid all intermediate reallocations.

**[String Allocation vs Unicode Correctness]**
**Learning:** Blindly replacing `s.chars().collect::<Vec<char>>()` with `s.as_bytes()` to remove a heap allocation completely breaks algorithms when dealing with non-ASCII text, because a `char` is a 4-byte scalar value while UTF-8 characters have variable byte lengths.
**Action:** Only swap character iteration for byte slice manipulation (`&[u8]`) if the algorithm constraints explicitly guarantee strictly ASCII input. Otherwise, the performance optimization is a functional regression.
