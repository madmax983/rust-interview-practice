/// Stack-Based Bytecode Virtual Machine
///
/// What this implements and what crate(s) it replaces:
/// This implements a foundational stack-based virtual machine (VM) with a custom bytecode instruction set.
/// It demystifies how scripting languages and bytecode interpreters work under the hood.
/// It replaces the need for lightweight embedding crates like `rhai`, `rune`, or `mlua` when you need
/// a highly customized, minimal execution engine without external dependencies.
///
/// Real-world systems that use this:
/// - Python (CPython uses a stack-based bytecode interpreter)
/// - Java (JVM is a stack-based machine)
/// - Lua (Uses a register-based VM, but earlier versions were stack-based; both concepts apply)
/// - WebAssembly (Wasm execution engines)
///
/// Why build it yourself?
/// Building a VM from scratch forces you to understand the exact mechanics of execution context,
/// call stacks, instruction decoding, and constant pools. It reveals how high-level language constructs
/// (like `if` statements or arithmetic) are lowered into linear byte arrays.
///
/// Architecture
/// ------------
///
/// The VM executes a `Chunk` of bytecode. A `Chunk` consists of:
/// 1. A stream of instructions (`u8` opcodes + operands).
/// 2. A Constant Pool (`Vec<Value>`) to store literals (numbers, strings) that don't fit in an opcode.
///
/// Execution Model:
///
/// ```text
/// Bytecode Stream:  [OP_CONST] [0] [OP_CONST] [1] [OP_ADD] [OP_RETURN]
///                    |          |   |          |   |        |
///                    v          |   v          |   v        v
///                 Load Const #0 | Load Const #1| Add Top 2| Return Top
///                               v              v
/// Constant Pool:  [0]: 5        | [1]: 10      |
///                                              v
/// Stack Changes:
/// 1. [] -> [5]
/// 2. [5] -> [5, 10]
/// 3. [5, 10] -> [15]
/// 4. [15] -> (Execution Ends, returns 15)
/// ```
///
/// Invariants:
/// - Stack must never underflow (popping an empty stack).
/// - Instruction Pointer (IP) must remain within the bounds of the chunk's code.
/// - Constant indices must be valid within the constant pool.
///
/// Complexity:
/// - Operation Dispatch: O(1) via `match` or jump tables.
/// - Stack Operations (Push/Pop): O(1) amortized.
/// - Overall Execution: O(N) where N is the number of executed instructions.
///
/// Design Decisions:
/// - We use an explicit `Value` enum to allow dynamic typing (Numbers, Booleans, etc.).
/// - Operands (like constant indices or jump offsets) follow their opcodes inline within the `code` array.
/// - We use a `Vec` for the stack for simplicity, though a fixed-size array could prevent reallocation overhead.
use std::fmt;

/// Supported dynamic types within the VM.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// A 64-bit floating point number.
    Number(f64),
    /// A boolean value.
    Bool(bool),
    /// Null/Nil equivalent.
    Nil,
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Number(n) => write!(f, "{}", n),
            Self::Bool(b) => write!(f, "{}", b),
            Self::Nil => write!(f, "nil"),
        }
    }
}

/// Opcodes defining the instruction set of the VM.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpCode {
    /// Return from the current chunk.
    Return = 0,
    /// Load a constant from the constant pool. Operand: 1 byte index.
    Constant,
    /// Negate a number.
    Negate,
    /// Add two numbers.
    Add,
    /// Subtract two numbers.
    Subtract,
    /// Multiply two numbers.
    Multiply,
    /// Divide two numbers.
    Divide,
    /// Push `Nil` onto the stack.
    Nil,
    /// Push `true` onto the stack.
    True,
    /// Push `false` onto the stack.
    False,
    /// Logical NOT.
    Not,
    /// Check equality.
    Equal,
    /// Greater than.
    Greater,
    /// Less than.
    Less,
    /// Unconditional jump forward. Operand: 2 byte offset.
    Jump,
    /// Jump forward if the top of the stack is false (leaves value on stack). Operand: 2 byte offset.
    JumpIfFalse,
    /// Pop the top value off the stack.
    Pop,
}

impl TryFrom<u8> for OpCode {
    type Error = u8;

