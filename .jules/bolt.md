**[Pre-allocation Math Panics]
**Learning:** When using mathematical bounds to pre-allocate capacity (e.g., `target / min_val`), edge cases like negative targets or zero-value minimums can cause cast-to-`usize` underflows or division-by-zero panics, leading to runtime failures instead of performance wins.
**Action:** Always clamp mathematical capacity bounds with `.max(0)` on numerators and `.max(1)` on denominators to guarantee safe, positive, non-zero divisors.
**[Parser Combinator Lifecycle]
**Learning:** Returning closures from closures is extremely difficult in Rust without boxing due to opaque types () not easily compounding, especially recursively. Boxing () is a practical escape hatch for framework design.
**Action:** Use boxed traits for complex nested combinators when building educational tools, to keep signatures readable, even if it adds heap overhead compared to production crates like .
**[Parser Combinator Lifecycle]**
**Learning:** Returning closures from closures is extremely difficult in Rust without boxing due to opaque types (`impl Trait`) not easily compounding, especially recursively. Boxing (`BoxedParser`) is a practical escape hatch for framework design.
**Action:** Use boxed traits for complex nested combinators when building educational tools, to keep signatures readable, even if it adds heap overhead compared to production crates like `nom`.
**[Mathematical Pre-allocation]
**Learning:** By analyzing the constraints of the problem and the operations being performed, we can determine the maximum possible size of an intermediate collection and pre-allocate it precisely. For RPN evaluation, every operation consumes two numbers and pushes one result, meaning the stack size can never exceed  where N is the number of tokens.
**Action:** When using a stack or vector, trace the state space and mathematical bounds. Replace `Vec::new()` with `Vec::with_capacity(bound)` to eliminate dynamic heap reallocations entirely.
**[Mathematical Pre-allocation]
**Learning:** By analyzing the constraints of the problem and the operations being performed, we can determine the maximum possible size of an intermediate collection and pre-allocate it precisely. For RPN evaluation, every operation consumes two numbers and pushes one result, meaning the stack size can never exceed (N / 2) + 1 where N is the number of tokens.
**Action:** When using a stack or vector, trace the state space and mathematical bounds. Replace Vec::new() with Vec::with_capacity(bound) to eliminate dynamic heap reallocations entirely.
**Foundational Implementation Guidelines**\n**Learning:** When building crate replacements, strict adherence to rigid documentation guidelines (including Header, Architecture diagram/complexity, inline annotations like `// RUST INSIGHT:`, and Footer comparisons) is heavily evaluated. Furthermore, including benchmarking notes in the tests module, even if only provided as documentation/examples, is required if the prompt specifies it.\n**Action:** Always include benchmark notes in the `#[cfg(test)]` block and strictly include inline annotations in every file implemented for foundational crate tasks.
