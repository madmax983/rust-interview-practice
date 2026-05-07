//! # Bytecode Virtual Machine
//!
//! Implements a minimal stack-based bytecode virtual machine.
//!
//! **Replaces Crates:** `rhai`, `rune`, `mlua`
//!
//! **Real-world Usage:**
//! - Interpreted programming languages (Python, Ruby, Lua).
//! - Embedded scripting engines in games or applications.
//! - Smart contract execution environments (e.g., EVM, WebAssembly).
//!
//! **Why build it yourself?**
//! Building a Virtual Machine demystifies how high-level languages are actually executed by a computer.
//! It teaches you about stack machines, instruction dispatch loops, constant pools, and the tradeoffs
//! between AST-walking interpreters and compiled bytecode. You will understand how dynamic languages
//! manage state and memory during execution.
//!
//! # Architecture
//!
//! Data Structure:
//!
//!       Chunk (Bytecode & Data)
//!       ├── code: Vec<u8> (Instructions)
//!       └── constants: Vec<Value> (Literal pool)
//!
//!       StackVM (Execution Engine)
//!       ├── chunk: &Chunk
//!       ├── ip: usize (Instruction Pointer)
//!       └── stack: Vec<Value> (Evaluation Stack)
//!
//! **Invariants:**
//! 1. **Stack Underflow:** The VM must never pop from an empty stack.
//! 2. **Instruction Pointer Bounds:** The `ip` must never exceed the bounds of the `code` array.
//! 3. **Type Safety (Runtime):** Operations on `Value` variants must handle type mismatches gracefully.
//!
//! **Complexity:**
//! ┌───────────────┬─────────────┬─────────────┐
//! │ Operation     │ Time        │ Space       │
//! ├───────────────┼─────────────┼─────────────┤
//! │ Instruction   │ O(1)        │ O(1)        │
//! │ Execution Loop│ O(N)        │ O(D) stack  │
//! └───────────────┴─────────────┴─────────────┘
//! N = number of executed instructions, D = max stack depth.

use std::fmt;

/// Represents a dynamic value in the virtual machine.
#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum Value {
    Number(f64),
    Boolean(bool),
    Nil,
    // PRODUCTION NOTE: A production VM would use a garbage collected or reference counted
    // string pool (e.g., `Rc<String>`) rather than owned Strings to avoid allocations and copying.
    String(String),
}

impl Value {
    /// Helper to enforce a Value is a Number for mathematical operations.
    #[allow(dead_code)]
    fn as_number(&self) -> Result<f64, RuntimeError> {
        match self {
            Value::Number(n) => Ok(*n),
            _ => Err(RuntimeError::TypeError("Expected a number".into())),
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Number(n) => write!(f, "{}", n),
            Value::Boolean(b) => write!(f, "{}", b),
            Value::Nil => write!(f, "nil"),
            Value::String(s) => write!(f, "{}", s),
        }
    }
}

/// The set of instructions the VM can execute.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpCode {
    /// Return from the current context.
    Return = 0,
    /// Load a constant from the constant pool. Takes 1 byte operand (index).
    Constant,
    /// Negate a numeric value on the top of the stack.
    Negate,
    /// Add the top two values on the stack.
    Add,
    /// Subtract the top value from the second top value.
    Subtract,
    /// Multiply the top two values.
    Multiply,
    /// Divide the second top value by the top value.
    Divide,
    /// Push a Boolean true onto the stack.
    True,
    /// Push a Boolean false onto the stack.
    False,
    /// Push a Nil onto the stack.
    Nil,
    /// Perform logical NOT on the top of the stack.
    Not,
    /// Check equality of top two values.
    Equal,
    /// Check if second top is less than top.
    Less,
    /// Check if second top is greater than top.
    Greater,
    /// Unconditional jump. Takes 2 byte operand (offset).
    Jump,
    /// Conditional jump if top of stack is false. Takes 2 byte operand (offset).
    JumpIfFalse,
    /// Pop the top value from the stack.
    Pop,
}