    /// Decodes a raw byte into an `OpCode`, returning the offending byte as the
    /// error for any value that does not map to a known opcode. This keeps
    /// decoding of untrusted bytecode fallible instead of panicking.
    fn try_from(byte: u8) -> Result<Self, Self::Error> {
        match byte {
            0 => Ok(Self::Return),
            1 => Ok(Self::Constant),
            2 => Ok(Self::Negate),
            3 => Ok(Self::Add),
            4 => Ok(Self::Subtract),
            5 => Ok(Self::Multiply),
            6 => Ok(Self::Divide),
            7 => Ok(Self::Nil),
            8 => Ok(Self::True),
            9 => Ok(Self::False),
            10 => Ok(Self::Not),
            11 => Ok(Self::Equal),
            12 => Ok(Self::Greater),
            13 => Ok(Self::Less),
            14 => Ok(Self::Jump),
            15 => Ok(Self::JumpIfFalse),
            16 => Ok(Self::Pop),
            _ => Err(byte),
        }
    }
}

/// Errors that can occur during execution.
#[derive(Debug, PartialEq, Eq)]
pub enum InterpretError {
    /// The VM attempted to execute an unknown opcode or failed an invariant.
    CompileError(String),
    /// A type error or runtime invariant violation (e.g., adding a bool to a number).
    RuntimeError(String),
}

/// A sequential block of bytecode instructions and constants.
#[derive(Debug, Clone)]
pub struct Chunk {
    /// The byte stream of instructions and inline operands.
    pub code: Vec<u8>,
    /// The constant pool containing literal values.
    pub constants: Vec<Value>,
}

impl Chunk {
    /// Creates a new, empty chunk.
    pub fn new() -> Self {
        Self {
            code: Vec::new(),
            constants: Vec::new(),
        }
    }

    /// Appends a byte (opcode or operand) to the chunk.
    pub fn write_byte(&mut self, byte: u8) {
        self.code.push(byte);
    }

    /// Appends an opcode to the chunk.
    pub fn write_opcode(&mut self, opcode: OpCode) {
        self.write_byte(opcode as u8);
    }

    /// Adds a constant to the pool and returns its index.
    pub fn add_constant(&mut self, value: Value) -> usize {
        self.constants.push(value);
        self.constants.len() - 1
    }

    /// Helper to write a jump instruction with a placeholder offset,
    /// returning the offset of the placeholder to patch later.
    pub fn write_jump(&mut self, opcode: OpCode) -> usize {
        self.write_opcode(opcode);
        self.write_byte(0xff); // Placeholder High
        self.write_byte(0xff); // Placeholder Low
        self.code.len() - 2
    }

    /// Patches a previously written jump instruction placeholder with the actual offset.
    pub fn patch_jump(&mut self, offset: usize) -> Result<(), InterpretError> {
        let jump = self.code.len() - offset - 2;
        if jump > u16::MAX as usize {
            return Err(InterpretError::CompileError(
                "Too much code to jump over.".to_string(),
            ));
        }

        self.code[offset] = ((jump >> 8) & 0xff) as u8;
        self.code[offset + 1] = (jump & 0xff) as u8;
        Ok(())
    }
}

/// The Virtual Machine that executes `Chunk`s.
pub struct VM {
    /// The evaluation stack.
    stack: Vec<Value>,
}

impl VM {
    /// Creates a new VM instance.
    pub fn new() -> Self {
        Self {
            stack: Vec::with_capacity(256),
        }
    }

    /// Pushes a value onto the evaluation stack.
    fn push(&mut self, value: Value) {
        self.stack.push(value);
    }

    /// Pops a value off the evaluation stack.
    fn pop(&mut self) -> Result<Value, InterpretError> {
        self.stack
            .pop()
            .ok_or_else(|| InterpretError::RuntimeError("Stack underflow".to_string()))
    }

    /// Peeks at a value near the top of the stack without popping it.
    /// `distance = 0` is the top of the stack.
    fn peek(&self, distance: usize) -> Result<&Value, InterpretError> {
        let len = self.stack.len();
        if distance >= len {
            Err(InterpretError::RuntimeError(
                "Stack peek out of bounds".to_string(),
            ))
        } else {
            Ok(&self.stack[len - 1 - distance])
        }
    }

