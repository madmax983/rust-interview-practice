**[Pre-allocation Math Panics]
**Learning:** When using mathematical bounds to pre-allocate capacity (e.g., `target / min_val`), edge cases like negative targets or zero-value minimums can cause cast-to-`usize` underflows or division-by-zero panics, leading to runtime failures instead of performance wins.
**Action:** Always clamp mathematical capacity bounds with `.max(0)` on numerators and `.max(1)` on denominators to guarantee safe, positive, non-zero divisors.