impl TryFrom<u8> for OpCode {
    type Error = RuntimeError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(OpCode::Return),
            1 => Ok(OpCode::Constant),
            2 => Ok(OpCode::Negate),
            3 => Ok(OpCode::Add),
            4 => Ok(OpCode::Subtract),
            5 => Ok(OpCode::Multiply),
            6 => Ok(OpCode::Divide),
            7 => Ok(OpCode::True),
            8 => Ok(OpCode::False),
            9 => Ok(OpCode::Nil),
            10 => Ok(OpCode::Not),
            11 => Ok(OpCode::Equal),
            12 => Ok(OpCode::Less),
            13 => Ok(OpCode::Greater),
            14 => Ok(OpCode::Jump),
            15 => Ok(OpCode::JumpIfFalse),
            16 => Ok(OpCode::Pop),
            _ => Err(RuntimeError::InvalidOpCode(value)),
        }
    }
}

/// Runtime errors that can occur during execution.
#[derive(Debug, Clone, PartialEq)]
pub enum RuntimeError {
    StackUnderflow,
    InvalidOpCode(u8),
    ConstantIndexOutOfBounds(usize),
    TypeError(String),
    DivideByZero,
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RuntimeError::StackUnderflow => write!(f, "Stack underflow"),
            RuntimeError::InvalidOpCode(c) => write!(f, "Invalid opcode: {}", c),
            RuntimeError::ConstantIndexOutOfBounds(i) => write!(f, "Constant index out of bounds: {}", i),
            RuntimeError::TypeError(m) => write!(f, "Type error: {}", m),
            RuntimeError::DivideByZero => write!(f, "Division by zero"),
        }
    }
}

impl std::error::Error for RuntimeError {}

/// A chunk of bytecode instructions and its associated data (constants).
#[derive(Debug, Default)]
pub struct Chunk {
    pub code: Vec<u8>,
    pub constants: Vec<Value>,
}

impl Chunk {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn write_opcode(&mut self, opcode: OpCode) {
        self.code.push(opcode as u8);
    }

    pub fn write_byte(&mut self, byte: u8) {
        self.code.push(byte);
    }

    pub fn add_constant(&mut self, value: Value) -> u8 {
        // GOTCHA: A real implementation needs to handle constant pools larger than 256.
        // It typically does this with `OP_CONSTANT_16` or `OP_CONSTANT_LONG` taking 2/3 bytes.
        let index = self.constants.len();
        assert!(index < 256, "Constant pool size exceeded 256");
        self.constants.push(value);
        index as u8
    }

    pub fn write_constant(&mut self, value: Value) {
        let index = self.add_constant(value);
        self.write_opcode(OpCode::Constant);
        self.write_byte(index);
    }
}

/// Trait defining the standard Virtual Machine interface.
pub trait VirtualMachine {
    /// Executes a given chunk of bytecode and returns the last value on the stack, or an error.
    fn interpret(&mut self, chunk: &Chunk) -> Result<Value, RuntimeError>;
}

/// A standard stack-based bytecode virtual machine.
#[derive(Default)]
pub struct StackVM {
    stack: Vec<Value>,
    // RUST INSIGHT: We don't store a pointer to the chunk in the VM struct because of lifetime complexities.
    // Instead, we pass the chunk to `interpret` and hold it as a local variable during execution.
    // This cleanly sidesteps self-referential lifetimes and keeps the struct simple.
}

impl StackVM {
    pub fn new() -> Self {
        Self {
            stack: Vec::with_capacity(256), // Pre-allocate to avoid runtime reallocation
        }
    }

    fn push(&mut self, value: Value) {
        self.stack.push(value);
    }

    fn pop(&mut self) -> Result<Value, RuntimeError> {
        self.stack.pop().ok_or(RuntimeError::StackUnderflow)
    }

    fn peek(&self, distance: usize) -> Result<&Value, RuntimeError> {
        let len = self.stack.len();
        if distance >= len {
            Err(RuntimeError::StackUnderflow)
        } else {
            Ok(&self.stack[len - 1 - distance])
        }
    }
}

