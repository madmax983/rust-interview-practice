//! # Bytecode Virtual Machine Implementation
//!
//! Implements a minimal stack-based virtual machine (VM) with a byte-array Chunk and constant pool.
//!
//! **Replaces Crates:** `rhai`, `rune`, `mlua`
//!
//! **Real-world Usage:**
//! - Embedded scripting in game engines (e.g., Lua)
//! - Dynamic language runtimes (e.g., Python, Ruby, JavaScript)
//! - Smart contract execution environments (e.g., EVM, WASM)
//!
//! **Why build it yourself?**
//! Building a VM teaches you how dynamic languages are executed, how compilers target a bytecode
//! instruction set, and how a stack-based architecture handles operations without registers. You'll learn
//! about the instruction dispatch loop, managing an execution stack, and how to safely access bytecode memory.

use std::fmt;

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure Diagram:
//
//       VM State               Chunk
//     ┌─────────┐           ┌─────────────┐
//     │ Stack   │           │ Code (bytes)│
//     │ ┌─────┐ │           │ ┌─────────┐ │
//     │ │ val │ │           │ │ OP_PUSH │ │
//     │ ├─────┤ │           │ ├─────────┤ │
//     │ │ val │ │  <────    │ │ OP_ADD  │ │
//     │ └─────┘ │           │ └─────────┘ │
//     │ IP: ptr │ ───────>  │             │
//     └─────────┘           │ Constants   │
//                           │ ┌─────────┐ │
//                           │ │ "foo"   │ │
//                           │ └─────────┘ │
//                           └─────────────┘
//
// Invariants:
// 1. IP (Instruction Pointer) must always point within the bounds of the chunk's code array.
// 2. Stack operations must check for underflow (popping an empty stack).
// 3. Stack depth must not exceed the maximum allowed depth (overflow).
// 4. Operand reads from bytecode must be bounds-checked to avoid panics on malformed bytecode.
//
// Complexity:
// ┌─────────────┬────────┬────────┐
// │ Operation   │ Time   │ Space  │
// ├─────────────┼────────┼────────┤
// │ Instruction │ O(1)   │ O(1)   │
// │ Push/Pop    │ O(1)   │ O(1)*  │
// └─────────────┴────────┴────────┘
// * Stack space is pre-allocated up to a maximum depth.
//
// Design Decisions & Tradeoffs:
// - Stack-based vs. Register-based: Stack-based is simpler to compile to and implement, though it typically requires more instructions to perform the same task as a register-based VM.
// - Value Representation: We use a simple enum for values. A production VM would use NaN-boxing or a more compact representation for performance.

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Number(f64),
    Boolean(bool),
    Nil,
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Number(n) => write!(f, "{}", n),
            Value::Boolean(b) => write!(f, "{}", b),
            Value::Nil => write!(f, "nil"),
        }
    }
}

// RUST INSIGHT: OpCode could be an enum, but using u8 directly allows for a denser bytecode format,
// which is standard practice for VMs. We cast them to and from u8.
pub const OP_RETURN: u8 = 0;
pub const OP_CONSTANT: u8 = 1;
pub const OP_NEGATE: u8 = 2;
pub const OP_ADD: u8 = 3;
pub const OP_SUBTRACT: u8 = 4;
pub const OP_MULTIPLY: u8 = 5;
pub const OP_DIVIDE: u8 = 6;
pub const OP_NIL: u8 = 7;
pub const OP_TRUE: u8 = 8;
pub const OP_FALSE: u8 = 9;
pub const OP_NOT: u8 = 10;
pub const OP_EQUAL: u8 = 11;

#[derive(Debug, Default)]
pub struct Chunk {
    pub code: Vec<u8>,
    pub constants: Vec<Value>,
    // In a real VM, we would also store line number information here for debugging.
}

impl Chunk {
    pub fn new() -> Self {
        Self {
            code: Vec::new(),
            constants: Vec::new(),
        }
    }

    pub fn write(&mut self, byte: u8) {
        self.code.push(byte);
    }

    pub fn add_constant(&mut self, value: Value) -> usize {
        self.constants.push(value);
        self.constants.len() - 1
    }
}

#[derive(Debug, PartialEq)]
pub enum VmError {
    CompileError,
    RuntimeError(String),
    UnexpectedEof,
}

