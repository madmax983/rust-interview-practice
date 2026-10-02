# ADR 0003: Export real `rip_*` C symbols from the library for the FFI drills

**Date:** 2026-10-02
**Status:** Accepted

## Context

`src/fundamentals/ffi.rs` drills both sides of the C boundary: calling a C library, and
exporting a C API that C code could call. There is no C compiler step in this repo. There is
no `build.rs`, no `cc` crate, and the only always-on dependency is `crc32fast`. The drills
still need to cross a real ABI boundary, not just call Rust functions that happen to be
marked `extern "C"`.

## Decision

- The **consumer** side calls libc functions that std already links on every tier-1 target
  (`strlen` and `qsort`). No extra linking is needed.
- The **producer** side is a small C API written in Rust. Each function is `extern "C"` (or
  `"C-unwind"`) with `#[unsafe(no_mangle)]`, and every symbol has the `rip_` prefix
  (rust-interview-practice). One of them, `rip_add_wrapping`, is imported again through an
  `unsafe extern "C"` block with `#[link_name]`, so the call goes through the linker.
- Every exported function is `unsafe extern "C" fn` when it takes a pointer, with a
  `# Safety` section. Fallible functions run inside `ffi_guard`, which returns an
  `FfiStatus` code and records an errno-style, thread-local last-error message.
- The test that a panic in an `extern "C"` fn aborts re-runs the test binary in a child
  process (`RIP_FFI_ABORT_CHILD=1`). It checks for `SIGABRT` and the message "panic in a
  function that cannot unwind". In-process, the abort would kill the whole test run.

## Rationale

`#[no_mangle]` puts symbols into one global namespace. If two crates in the same link export
the same name, the result is undefined behaviour. That is why Rust 2024 makes the attribute
unsafe. A crate-specific prefix makes a clash unlikely. The prefix is also what a real
cbindgen-exported library does. The alternatives were weaker:

- A `build.rs` with the `cc` crate would add a dependency, a C toolchain requirement on CI,
  and a separate language to practise. That works against the gittype goal.
- Plain `extern "C" fn` items without `no_mangle` exercise the calling convention but not
  symbol export, `link_name` or the linker. The drill would lose its point.

## Consequences

- The `rlib` exports 28 `rip_*` symbols. A downstream crate that links this one must
  not define symbols with the same names.
- Miri runs these tests (`cargo +nightly miri test --lib fundamentals::ffi`). It resolves
  `rip_add_wrapping` through the `extern` block. Three tests have `#[cfg_attr(miri, ignore)]`:
  the libc `strlen` and `qsort` calls, and the child-process abort test.
- The abort test depends on the standard test harness's `--exact` filter and the module path
  `fundamentals::ffi::tests::test_abort_child_process`. If the test is renamed or moved, the
  string in `test_panic_in_extern_c_aborts_the_process` has to change too.
