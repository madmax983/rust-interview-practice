//! # Bytecode Virtual Machine
//!
//! ## Purpose
//! Implements a minimal stack-based virtual machine capable of executing
//! bytecode chunks with a constant pool.
//!
//! ## Replaces
//! - `rhai`
//! - `rune`
//! - `mlua`
//!
//! ## Real-world Usage
//! - Used in dynamic scripting languages embedded in Rust applications.
//! - Foundational to interpreters like Lua, Python, or WebAssembly engines.
//!
//! ## Why build it yourself?
//! Building a VM demystifies how high-level code translates to machine-like
//! instructions. You learn about instruction dispatch, stack manipulation,
//! and how to manage dynamic types in a statically-typed language like Rust.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────┐       ┌────────────┐       ┌───────────────┐
//! │                 │       │            │       │               │
//! │ Bytecode Chunk  ├──────►│ Instruction│──────►│ Operand Stack │
//! │ (Array of u8)   │       │ Dispatch   │       │ (Vec<Value>)  │
//! │                 │       │            │       │               │
//! └─────────────────┘       └──────┬─────┘       └───────────────┘
//! ┌─────────────────┐              │
//! │                 │              │
//! │ Constant Pool   │◄─────────────┘
//! │ (Array of Value)│
//! │                 │
//! └─────────────────┘
//! ```
//!
//! ### Invariants
//! 1. The instruction pointer (IP) must never exceed the bounds of the bytecode array.
//! 2. Operations must check stack size to prevent underflow (popping empty stack).
//! 3. Constants are indexed properly and out-of-bounds constant access must be handled safely.
//!
//! ### Complexity
//! - **Dispatch:** O(1) loop iteration and match.
//! - **Stack Ops:** O(1) amortized push/pop on a `Vec`.

use std::fmt;

/// Represents the types of values our VM can process.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Number(f64),
    Bool(bool),
    // PRODUCTION NOTE: In a real VM, Strings or Objects would be heap-allocated
    // and reference-counted (e.g., `Rc<String>` or a custom Garbage Collected pointer).
    // For simplicity, we clone the string.
    String(String),
    Nil,
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Number(n) => write!(f, "{}", n),
            Self::Bool(b) => write!(f, "{}", b),
            Self::String(s) => write!(f, "{}", s),
            Self::Nil => write!(f, "nil"),
        }
    }
}

/// The instruction set architecture (ISA) for our virtual machine.
///
/// We use an enum to represent instructions, which allows Rust's pattern
/// matching to guarantee exhaustiveness during dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum OpCode {
    /// Return from the current function/script.
    Return = 0,
    /// Load a constant from the constant pool onto the stack.
    /// Followed by 1 byte representing the constant index.
    Constant = 1,
    /// Negate the number on top of the stack.
    Negate = 2,
    /// Add the top two numbers on the stack.
    Add = 3,
    /// Subtract the top two numbers on the stack.
    Subtract = 4,
    /// Multiply the top two numbers on the stack.
    Multiply = 5,
    /// Divide the top two numbers on the stack.
    Divide = 6,
}

impl TryFrom<u8> for OpCode {
    type Error = String;

    fn try_from(byte: u8) -> Result<Self, Self::Error> {
        // GOTCHA: Converting from raw bytes to enums can be unsafe or tricky.
        // We do a manual match. In production, a crate like `num_enum` might be used.
        match byte {
            0 => Ok(Self::Return),
            1 => Ok(Self::Constant),
            2 => Ok(Self::Negate),
            3 => Ok(Self::Add),
            4 => Ok(Self::Subtract),
            5 => Ok(Self::Multiply),
            6 => Ok(Self::Divide),
            _ => Err(format!("Unknown opcode: {}", byte)),
        }
    }
}

/// A chunk of bytecode representing a compiled unit of code (like a function or script).
#[derive(Debug, Clone, Default)]
pub struct Chunk {
    /// The raw bytecode instructions.
    pub code: Vec<u8>,
    /// The constant pool containing literals (numbers, strings) used in the code.
    pub constants: Vec<Value>,
    // RUST INSIGHT:
    // Rust's vectors keep track of their own length and capacity, making it easy
    // to build bytecode dynamically without manual memory management.
}