pub trait Interpreter {
    fn interpret(&mut self, chunk: &Chunk) -> Result<(), VmError>;
}

pub struct Vm {
    stack: Vec<Value>,
    // RUST INSIGHT: A fixed maximum stack size prevents accidental out-of-memory errors
    // from infinite recursion or loops in the guest language.
    max_stack_size: usize,
}

impl Vm {
    pub fn new(max_stack_size: usize) -> Self {
        Self {
            // Pre-allocate the stack to avoid reallocations during execution
            stack: Vec::with_capacity(max_stack_size),
            max_stack_size,
        }
    }

    pub fn reset_stack(&mut self) {
        self.stack.clear();
    }

    fn push(&mut self, value: Value) -> Result<(), VmError> {
        if self.stack.len() >= self.max_stack_size {
            return Err(VmError::RuntimeError("Stack overflow".to_string()));
        }
        self.stack.push(value);
        Ok(())
    }

    fn pop(&mut self) -> Result<Value, VmError> {
        self.stack.pop().ok_or_else(|| VmError::RuntimeError("Stack underflow".to_string()))
    }

    #[allow(dead_code)]
    fn peek(&self, distance: usize) -> Option<&Value> {
        if self.stack.len() > distance {
            Some(&self.stack[self.stack.len() - 1 - distance])
        } else {
            None
        }
    }
}

impl Interpreter for Vm {
    fn interpret(&mut self, chunk: &Chunk) -> Result<(), VmError> {
        let mut ip = 0; // Instruction Pointer

        loop {
            // GOTCHA: We must perform bounds checking. Reading beyond the bytecode chunk
            // should not panic; it indicates malformed bytecode and should return an error.
            if ip >= chunk.code.len() {
                return Err(VmError::UnexpectedEof);
            }

            let instruction = chunk.code[ip];
            ip += 1;

            match instruction {
                OP_RETURN => {
                    // A simple VM might just pop and print the final value, or return it.
                    // For this implementation, we just end execution.
                    if let Ok(val) = self.pop() {
                        // RUST INSIGHT: We can conditionally compile debug prints.
                        // println!("Return: {}", val);
                        let _ = val;
                    }
                    return Ok(());
                }
                OP_CONSTANT => {
                    if ip >= chunk.code.len() {
                        return Err(VmError::UnexpectedEof);
                    }
                    let constant_index = chunk.code[ip] as usize;
                    ip += 1;

                    if constant_index >= chunk.constants.len() {
                        return Err(VmError::RuntimeError("Invalid constant index".to_string()));
                    }

                    // PRODUCTION NOTE: A real implementation might avoid clone and instead use Arc or similar for complex values
                    let constant = chunk.constants[constant_index].clone();
                    self.push(constant)?;
                }
                OP_NIL => self.push(Value::Nil)?,
                OP_TRUE => self.push(Value::Boolean(true))?,
                OP_FALSE => self.push(Value::Boolean(false))?,
                OP_EQUAL => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.push(Value::Boolean(a == b))?;
                }
                OP_NOT => {
                    let value = self.pop()?;
                    let is_falsy = match value {
                        Value::Nil | Value::Boolean(false) => true,
                        _ => false,
                    };
                    self.push(Value::Boolean(is_falsy))?;
                }
                OP_NEGATE => {
                    let value = self.pop()?;
                    match value {
                        Value::Number(n) => self.push(Value::Number(-n))?,
                        _ => return Err(VmError::RuntimeError("Operand must be a number".to_string())),
                    }
                }
                OP_ADD | OP_SUBTRACT | OP_MULTIPLY | OP_DIVIDE => {
                    let b = self.pop()?;
                    let a = self.pop()?;

                    match (a, b) {
                        (Value::Number(a_num), Value::Number(b_num)) => {
                            let result = match instruction {
                                OP_ADD => a_num + b_num,
                                OP_SUBTRACT => a_num - b_num,
                                OP_MULTIPLY => a_num * b_num,
                                OP_DIVIDE => {
                                    if (b_num).abs() < f64::EPSILON {
                                        return Err(VmError::RuntimeError("Division by zero".to_string()));
                                    }
                                    a_num / b_num
                                },
                                _ => unreachable!(),
                            };
                            self.push(Value::Number(result))?;
                        }
                        _ => return Err(VmError::RuntimeError("Operands must be numbers".to_string())),
                    }
                }
                _ => return Err(VmError::RuntimeError(format!("Unknown opcode: {}", instruction))),
            }
        }
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Canonical Comparisons:
// - `rune` and `mlua` are production-ready VMs with rich feature sets, garbage collection,
//   and tight Rust integration.
// - `rhai` provides a simple, fast scripting language that compiles to AST rather than bytecode.
//
// Missing Features:
// - Variables (local/global)
// - Control flow (if/else, loops, jumps)
// - Functions and call frames
// - Garbage collection (values currently cloned or use Rust's memory management)
// - NaN-boxing for compact value representation
//
// Next Steps:
// - Implement a compiler to generate `Chunk` from an AST.
// - Add jump instructions (`OP_JUMP`, `OP_JUMP_IF_FALSE`) for control flow.
// - Introduce a `CallFrame` struct to support function calls and local variables.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arithmetic() {
        let mut chunk = Chunk::new();

