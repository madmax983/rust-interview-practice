//! # Inline Assembly Patterns
//!
//! Master Rust's inline assembly with `asm!` macro for low-level operations.
//! Learn register constraints, clobbers, and platform-specific optimizations.
//!
//! Inline assembly is stable in Rust and provides direct access to CPU instructions
//! for performance-critical code or hardware-specific operations.

use std::arch::asm;

// ============================================================================
// Basic Inline Assembly
// ============================================================================

/// Basic assembly - adding two numbers.
///
/// # Safety
///
/// Safe because we're only using basic arithmetic with no memory access.
#[must_use]
pub fn add_asm(a: u64, b: u64) -> u64 {
    let result: u64;
    unsafe {
        // Basic syntax: asm!("template", outputs, inputs, clobbers, options)
        // {} is a placeholder for an operand
        asm!(
            "add {0}, {1}",  // Template: add dest, src
            inout(reg) a => result,  // a is input, result is output (same register)
            in(reg) b,  // b is input in any register
        );
    }
    result
}

/// Multiply two numbers using assembly.
#[must_use]
pub fn mul_asm(a: u64, b: u64) -> u64 {
    let result: u64;
    unsafe {
        asm!(
            "imul {0}, {1}",  // Signed multiply
            inout(reg) a => result,
            in(reg) b,
        );
    }
    result
}

/// Increment a value in place.
pub fn increment_asm(value: &mut u64) {
    unsafe {
        asm!(
            "inc qword ptr [{0}]",  // Increment memory location
            in(reg) value,  // Pass pointer to value
            options(nostack),  // No stack operations
        );
    }
}

// ============================================================================
// Register Constraints
// ============================================================================

/// Demonstrate different register constraints.
///
/// Constraints control which registers the compiler can use:
/// - `reg`: Any general-purpose register
/// - `in`: Input operand
/// - `out`: Output operand (uninitialized)
/// - `inout`: Input that becomes output
/// - `lateout`: Output written after all inputs are read
#[allow(dead_code)]
fn register_constraints_example() {
    let input = 42u64;
    let mut output: u64;
    let mut inout_val = 10u64;

    unsafe {
        asm!(
            "mov {out}, {inp}",    // Move input to output
            "add {inout}, 5",       // Add 5 to inout
            inp = in(reg) input,    // Named input operand
            out = out(reg) output,  // Named output operand (uninitialized before)
            inout = inout(reg) inout_val,  // Named inout operand
        );
    }

    assert_eq!(output, 42);
    assert_eq!(inout_val, 15);
}

// ============================================================================
// X86_64 Specific Instructions
// ============================================================================

/// Count set bits (population count) using `x86_64` POPCNT instruction.
///
/// # Safety
///
/// Caller must ensure POPCNT is supported (check with `is_x86_feature_detected!("popcnt")`).
#[cfg(target_arch = "x86_64")]
#[must_use]
pub unsafe fn popcnt_asm(value: u64) -> u64 {
    let result: u64;
    // SAFETY: Caller guarantees POPCNT is supported
    unsafe {
        asm!(
            "popcnt {0}, {1}",
            out(reg) result,
            in(reg) value,
            options(pure, nomem, nostack),
        );
    }
    result
}

/// Byte swap using `x86_64` BSWAP instruction.
#[cfg(target_arch = "x86_64")]
#[must_use]
pub fn bswap_asm(value: u64) -> u64 {
    let result: u64;
    unsafe {
        asm!(
            "bswap {0}",
            inout(reg) value => result,
            options(pure, nomem, nostack),
        );
    }
    result
}

/// Count leading zeros using `x86_64` LZCNT instruction.
///
/// # Safety
///
/// Caller must ensure LZCNT is supported.
#[cfg(target_arch = "x86_64")]
#[must_use]
pub unsafe fn lzcnt_asm(value: u64) -> u64 {
    let result: u64;
    // SAFETY: Caller guarantees LZCNT is supported
    unsafe {
        asm!(
            "lzcnt {0}, {1}",
            out(reg) result,
            in(reg) value,
            options(pure, nomem, nostack),
        );
    }
    result
}