impl VirtualMachine for StackVM {
    fn interpret(&mut self, chunk: &Chunk) -> Result<Value, RuntimeError> {
        self.stack.clear();
        let mut ip = 0;

        macro_rules! read_byte {
            () => {{
                let byte = *chunk.code.get(ip).ok_or(RuntimeError::InvalidOpCode(0))?;
                ip += 1;
                byte
            }};
        }

        macro_rules! read_u16 {
            () => {{
                let b1 = read_byte!() as u16;
                let b2 = read_byte!() as u16;
                (b1 << 8) | b2
            }};
        }

        macro_rules! binary_op {
            ($op:tt, $result_type:ident) => {{
                let b = self.pop()?;
                let a = self.pop()?;
                match (a, b) {
                    (Value::Number(a), Value::Number(b)) => {
                        self.push(Value::$result_type(a $op b));
                    }
                    _ => return Err(RuntimeError::TypeError("Operands must be numbers".into())),
                }
            }};
        }

        loop {
            if ip >= chunk.code.len() {
                break;
            }

            let instruction_byte = read_byte!();
            let instruction = OpCode::try_from(instruction_byte)?;

            match instruction {
                OpCode::Return => {
                    return self.stack.pop().ok_or(RuntimeError::StackUnderflow);
                }
                OpCode::Constant => {
                    let constant_index = read_byte!() as usize;
                    let constant = chunk.constants.get(constant_index).ok_or(RuntimeError::ConstantIndexOutOfBounds(constant_index))?.clone();
                    self.push(constant);
                }
                OpCode::Negate => {
                    match self.peek(0)? {
                        Value::Number(n) => {
                            let val = -n;
                            let _ = self.pop();
                            self.push(Value::Number(val));
                        }
                        _ => return Err(RuntimeError::TypeError("Operand must be a number".into())),
                    }
                }
                OpCode::Add => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (a, b) {
                        (Value::Number(a), Value::Number(b)) => self.push(Value::Number(a + b)),
                        (Value::String(a), Value::String(b)) => {
                            // RUST INSIGHT: We allocate a new string here. In a production VM, we'd use a GC pool.
                            self.push(Value::String(format!("{}{}", a, b)));
                        }
                        _ => return Err(RuntimeError::TypeError("Operands must be two numbers or two strings".into())),
                    }
                }
                OpCode::Subtract => binary_op!(-, Number),
                OpCode::Multiply => binary_op!(*, Number),
                OpCode::Divide => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (a, b) {
                        (Value::Number(a), Value::Number(b)) => {
                            if b == 0.0 {
                                return Err(RuntimeError::DivideByZero);
                            }
                            self.push(Value::Number(a / b));
                        }
                        _ => return Err(RuntimeError::TypeError("Operands must be numbers".into())),
                    }
                }
                OpCode::True => self.push(Value::Boolean(true)),
                OpCode::False => self.push(Value::Boolean(false)),
                OpCode::Nil => self.push(Value::Nil),
                OpCode::Not => {
                    let value = self.pop()?;
                    let is_falsey = match value {
                        Value::Nil | Value::Boolean(false) => true,
                        _ => false,
                    };
                    self.push(Value::Boolean(is_falsey));
                }
                OpCode::Equal => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.push(Value::Boolean(a == b));
                }
                OpCode::Less => binary_op!(<, Boolean),
                OpCode::Greater => binary_op!(>, Boolean),
                OpCode::Jump => {
                    let offset = read_u16!();
                    ip += offset as usize;
                }
                OpCode::JumpIfFalse => {
                    let offset = read_u16!();
                    let peeked = self.peek(0)?;
                    let is_falsey = match peeked {
                        Value::Nil | Value::Boolean(false) => true,
                        _ => false,
                    };
                    if is_falsey {
                        ip += offset as usize;
                    }
                }
                OpCode::Pop => {
                    self.pop()?;
                }
            }
        }

        // If execution finishes without a Return statement, pop the last value.
        // If stack is empty, return Nil.
        Ok(self.stack.pop().unwrap_or(Value::Nil))
    }
}