impl Chunk {
    #[must_use]
    pub fn new() -> Self {
        Self {
            code: Vec::new(),
            constants: Vec::new(),
        }
    }

    /// Appends a raw byte to the chunk.
    pub fn write_byte(&mut self, byte: u8) {
        self.code.push(byte);
    }

    /// Appends an instruction to the chunk.
    pub fn write_opcode(&mut self, opcode: OpCode) {
        self.code.push(opcode as u8);
    }

    /// Adds a constant to the pool and returns its index.
    pub fn add_constant(&mut self, value: Value) -> usize {
        self.constants.push(value);
        self.constants.len() - 1
    }
}

#[derive(Debug)]
pub enum InterpretError {
    CompileError,
    RuntimeError(String),
    UnexpectedEof,
}

/// A generic interpreter trait showing swappable execution strategies.
pub trait Interpreter {
    /// Loads a chunk of bytecode and executes it.
    ///
    /// # Errors
    /// Returns an `InterpretError` if execution fails.
    fn interpret(&mut self, chunk: Chunk) -> Result<(), InterpretError>;
}

/// The Stack-based Virtual Machine.
#[derive(Default)]
pub struct VM {
    chunk: Chunk,
    ip: usize,
    stack: Vec<Value>,
}

impl VM {
    #[must_use]
    pub fn new() -> Self {
        Self {
            chunk: Chunk::new(),
            ip: 0,
            stack: Vec::with_capacity(256),
        }
    }
}

impl Interpreter for VM {
    fn interpret(&mut self, chunk: Chunk) -> Result<(), InterpretError> {
        self.chunk = chunk;
        self.ip = 0;
        self.run()
    }
}

impl VM {

    fn push(&mut self, value: Value) {
        self.stack.push(value);
    }

    fn pop(&mut self) -> Result<Value, InterpretError> {
        self.stack
            .pop()
            .ok_or_else(|| InterpretError::RuntimeError("Stack underflow".to_string()))
    }