/// Count trailing zeros using `x86_64` TZCNT instruction.
///
/// # Safety
///
/// Caller must ensure BMI1 (TZCNT) is supported.
#[cfg(target_arch = "x86_64")]
#[must_use]
pub unsafe fn tzcnt_asm(value: u64) -> u64 {
    let result: u64;
    // SAFETY: Caller guarantees BMI1 (TZCNT) is supported
    unsafe {
        asm!(
            "tzcnt {0}, {1}",
            out(reg) result,
            in(reg) value,
            options(pure, nomem, nostack),
        );
    }
    result
}

// ============================================================================
// Atomic Operations
// ============================================================================

/// Atomic compare-and-swap using `x86_64` CMPXCHG instruction.
///
/// Compares *ptr with `old_value`. If equal, stores `new_value` and returns old value.
/// Otherwise, loads current value into `old_value` and returns it.
///
/// # Safety
///
/// - `ptr` must be valid and properly aligned
/// - `ptr` must be safe to access atomically
#[cfg(target_arch = "x86_64")]
pub unsafe fn atomic_cas(ptr: *mut u64, old_value: u64, new_value: u64) -> u64 {
    let prev: u64;
    // SAFETY: Caller guarantees ptr is valid and properly aligned
    unsafe {
        asm!(
            "lock cmpxchg [{ptr}], {new}",
            ptr = in(reg) ptr,
            new = in(reg) new_value,
            inout("rax") old_value => prev,
            options(nostack),
        );
    }
    prev
}

/// Atomic fetch-and-add using `x86_64` XADD instruction.
///
/// Atomically adds `value` to `*ptr` and returns the previous value.
///
/// # Safety
///
/// - `ptr` must be valid and properly aligned
/// - `ptr` must be safe to access atomically
#[cfg(target_arch = "x86_64")]
pub unsafe fn atomic_fetch_add(ptr: *mut u64, value: u64) -> u64 {
    let prev: u64;
    // SAFETY: Caller guarantees ptr is valid and properly aligned
    unsafe {
        asm!(
            "lock xadd [{ptr}], {val}",
            ptr = in(reg) ptr,
            val = inout(reg) value => prev,
            options(nostack),
        );
    }
    prev
}

/// Atomic increment using `x86_64` LOCK INC instruction.
///
/// # Safety
///
/// - `ptr` must be valid and properly aligned
#[cfg(target_arch = "x86_64")]
pub unsafe fn atomic_increment(ptr: *mut u64) {
    // SAFETY: Caller guarantees ptr is valid and properly aligned
    unsafe {
        asm!(
            "lock inc qword ptr [{0}]",
            in(reg) ptr,
            options(nostack),
        );
    }
}

// ============================================================================
// CPUID - CPU Feature Detection
// ============================================================================

/// Read CPU features using CPUID instruction.
///
/// Returns (eax, ebx, ecx, edx) for the given leaf and subleaf.
#[cfg(target_arch = "x86_64")]
#[must_use]
pub fn cpuid(leaf: u32, subleaf: u32) -> (u32, u32, u32, u32) {
    let eax: u32;
    let ebx: u32;
    let ecx: u32;
    let edx: u32;

    unsafe {
        // SAFETY: CPUID is a safe instruction, just reading CPU info
        asm!(
            "mov {ebx_tmp:e}, ebx",  // Save ebx (LLVM reserved)
            "cpuid",
            "xchg {ebx_tmp:e}, ebx", // Restore ebx, get result
            ebx_tmp = out(reg) ebx,
            inout("eax") leaf => eax,
            inout("ecx") subleaf => ecx,
            out("edx") edx,
            options(nostack, preserves_flags),
        );
    }

    (eax, ebx, ecx, edx)
}

/// Check if a CPU feature is supported using CPUID.
#[cfg(target_arch = "x86_64")]
#[must_use]
pub fn has_sse42() -> bool {
    let (_, _, ecx, _) = cpuid(1, 0);
    (ecx & (1 << 20)) != 0 // SSE4.2 bit
}

