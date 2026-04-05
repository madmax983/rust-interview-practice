**[Pre-allocation Math Panics]
**Learning:** When using mathematical bounds to pre-allocate capacity (e.g., `target / min_val`), edge cases like negative targets or zero-value minimums can cause cast-to-`usize` underflows or division-by-zero panics, leading to runtime failures instead of performance wins.
**Action:** Always clamp mathematical capacity bounds with `.max(0)` on numerators and `.max(1)` on denominators to guarantee safe, positive, non-zero divisors.
**[Parser Combinator Lifecycle]
**Learning:** Returning closures from closures is extremely difficult in Rust without boxing due to opaque types () not easily compounding, especially recursively. Boxing () is a practical escape hatch for framework design.
**Action:** Use boxed traits for complex nested combinators when building educational tools, to keep signatures readable, even if it adds heap overhead compared to production crates like .
**[Parser Combinator Lifecycle]**
**Learning:** Returning closures from closures is extremely difficult in Rust without boxing due to opaque types (`impl Trait`) not easily compounding, especially recursively. Boxing (`BoxedParser`) is a practical escape hatch for framework design.
**Action:** Use boxed traits for complex nested combinators when building educational tools, to keep signatures readable, even if it adds heap overhead compared to production crates like `nom`.