        let c1 = chunk.add_constant(Value::Number(1.2));
        let c2 = chunk.add_constant(Value::Number(3.4));

        chunk.write(OP_CONSTANT);
        chunk.write(c1 as u8);

        chunk.write(OP_CONSTANT);
        chunk.write(c2 as u8);

        chunk.write(OP_ADD);

        let c3 = chunk.add_constant(Value::Number(5.6));
        chunk.write(OP_CONSTANT);
        chunk.write(c3 as u8);

        chunk.write(OP_DIVIDE);
        chunk.write(OP_NEGATE);
        chunk.write(OP_RETURN);

        let mut vm = Vm::new(256);
        assert_eq!(vm.interpret(&chunk), Ok(()));
    }

    #[test]
    fn test_stack_overflow() {
        let mut chunk = Chunk::new();
        let c = chunk.add_constant(Value::Number(1.0));

        for _ in 0..10 {
            chunk.write(OP_CONSTANT);
            chunk.write(c as u8);
        }
        chunk.write(OP_RETURN);

        let mut vm = Vm::new(5); // Small stack
        assert_eq!(
            vm.interpret(&chunk),
            Err(VmError::RuntimeError("Stack overflow".to_string()))
        );
    }

    #[test]
    fn test_unexpected_eof() {
        let mut chunk = Chunk::new();
        chunk.write(OP_CONSTANT);
        // Missing constant index operand!

        let mut vm = Vm::new(256);
        assert_eq!(vm.interpret(&chunk), Err(VmError::UnexpectedEof));
    }

    #[test]
    fn test_division_by_zero() {
        let mut chunk = Chunk::new();

        let c1 = chunk.add_constant(Value::Number(1.0));
        let c2 = chunk.add_constant(Value::Number(0.0));

        chunk.write(OP_CONSTANT);
        chunk.write(c1 as u8);

        chunk.write(OP_CONSTANT);
        chunk.write(c2 as u8);

        chunk.write(OP_DIVIDE);
        chunk.write(OP_RETURN);

        let mut vm = Vm::new(256);
        assert_eq!(
            vm.interpret(&chunk),
            Err(VmError::RuntimeError("Division by zero".to_string()))
        );
    }

    // Benchmarking Note:
    // To benchmark this implementation against `rune` or `mlua`, one would use the `criterion` crate:
    // ```rust
    // pub fn bench_interpret(c: &mut Criterion) {
    //     let mut chunk = Chunk::new();
    //     // Build a chunk with a loop
    //     c.bench_function("vm interpret loop", |b| b.iter(|| {
    //         let mut vm = Vm::new(256);
    //         vm.interpret(&chunk).unwrap();
    //         std::hint::black_box(vm);
    //     }));
    // }
    // ```

    #[test]
    fn test_logic_and_equality() {
        let mut chunk = Chunk::new();

        chunk.write(OP_TRUE);
        chunk.write(OP_FALSE);
        chunk.write(OP_EQUAL); // true == false -> false
        chunk.write(OP_NOT);   // not false -> true
        chunk.write(OP_RETURN);

        let mut vm = Vm::new(256);
        assert_eq!(vm.interpret(&chunk), Ok(()));
    }
}