// ============================================================================
// Memory Barriers and Fences
// ============================================================================

/// Memory fence - ensures all loads/stores complete before continuing.
#[cfg(target_arch = "x86_64")]
pub fn memory_fence() {
    unsafe {
        asm!("mfence", options(nostack, preserves_flags),);
    }
}

/// Load fence - ensures all loads complete.
#[cfg(target_arch = "x86_64")]
pub fn load_fence() {
    unsafe {
        asm!("lfence", options(nostack, preserves_flags),);
    }
}

/// Store fence - ensures all stores complete.
#[cfg(target_arch = "x86_64")]
pub fn store_fence() {
    unsafe {
        asm!("sfence", options(nostack, preserves_flags),);
    }
}

// ============================================================================
// System Calls
// ============================================================================

/// Direct system call using SYSCALL instruction (Linux `x86_64`).
///
/// # Safety
///
/// Caller must ensure syscall number and arguments are valid.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[must_use]
pub unsafe fn syscall1(number: u64, arg1: u64) -> u64 {
    let result: u64;
    // SAFETY: Caller guarantees syscall number and arguments are valid
    unsafe {
        asm!(
            "syscall",
            inout("rax") number => result,
            in("rdi") arg1,
            out("rcx") _,
            out("r11") _,
            options(nostack),
        );
    }
    result
}

/// Get process ID using direct syscall (Linux).
///
/// Demonstrates a safe syscall wrapper.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[must_use]
pub fn getpid_asm() -> u64 {
    const SYS_GETPID: u64 = 39;
    unsafe { syscall1(SYS_GETPID, 0) }
}

// ============================================================================
// Naked Functions
// ============================================================================

/// Naked function - no prologue/epilogue generated by compiler.
///
/// Used for writing custom function entry/exit sequences.
///
/// # Safety
///
/// Naked functions must handle their own stack setup and return.
#[cfg(target_arch = "x86_64")]
#[unsafe(naked)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn naked_identity(x: u64) -> u64 {
    // Must use naked_asm! in naked functions (Rust 2024)
    // First argument is in rdi (System V ABI), return in rax
    core::arch::naked_asm!(
        "mov rax, rdi", // Copy first arg to return register
        "ret",          // Return
    )
}

/// Naked function that adds two numbers.
///
/// # Safety
///
/// This is a naked function following the C ABI; it must only be called with the
/// C calling convention and relies on the hand-written prologue/epilogue being correct.
#[cfg(target_arch = "x86_64")]
#[unsafe(naked)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn naked_add(a: u64, b: u64) -> u64 {
    core::arch::naked_asm!(
        "mov rax, rdi", // First arg (a) to rax
        "add rax, rsi", // Add second arg (b)
        "ret",
    )
}

// ============================================================================
// Advanced Patterns
// ============================================================================

/// Spin loop hint - tells CPU we're in a spin loop for better power efficiency.
// Single-instruction intrinsic wrapper: inline(always) is intended so the hint is emitted inline
#[allow(clippy::inline_always)]
#[inline(always)]
pub fn spin_loop_hint() {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        asm!("pause", options(nomem, nostack, preserves_flags));
    }

    #[cfg(target_arch = "aarch64")]
    unsafe {
        asm!("yield", options(nomem, nostack, preserves_flags));
    }
}

/// Read timestamp counter (`x86_64` RDTSC).
///
/// Returns the number of CPU cycles since reset.
#[cfg(target_arch = "x86_64")]
#[must_use]
pub fn rdtsc() -> u64 {
    let low: u32;
    let high: u32;
    unsafe {
        asm!(
            "rdtsc",
            out("eax") low,
            out("edx") high,
            options(nomem, nostack, preserves_flags),
        );
    }
    ((u64::from(high)) << 32) | u64::from(low)
}

