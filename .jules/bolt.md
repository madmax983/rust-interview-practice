**Pre-allocate vectors in stream processing**
**Learning:** When building collections (like rows in a CSV) iteratively, if the elements (rows) typically have a similar length, tracking the length of the previous element to use `Vec::with_capacity(previous_len)` avoids multiple reallocation steps as the vector grows.
**Action:** Identify stream processing or iterator implementations where a new vector is created for each item (`let mut vec = Vec::new()`) and modify them to track expected sizes (e.g. `self.expected_len`) to pre-allocate vector capacities.
