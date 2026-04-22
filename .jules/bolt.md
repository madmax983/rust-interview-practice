**[Template Engine Scope Refactor]**
**Learning:** Trying to use a standard enum (`RenderContext<'a>`) where the `Scoped` variant stores a reference to its own type `&'a RenderContext<'a>` forces the parent reference to live for the entire lifetime `'a` (which is typically the lifetime of the input data). This fundamentally fails the borrow checker when you build the linked list of contexts using short-lived local variables inside a `for` loop, because the parent `root_context` drops at the end of the method, and inner `scoped_context` instances drop at the end of each loop iteration.
**Action:** When creating a linked list of references for nested scoping, use two separate lifetimes: one for the underlying data (`'data`) and one for the parent reference (`'parent`). Alternatively, avoid the explicit named lifetimes by just passing `&RenderContext` everywhere and letting the compiler elide lifetimes, or use a stack structure (e.g. `Vec<(&str, &Value)>`) passed via mutable reference. I successfully fixed it by relying on lifetime elision `&RenderContext<'_>` in the function signatures and not tying the parent reference lifetime to the data lifetime.
**Zero-allocation string processing in Word Search**
**Learning:** `word.chars().collect::<Vec<char>>()` introduces an O(L) heap allocation that can be completely eliminated if the problem guarantees ASCII inputs.
**Action:** Use `word.as_bytes()` and cast characters `as u8` for O(1) comparison in hot recursive loops like DFS to avoid intermediate allocations and UTF-8 decoding overhead.
**Subarray Sum Equals K**\n**Learning:** The Prefix Sum + HashMap technique can be elegantly expressed functionally in Rust using , passing the HashMap and running states as the accumulator, eliminating all explicit mutable variables.\n**Action:** Consider using  to thread HashMap state in future prefix-sum or DP problems to increase code purity.
**Subarray Sum Equals K**\n**Learning:** The Prefix Sum + HashMap technique can be elegantly expressed functionally in Rust using Iterator fold, passing the HashMap and running states as the accumulator, eliminating all explicit mutable variables.\n**Action:** Consider using fold to thread HashMap state in future prefix-sum or DP problems to increase code purity.
**[Dynamic Error Handling Framework]
**Learning:** Extending `Result` with an extension trait for context chaining requires handling Debug/Display trait bounds properly. The `ContextExt` trait implementation needs to enforce these bounds to satisfy `Error::msg()` requirements.
**Action:** Always ensure that extension trait implementations match or exceed the trait bounds required by the underlying functions they wrap, especially when dealing with type-erased errors like `Box<dyn Error>`.
**[First Missing Positive - Cyclic Sort]**
**Learning:** The "First Missing Positive" problem (LeetCode #41) uses cyclic sorting where the array values map to indices `x - 1`. In Rust, we have to cast `(nums[i] - 1) as usize` to index into the array, which enforces explicit bounds checking.
**Action:** When implementing in-place cyclic sorts or mapping values to indices, always ensure robust bounds checking before casting to `usize` to prevent panics and satisfy Rust's strict safety guarantees.
**Garbage Collector Unsafe Drop Semantics**
**Learning:** Dropping objects during the sweep phase of a custom Garbage Collector is highly unsafe if the objects implement `Drop` and contain cyclic references. If object A and B reference each other and are unreachable, dropping A first might allow B to access A's freed memory during its own `Drop` via a safe `Deref` call. This leads to a Use-After-Free (UAF) bug in entirely "safe" Rust.
**Action:** When implementing custom memory managers or GCs, explicitly handle `Drop` logic. A "production-aware" GC requires enforcing `Drop` constraints via traits (like `Finalize`) or using compiler plugins, and it's critical to document this limitation when implementing a simplified model.
**[String iteration optimization]
**Learning:** Calling `.chars().collect::<Vec<char>>()` on a string slice just to iterate with an index is an anti-pattern that creates an unnecessary O(N) heap allocation. Furthermore, reconstructing the remaining part of the string with `.iter().collect::<String>()` creates a second heap allocation.
**Action:** Use `.char_indices()` to iterate over the string safely, yielding byte offsets and characters. We can then use these byte offsets alongside `char::len_utf8()` to slice the original string (e.g. `&s[idx + c.len_utf8()..]`) to get the remainder without any allocations.
**[Raft State Machine Implementation]
**Learning:** Decoupling network IO from state machine logic allows for exhaustive, fast unit testing without the flakiness of actual networking or async runtime behavior. By modeling Raft strictly as `step(Message)` and `tick() -> OutputMessages`, you can perfectly simulate split brains, partition recovery, and term precedence.
**Action:** Always favor pushing I/O and async behavior to the very edge of the system ("imperative shell, functional core") to maximize the test surface of the core logic.
**[Optimizing Combinations Capacity]
**Learning:** Initializing `Vec::new()` for dynamically sized but mathematically determinable collections results in multiple heap reallocations.
**Action:** Always consider pre-calculating the exact capacity (e.g. `C(n, k)` for combinations) and using `Vec::with_capacity(capacity)` to eliminate reallocation overhead entirely, turning array building into a zero-allocation operation during execution.
**Swap Nodes in Pairs**
**Learning:** Implementing linked list node swapping requires careful use of  and re-borrowing () to manipulate nodes in-place iteratively without violating Rust's single-mutable-reference rule.
**Action:** When iterating through an  chain, maintain a mutable reference to the *location* where the next node should be attached (), rather than trying to hold references to the nodes themselves simultaneously.
**Swap Nodes in Pairs**
**Learning:** Implementing linked list node swapping requires careful use of `Option::take()` and re-borrowing (`&mut`) to manipulate nodes in-place iteratively without violating Rust's single-mutable-reference rule.
**Action:** When iterating through an `Option<Box<Node>>` chain, maintain a mutable reference to the *location* where the next node should be attached (`&mut Option<Box<Node>>`), rather than trying to hold references to the nodes themselves simultaneously.
**Reactive Signals: Deduplication and Exponential Blowups**
**Learning:** Naive Push-based reactive systems that push to a `Vec` inside `Signal::get()` can cause exponential duplication if an Effect evaluates a signal multiple times (or in a loop). This blows up memory and triggers effects recursively.
**Action:** When tracking subscribers, use a `HashMap` keyed by a unique identifier (like an AtomicUsize `EffectId`) to deduplicate subscriptions natively.

**Reactive Signals: Memo Self-Subscription**
**Learning:** If a `Memo` creates an `Effect` that updates its own `Signal`, calling the trait's tracked `get()` method inside that effect will subscribe the effect to its own output, leading to recursive evaluation loops that `RefCell` will catch as panics (or silently swallow if `try_borrow_mut` is used).
**Action:** Within internal reactive primitives, bypass public tracking methods (`get()`) and access raw inner values directly (e.g., `borrow().value.clone()`) to prevent unwanted self-subscription.

**Reactive Signals: Cloning Derived State**
**Learning:** Derived state structures (like `Memo`) usually contain the `Effect` that drives them. Implementing `Clone` manually with `unimplemented!()` is an anti-pattern.
**Action:** Wrap the internal `Effect` in an `Rc` so that the struct can implement `Clone` safely, allowing multiple handles to share the exact same background computation.