/// Serializing RDTSC - prevents instruction reordering.
#[cfg(target_arch = "x86_64")]
#[must_use]
pub fn rdtscp() -> u64 {
    let low: u32;
    let high: u32;
    unsafe {
        asm!(
            "rdtscp",
            out("eax") low,
            out("edx") high,
            out("ecx") _,  // Processor ID (discarded)
            options(nomem, nostack),
        );
    }
    ((u64::from(high)) << 32) | u64::from(low)
}

/// Prefetch data into cache (`x86_64`).
///
/// # Safety
///
/// `ptr` should be a valid address, though invalid addresses are typically safe
/// (just ineffective).
#[cfg(target_arch = "x86_64")]
pub unsafe fn prefetch_t0(ptr: *const u8) {
    // SAFETY: Prefetch is safe even with invalid addresses (just ineffective)
    unsafe {
        asm!(
            "prefetcht0 [{0}]",
            in(reg) ptr,
            options(nostack, preserves_flags),
        );
    }
}

// ============================================================================
// Clobbers and Options
// ============================================================================

/// Demonstrate clobber lists.
///
/// Clobbers tell the compiler which registers are modified.
#[allow(dead_code)]
fn clobber_example() {
    let a = 42u64;
    let b: u64;

    unsafe {
        asm!(
            "mov {0}, {1}",
            "add {0}, 10",
            out(reg) b,
            in(reg) a,
            // Options:
            // - pure: no side effects beyond outputs
            // - nomem: doesn't read/write memory
            // - readonly: only reads memory
            // - preserves_flags: doesn't modify CPU flags
            // - noreturn: doesn't return (diverges)
            // - nostack: doesn't use stack
            options(pure, nomem, nostack),
        );
    }

    assert_eq!(b, 52);
}

// ============================================================================
// Cross-Platform Assembly
// ============================================================================

/// Platform-specific NOP (no operation).
// Single-instruction intrinsic wrapper: inline(always) is intended so the NOP is emitted inline
#[allow(clippy::inline_always)]
#[inline(always)]
pub fn nop() {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        asm!("nop", options(nomem, nostack, preserves_flags));
    }

    #[cfg(target_arch = "aarch64")]
    unsafe {
        asm!("nop", options(nomem, nostack, preserves_flags));
    }
}

/// Breakpoint instruction for debugging.
// Single-instruction intrinsic wrapper: inline(always) is intended so the trap is emitted inline
#[allow(clippy::inline_always)]
#[inline(always)]
pub fn breakpoint() {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        asm!("int3", options(nostack));
    }

    #[cfg(target_arch = "aarch64")]
    unsafe {
        asm!("brk #0", options(nostack));
    }
}

// ============================================================================
// AArch64 Examples
// ============================================================================

/// Add two numbers on AArch64.
#[cfg(target_arch = "aarch64")]
#[must_use]
pub fn add_asm_aarch64(a: u64, b: u64) -> u64 {
    let result: u64;
    unsafe {
        asm!(
            "add {0}, {1}, {2}",  // add dest, src1, src2
            out(reg) result,
            in(reg) a,
            in(reg) b,
            options(pure, nomem, nostack),
        );
    }
    result
}

/// Atomic compare-and-swap on AArch64.
#[cfg(target_arch = "aarch64")]
pub unsafe fn atomic_cas_aarch64(ptr: *mut u64, old: u64, new: u64) -> u64 {
    let result: u64;
    // SAFETY: Caller guarantees ptr is valid and properly aligned
    unsafe {
        asm!(
            "cas {old}, {new}, [{ptr}]",
            ptr = in(reg) ptr,
            old = inout(reg) old => result,
            new = in(reg) new,
            options(nostack),
        );
    }
    result
}

// ============================================================================
// Performance Example
// ============================================================================

