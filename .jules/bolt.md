**Zero-allocation string processing in Word Search**
**Learning:** `word.chars().collect::<Vec<char>>()` introduces an O(L) heap allocation that can be completely eliminated if the problem guarantees ASCII inputs.
**Action:** Use `word.as_bytes()` and cast characters `as u8` for O(1) comparison in hot recursive loops like DFS to avoid intermediate allocations and UTF-8 decoding overhead.