    /// Executes the provided chunk.
    pub fn interpret(&mut self, chunk: &Chunk) -> Result<Value, InterpretError> {
        let mut ip = 0;

        macro_rules! read_byte {
            () => {{
                if ip >= chunk.code.len() {
                    return Err(InterpretError::RuntimeError(
                        "Unexpected end of code".to_string(),
                    ));
                }
                let byte = chunk.code[ip];
                ip += 1;
                byte
            }};
        }

        macro_rules! read_constant {
            () => {{
                let index = read_byte!() as usize;
                if index >= chunk.constants.len() {
                    return Err(InterpretError::RuntimeError(
                        "Invalid constant index".to_string(),
                    ));
                }
                chunk.constants[index].clone()
            }};
        }

        macro_rules! read_short {
            () => {{
                let high = read_byte!() as u16;
                let low = read_byte!() as u16;
                (high << 8) | low
            }};
        }

        macro_rules! binary_op {
            ($op:tt) => {{
                let b = self.pop()?;
                let a = self.pop()?;
                match (a, b) {
                    (Value::Number(a), Value::Number(b)) => self.push(Value::Number(a $op b)),
                    _ => return Err(InterpretError::RuntimeError("Operands must be numbers".to_string())),
                }
            }};
        }

        loop {
            let instruction = OpCode::try_from(read_byte!())
                .map_err(|byte| InterpretError::CompileError(format!("Unknown opcode: {byte}")))?;

            // RUST INSIGHT: Exhaustive pattern matching over `OpCode`
            // Rust ensures we handle every single opcode defined in our enum.
            // If we add a new opcode, the compiler will error here until we implement its behavior.
            match instruction {
                OpCode::Return => {
                    // Return the top of the stack, or Nil if empty
                    return Ok(self.stack.pop().unwrap_or(Value::Nil));
                }
                OpCode::Constant => {
                    let constant = read_constant!();
                    self.push(constant);
                }
                OpCode::Negate => match self.pop()? {
                    Value::Number(n) => self.push(Value::Number(-n)),
                    _ => {
                        return Err(InterpretError::RuntimeError(
                            "Operand must be a number".to_string(),
                        ));
                    }
                },
                OpCode::Add => {
                    // GOTCHA: String concatenation usually overloads the Add opcode.
                    // For simplicity, we only allow numeric addition here.
                    binary_op!(+);
                }
                OpCode::Subtract => binary_op!(-),
                OpCode::Multiply => binary_op!(*),
                OpCode::Divide => binary_op!(/),
                OpCode::Nil => self.push(Value::Nil),
                OpCode::True => self.push(Value::Bool(true)),
                OpCode::False => self.push(Value::Bool(false)),
                OpCode::Not => {
                    let value = self.pop()?;
                    self.push(Value::Bool(Self::is_falsey(&value)));
                }
                OpCode::Equal => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.push(Value::Bool(a == b));
                }
                OpCode::Greater => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (a, b) {
                        (Value::Number(a), Value::Number(b)) => self.push(Value::Bool(a > b)),
                        _ => {
                            return Err(InterpretError::RuntimeError(
                                "Operands must be numbers".to_string(),
                            ));
                        }
                    }
                }
                OpCode::Less => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (a, b) {
                        (Value::Number(a), Value::Number(b)) => self.push(Value::Bool(a < b)),
                        _ => {
                            return Err(InterpretError::RuntimeError(
                                "Operands must be numbers".to_string(),
                            ));
                        }
                    }
                }
                OpCode::Jump => {
                    let offset = read_short!();
                    ip += offset as usize;
                }
                OpCode::JumpIfFalse => {
                    let offset = read_short!();
                    if Self::is_falsey(self.peek(0)?) {
                        ip += offset as usize;
                    }
                }
                OpCode::Pop => {
                    self.pop()?;
                }
            }
        }
    }

    /// Determines if a value evaluates to `false` in a boolean context.
    /// In this VM, `nil` and `false` are falsey, everything else is truthy.
    fn is_falsey(value: &Value) -> bool {
        match value {
            Value::Nil => true,
            Value::Bool(b) => !b,
            _ => false,
        }
    }
}

impl Default for VM {
    fn default() -> Self {
        Self::new()
    }
}