    /// The core execution loop.
    fn run(&mut self) -> Result<(), InterpretError> {
        loop {
            if self.ip >= self.chunk.code.len() {
                return Ok(()); // Reached end of chunk unexpectedly, but safely
            }

            let byte = self.read_byte()?;
            let instruction =
                OpCode::try_from(byte).map_err(|e| InterpretError::RuntimeError(e))?;

            match instruction {
                OpCode::Return => {
                    let _ = self.pop();
                    return Ok(());
                }
                OpCode::Constant => {
                    let constant = self.read_constant()?;
                    self.push(constant);
                }
                OpCode::Negate => {
                    let val = self.pop()?;
                    match val {
                        Value::Number(n) => self.push(Value::Number(-n)),
                        _ => {
                            return Err(InterpretError::RuntimeError(
                                "Operand must be a number".to_string(),
                            ));
                        }
                    }
                }
                OpCode::Add => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (a, b) {
                        (Value::Number(x), Value::Number(y)) => self.push(Value::Number(x + y)),
                        (Value::String(x), Value::String(y)) => self.push(Value::String(x + &y)),
                        _ => {
                            return Err(InterpretError::RuntimeError(
                                "Operands must be two numbers or two strings".to_string(),
                            ));
                        }
                    }
                }
                OpCode::Subtract => self.binary_op(|a, b| a - b)?,
                OpCode::Multiply => self.binary_op(|a, b| a * b)?,
                OpCode::Divide => self.binary_op(|a, b| a / b)?,
            }
        }
    }

    fn read_byte(&mut self) -> Result<u8, InterpretError> {
        if self.ip >= self.chunk.code.len() {
            return Err(InterpretError::UnexpectedEof);
        }
        let byte = self.chunk.code[self.ip];
        self.ip += 1;
        Ok(byte)
    }

    fn read_constant(&mut self) -> Result<Value, InterpretError> {
        let index = self.read_byte()? as usize;
        if index < self.chunk.constants.len() {
            // RUST INSIGHT:
            // Cloning the value here. If values were large, we'd use `Rc` or `Arc` to
            // avoid expensive copies, maintaining zero-cost abstractions where possible.
            Ok(self.chunk.constants[index].clone())
        } else {
            Err(InterpretError::RuntimeError(
                "Invalid constant index".to_string(),
            ))
        }
    }

    fn binary_op<F>(&mut self, op: F) -> Result<(), InterpretError>
    where
        F: FnOnce(f64, f64) -> f64,
    {
        let b = self.pop()?;
        let a = self.pop()?;

        match (a, b) {
            (Value::Number(x), Value::Number(y)) => {
                self.push(Value::Number(op(x, y)));
                Ok(())
            }
            _ => Err(InterpretError::RuntimeError(
                "Operands must be numbers".to_string(),
            )),
        }
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `rhai`: Provides a full scripting language with AST walking and bytecode execution.
// - `rune`: A highly optimized VM written in Rust for a custom dynamic language.
// - `mlua`: Wraps the canonical Lua C implementation, exposing a fast stack-based VM.
//
// Missing vs. Production:
// - **Garbage Collection**: We clone strings and don't support objects/arrays. Production VMs use mark-and-sweep or reference counting.
// - **Variables & Scope**: No local/global variables, just a stack.
// - **Control Flow**: No Jump instructions for `if`/`while` loops.
// - **Direct Threaded Code**: We use a simple `match` loop. Real VMs often use threaded dispatch or JIT compilation for performance.
//
// Next Steps:
// 1. Add `OpCode::Jump` and `OpCode::JumpIfFalse` for control flow.
// 2. Add local variables by accessing elements deep in the stack.
//
// Benchmarking:
// To benchmark this VM against alternatives (like evaluating AST nodes directly),
// use the `criterion` crate:
// ```rust
// pub fn vm_benchmark(c: &mut Criterion) {
//     c.bench_function("vm_add", |b| {
//         b.iter(|| {
//             let mut vm = VM::new();
//             // Construct simple chunk...
//             vm.interpret(black_box(chunk)).unwrap();
//         })
//     });
// }
// ```

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vm_addition() {
        let mut chunk = Chunk::new();

        let c1 = chunk.add_constant(Value::Number(1.2));
        let c2 = chunk.add_constant(Value::Number(3.4));

        chunk.write_opcode(OpCode::Constant);
        chunk.write_byte(c1 as u8);

        chunk.write_opcode(OpCode::Constant);
        chunk.write_byte(c2 as u8);

        chunk.write_opcode(OpCode::Add);
        chunk.write_opcode(OpCode::Return);

        let mut vm = VM::new();
        assert!(vm.interpret(chunk).is_ok());
    }

    #[test]
    fn test_vm_math_expression() {
        // Compute: -((1.2 + 3.4) / 2.0)
        let mut chunk = Chunk::new();

        let c1 = chunk.add_constant(Value::Number(1.2));
        let c2 = chunk.add_constant(Value::Number(3.4));
        let c3 = chunk.add_constant(Value::Number(2.0));

        chunk.write_opcode(OpCode::Constant);
        chunk.write_byte(c1 as u8);

        chunk.write_opcode(OpCode::Constant);
        chunk.write_byte(c2 as u8);

        chunk.write_opcode(OpCode::Add);

        chunk.write_opcode(OpCode::Constant);
        chunk.write_byte(c3 as u8);

        chunk.write_opcode(OpCode::Divide);
        chunk.write_opcode(OpCode::Negate);

        chunk.write_opcode(OpCode::Return);

        let mut vm = VM::new();
        assert!(vm.interpret(chunk).is_ok());
    }

    #[test]
    fn test_stack_underflow() {
        let mut chunk = Chunk::new();
        chunk.write_opcode(OpCode::Add);

        let mut vm = VM::new();
        let result = vm.interpret(chunk);

        assert!(matches!(result, Err(InterpretError::RuntimeError(_))));
    }

    #[test]
    fn test_string_concatenation() {
        let mut chunk = Chunk::new();

        let s1 = chunk.add_constant(Value::String("hello ".to_string()));
        let s2 = chunk.add_constant(Value::String("world".to_string()));

        chunk.write_opcode(OpCode::Constant);
        chunk.write_byte(s1 as u8);

        chunk.write_opcode(OpCode::Constant);
        chunk.write_byte(s2 as u8);

        chunk.write_opcode(OpCode::Add);
        chunk.write_opcode(OpCode::Return);

        let mut vm = VM::new();
        assert!(vm.interpret(chunk).is_ok());
    }
}