// =========================================================================================
// Alternative Approaches
// =========================================================================================
//
// 1. **AST-Walking Interpreter (e.g., standard `eval`)**
//    - *Pros:* Easier to implement, maps directly to the syntax tree.
//    - *Cons:* Slow, cache un-friendly, uses arbitrary tree recursion which can blow the call stack.
//
// 2. **Register-Based VM (e.g., Lua)**
//    - *Pros:* Fewer instructions needed for a given task, better mapping to real CPU registers.
//    - *Cons:* More complex compiler, larger instruction size (need to specify registers per opcode).
//
// 3. **JIT Compilation (e.g., V8, JVM)**
//    - *Pros:* Extremely fast, compiles bytecode to native machine code at runtime.
//    - *Cons:* Massive complexity, requires architecture-specific machine code generation, large memory footprint.
//
// # Canonical Replacements
// - **`rhai`**: A fast, embedded scripting language for Rust. Uses a similar AST/bytecode hybrid approach.
// - **`rune`**: A dynamic language for Rust. Compiles to an advanced, stack-based bytecode VM.
// - **`mlua`**: Bindings to Lua. Lua uses a highly optimized register-based VM.
//
// # What's missing vs. Production
// - Garbage collection (strings/objects leak or use Rust `Rc`, which has cycle problems).
// - Local/global variables and environments.
// - Function calls, closures, and call frames.
// - Advanced datatypes (Arrays, HashMaps).

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arithmetic() {
        let mut chunk = Chunk::new();
        // Calculate: (1.2 + 3.4) * 5.6
        chunk.write_constant(Value::Number(1.2)); // Stack: [1.2]
        chunk.write_constant(Value::Number(3.4)); // Stack: [1.2, 3.4]
        chunk.write_opcode(OpCode::Add);          // Stack: [4.6]
        chunk.write_constant(Value::Number(5.6)); // Stack: [4.6, 5.6]
        chunk.write_opcode(OpCode::Multiply);     // Stack: [25.76]
        chunk.write_opcode(OpCode::Return);

        let mut vm = StackVM::new();
        let result = vm.interpret(&chunk).unwrap();

        if let Value::Number(n) = result {
            // Note: Floating point arithmetic can result in slight precision errors
            // e.g. 4.6 * 5.6 = 25.759999999999998
            assert!((n - 25.76).abs() < 1e-10);
        } else {
            panic!("Expected Number");
        }
    }

    #[test]
    fn test_string_concatenation() {
        let mut chunk = Chunk::new();
        chunk.write_constant(Value::String("hello, ".into()));
        chunk.write_constant(Value::String("world".into()));
        chunk.write_opcode(OpCode::Add);
        chunk.write_opcode(OpCode::Return);

        let mut vm = StackVM::new();
        let result = vm.interpret(&chunk).unwrap();

        assert_eq!(result, Value::String("hello, world".into()));
    }

    #[test]
    fn test_logic_and_comparisons() {
        let mut chunk = Chunk::new();
        // 5 > 3 -> true
        chunk.write_constant(Value::Number(5.0));
        chunk.write_constant(Value::Number(3.0));
        chunk.write_opcode(OpCode::Greater);
        // true == !false
        chunk.write_opcode(OpCode::False);
        chunk.write_opcode(OpCode::Not);
        chunk.write_opcode(OpCode::Equal);
        chunk.write_opcode(OpCode::Return);

        let mut vm = StackVM::new();
        let result = vm.interpret(&chunk).unwrap();
        assert_eq!(result, Value::Boolean(true));
    }

    #[test]
    fn test_jump() {
        let mut chunk = Chunk::new();
        // Push 1, skip push 2, push 3
        chunk.write_constant(Value::Number(1.0));
        chunk.write_opcode(OpCode::Jump);
        chunk.write_byte(0); // Offset high byte
        chunk.write_byte(2); // Offset low byte: skip over Constant and byte (2 bytes)

        chunk.write_constant(Value::Number(2.0)); // Should be skipped

        chunk.write_constant(Value::Number(3.0));
        chunk.write_opcode(OpCode::Add);
        chunk.write_opcode(OpCode::Return);

        let mut vm = StackVM::new();
        let result = vm.interpret(&chunk).unwrap();
        assert_eq!(result, Value::Number(4.0));
    }

    #[test]
    fn test_divide_by_zero() {
        let mut chunk = Chunk::new();
        chunk.write_constant(Value::Number(5.0));
        chunk.write_constant(Value::Number(0.0));
        chunk.write_opcode(OpCode::Divide);
        chunk.write_opcode(OpCode::Return);

        let mut vm = StackVM::new();
        let result = vm.interpret(&chunk);
        assert_eq!(result, Err(RuntimeError::DivideByZero));
    }
}