// Canonical comparisons:
// - CPython/Lua: This architecture closely maps to how standard C interpreters are built (e.g., `clox` from Crafting Interpreters).
//   Production VMs often use a register-based architecture (like Lua) to reduce instruction count and stack manipulation overhead,
//   or advanced JIT compilation (like V8) to compile hot paths to native machine code.
//
// Missing vs Production:
// - Computed gotos / Direct Threading: Rust's `match` is fast, but production interpreters in C often use
//   computed gotos to dispatch opcodes, bypassing the central `switch` loop overhead.
// - Garbage Collection: We have no way to allocate heap objects (like Strings or Instances). A real VM
//   requires a GC (like the Mark-and-Sweep implementation in `garbage_collector.rs`).
// - Local/Global Variables: The VM currently only evaluates expressions; it lacks environment scopes for variable binding.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arithmetic() {
        let mut chunk = Chunk::new();
        let mut vm = VM::new();

        // 1.2 + 3.4
        let a = chunk.add_constant(Value::Number(1.2));
        let b = chunk.add_constant(Value::Number(3.4));

        chunk.write_opcode(OpCode::Constant);
        chunk.write_byte(a as u8);
        chunk.write_opcode(OpCode::Constant);
        chunk.write_byte(b as u8);
        chunk.write_opcode(OpCode::Add);
        chunk.write_opcode(OpCode::Return);

        let result = vm.interpret(&chunk).unwrap();
        assert_eq!(result, Value::Number(4.6));
    }

    #[test]
    fn test_logical_not() {
        let mut chunk = Chunk::new();
        let mut vm = VM::new();

        // !true -> false
        chunk.write_opcode(OpCode::True);
        chunk.write_opcode(OpCode::Not);
        chunk.write_opcode(OpCode::Return);

        let result = vm.interpret(&chunk).unwrap();
        assert_eq!(result, Value::Bool(false));
    }

    #[test]
    fn test_jump_if_false() {
        let mut chunk = Chunk::new();
        let mut vm = VM::new();

        // if (false) { return 10; } else { return 20; }
        // Emulated as:
        // 0: False
        // 1: JumpIfFalse to 7
        // 4: Pop
        // 5: Constant 10
        // 7: Jump to 10
        // 10: Pop
        // 11: Constant 20
        // 13: Return

        chunk.write_opcode(OpCode::False);
        let jump_if_false_offset = chunk.write_jump(OpCode::JumpIfFalse);

        chunk.write_opcode(OpCode::Pop); // Pop condition
        let ten = chunk.add_constant(Value::Number(10.0));
        chunk.write_opcode(OpCode::Constant);
        chunk.write_byte(ten as u8);

        let jump_offset = chunk.write_jump(OpCode::Jump);

        chunk.patch_jump(jump_if_false_offset).unwrap();
        chunk.write_opcode(OpCode::Pop); // Pop condition
        let twenty = chunk.add_constant(Value::Number(20.0));
        chunk.write_opcode(OpCode::Constant);
        chunk.write_byte(twenty as u8);

        chunk.patch_jump(jump_offset).unwrap();
        chunk.write_opcode(OpCode::Return);

        let result = vm.interpret(&chunk).unwrap();
        assert_eq!(result, Value::Number(20.0));
    }

    #[test]
    fn test_type_error() {
        let mut chunk = Chunk::new();
        let mut vm = VM::new();

        // true + 5
        let five = chunk.add_constant(Value::Number(5.0));

        chunk.write_opcode(OpCode::True);
        chunk.write_opcode(OpCode::Constant);
        chunk.write_byte(five as u8);
        chunk.write_opcode(OpCode::Add);
        chunk.write_opcode(OpCode::Return);

        let err = vm.interpret(&chunk).unwrap_err();
        assert_eq!(
            err,
            InterpretError::RuntimeError("Operands must be numbers".to_string())
        );
    }

    #[test]
    fn test_unknown_opcode_returns_err_not_panic() {
        let mut chunk = Chunk::new();
        let mut vm = VM::new();

        // 255 is not a valid opcode; decoding must be fallible, not panic.
        chunk.write_byte(255);
        chunk.write_opcode(OpCode::Return);

        let err = vm.interpret(&chunk).unwrap_err();
        assert_eq!(
            err,
            InterpretError::CompileError("Unknown opcode: 255".to_string())
        );
    }
}
