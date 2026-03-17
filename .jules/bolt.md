
**[Strings: Longest Substring Without Repeating Characters Optimization]**
**Learning:** For problems with explicitly guaranteed ASCII-only string constraints, iterating with `.chars().collect::<Vec<char>>()` introduces unnecessary allocation and UTF-8 decoding overhead. Additionally, using a `HashMap` for tracking byte occurrences is slower than indexing a simple `[-1i32; 256]` state array.
**Action:** Always verify string constraint guarantees. If constraints permit, prefer using `.as_bytes().iter()` combined with fixed-size tracking arrays over `Vec<char>` and `HashMap` to achieve significant performance gains via zero-allocation and continuous memory layout.