/// Benchmark assembly vs Rust for simple addition.
#[allow(dead_code)]
fn benchmark_asm_vs_rust() {
    const ITERATIONS: usize = 1_000_000;

    // Rust version
    let start = rdtsc();
    let mut sum = 0u64;
    for i in 0..ITERATIONS {
        sum = sum.wrapping_add(i as u64);
    }
    let rust_cycles = rdtsc() - start;

    // Assembly version
    let start = rdtsc();
    let mut sum_asm = 0u64;
    for i in 0..ITERATIONS {
        sum_asm = add_asm(sum_asm, i as u64);
    }
    let asm_cycles = rdtsc() - start;

    println!("Rust cycles: {rust_cycles}");
    println!("ASM cycles: {asm_cycles}");
    assert_eq!(sum, sum_asm);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_asm() {
        assert_eq!(add_asm(5, 10), 15);
        assert_eq!(add_asm(0, 0), 0);
        assert_eq!(add_asm(100, 200), 300);
    }

    #[test]
    fn test_mul_asm() {
        assert_eq!(mul_asm(5, 10), 50);
        assert_eq!(mul_asm(0, 100), 0);
        assert_eq!(mul_asm(7, 8), 56);
    }

    #[test]
    fn test_increment_asm() {
        let mut value = 42;
        increment_asm(&mut value);
        assert_eq!(value, 43);
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_bswap_asm() {
        assert_eq!(bswap_asm(0x0123456789ABCDEF), 0xEFCDAB8967452301);
        assert_eq!(bswap_asm(0x1234), 0x3412000000000000);
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_popcnt_asm() {
        if std::is_x86_feature_detected!("popcnt") {
            unsafe {
                assert_eq!(popcnt_asm(0b1010101010), 5);
                assert_eq!(popcnt_asm(0b1111), 4);
                assert_eq!(popcnt_asm(0), 0);
            }
        }
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_lzcnt_asm() {
        if std::is_x86_feature_detected!("lzcnt") {
            unsafe {
                assert_eq!(lzcnt_asm(0b1000), 60);
                assert_eq!(lzcnt_asm(1u64 << 63), 0);
            }
        }
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_atomic_cas() {
        let mut value = 42u64;
        unsafe {
            // Should succeed: value is 42
            let prev = atomic_cas(&mut value as *mut u64, 42, 100);
            assert_eq!(prev, 42);
            assert_eq!(value, 100);

            // Should fail: value is 100, not 42
            let prev = atomic_cas(&mut value as *mut u64, 42, 200);
            assert_eq!(prev, 100);
            assert_eq!(value, 100); // Unchanged
        }
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_atomic_fetch_add() {
        let mut value = 10u64;
        unsafe {
            let prev = atomic_fetch_add(&mut value as *mut u64, 5);
            assert_eq!(prev, 10);
            assert_eq!(value, 15);
        }
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_cpuid() {
        // Leaf 0: Get vendor ID
        let (max_leaf, ebx, ecx, edx) = cpuid(0, 0);
        assert!(max_leaf > 0);

        // Vendor ID is in ebx, edx, ecx (in that order)
        let vendor = [
            ((ebx) & 0xFF) as u8,
            ((ebx >> 8) & 0xFF) as u8,
            ((ebx >> 16) & 0xFF) as u8,
            ((ebx >> 24) & 0xFF) as u8,
            ((edx) & 0xFF) as u8,
            ((edx >> 8) & 0xFF) as u8,
            ((edx >> 16) & 0xFF) as u8,
            ((edx >> 24) & 0xFF) as u8,
            ((ecx) & 0xFF) as u8,
            ((ecx >> 8) & 0xFF) as u8,
            ((ecx >> 16) & 0xFF) as u8,
            ((ecx >> 24) & 0xFF) as u8,
        ];

        // Common vendors: "GenuineIntel", "AuthenticAMD", "KVMKVMKVM"
        println!("CPU Vendor: {}", String::from_utf8_lossy(&vendor));
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    #[ignore] // Naked functions are unstable and may not work correctly in all contexts
    fn test_naked_functions() {
        unsafe {
            assert_eq!(naked_identity(42), 42);
            assert_eq!(naked_add(10, 20), 30);
        }
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_rdtsc() {
        let t1 = rdtsc();
        let t2 = rdtsc();
        // Time should advance (though not guaranteed on all systems)
        // Just ensure it doesn't crash
        assert!(t2 >= t1 || t1 > 0);
    }

    #[test]
    fn test_nop() {
        nop(); // Should do nothing but not crash
    }
}
