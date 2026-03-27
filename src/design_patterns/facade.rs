// Suppress pedantic and nursery lints for design pattern examples.
#![allow(clippy::pedantic, clippy::nursery, unused)]
//! # Facade Pattern (The Session / RAII Wrapper)
//!
//! Replaces: **Facade Pattern** (GoF), **Session Beans** (Java), **God Objects**
//!
//! Real Rust usage: `sqlx::Transaction`, `std::net::TcpStream`, `git2::Repository`
//!
//! ## Why this pattern exists in Rust specifically
//! In OOP, a Facade is a structural pattern that provides a simplified, higher-level interface
//! to a complex subsystem of classes. It often takes the form of a "God Object" or a stateless
//! manager class holding references to various internal services.
//!
//! In Rust, the Facade pattern frequently transforms into what we call the **Session** or **RAII Wrapper**
//! pattern. Because Rust tracks lifetimes strictly, we bind high-level, simplified APIs (the Facade)
//! to the specific scopes and lifetimes of the low-level resources they manage.
//!
//! By tying the Facade's existence to a mutable borrow (`&mut`), Rust ensures that the complex
//! subsystem cannot be modified by other code while the Facade is active.
//!
//! ## Meta-Pattern: Make Illegal States Unrepresentable
//! A well-designed Rust Facade doesn't just hide complexity; it prevents invalid operations.
//! For example, a `TransactionFacade` borrows the database connection mutably. It is statically
//! impossible (illegal state) for another thread to execute a query on that connection outside
//! the transaction while the Facade exists.
//!
//! ## Architecture
//!
//! ```text
//! [ Subsystem A (CPU) ] \
//! [ Subsystem B (RAM) ] --> [ Facade / Session<'a> ] --> [ Client ]
//! [ Subsystem C (Disk)] /
//! ```
//!
//! **Invariants Enforced:**
//! - **Exclusive Access:** If the Facade requires a mutable borrow of a subsystem, it guarantees
//!   exclusive access for its lifetime.
//! - **Automatic Cleanup:** The Facade leverages the `Drop` trait (RAII) to ensure the subsystem
//!   is returned to a clean state when the Facade goes out of scope.
//!
//! **When to use:**
//! - When integrating with massive third-party APIs (like a rendering engine or database driver).
//! - When you need to orchestrate multiple sub-components to achieve a single domain-level task.
//!
//! **Anti-patterns:**
//! - A sprawling `struct AppFacade` holding `Arc<Mutex<T>>` to everything, acting as a global God Object.
//! - Writing Facades that don't enforce constraints via lifetimes or the type system.

// ============================================================================
// The Complex Subsystems
// ============================================================================

/// Subsystem 1: A raw CPU controller
pub struct Cpu {
    temperature: u8,
}
impl Cpu {
    pub fn freeze(&self) {
        println!("CPU: Freezing operations.");
    }
    pub fn jump(&self, position: u64) {
        println!("CPU: Jumping to 0x{:X}", position);
    }
    pub fn execute(&self) {
        println!("CPU: Executing instructions.");
    }
}

/// Subsystem 2: A raw Memory controller
pub struct Memory {
    capacity: u64,
}
impl Memory {
    pub fn load(&mut self, position: u64, data: &[u8]) {
        println!("Memory: Loading {} bytes at 0x{:X}", data.len(), position);
    }
}

/// Subsystem 3: A raw Disk controller
pub struct HardDrive {
    sector_size: u32,
}
impl HardDrive {
    pub fn read(&self, lba: u64, size: u32) -> Vec<u8> {
        println!("Disk: Reading {} bytes from LBA {}", size, lba);
        vec![0; size as usize]
    }
}

// ============================================================================
// The Facade
// ============================================================================

// ANTI-PATTERN: `class ComputerFacade { private CPU cpu; private Memory mem; ... }`
// In OOP, the Facade often takes ownership of the subsystems permanently.
// In Rust, we often want the Facade to be a temporary, scoped view over the subsystems.

/// The Facade providing a simple interface to start the computer.
///
/// **OWNERSHIP INSIGHT:** The Facade temporarily borrows the subsystems.
/// It takes `&mut Memory` because loading an OS modifies RAM, but only needs `&Cpu`
/// because reading instructions doesn't mutate the CPU struct itself.
pub struct ComputerFacade<'a> {
    cpu: &'a Cpu,
    ram: &'a mut Memory,
    disk: &'a HardDrive,
}

impl<'a> ComputerFacade<'a> {
    const BOOT_SECTOR: u64 = 0x00;
    const SECTOR_SIZE: u32 = 512;
    const BOOT_ADDRESS: u64 = 0x7C00;

    /// Creates a new Facade for the computer subsystems.
    pub fn new(cpu: &'a Cpu, ram: &'a mut Memory, disk: &'a HardDrive) -> Self {
        Self { cpu, ram, disk }
    }

    /// The simplified high-level API.
    ///
    /// **COMPILE-TIME WIN:** While `start()` is running, the compiler guarantees
    /// that no other part of the program can mutate the `ram` because we hold an exclusive
    /// mutable borrow through the Facade's lifetime.
    pub fn start(&mut self) {
        println!("Facade: Initiating boot sequence...");

        self.cpu.freeze();

        // Facade orchestrates the complex interaction
        let boot_data = self.disk.read(Self::BOOT_SECTOR, Self::SECTOR_SIZE);
        self.ram.load(Self::BOOT_ADDRESS, &boot_data);

        self.cpu.jump(Self::BOOT_ADDRESS);
        self.cpu.execute();

        println!("Facade: Boot sequence complete.");
    }
}

// ============================================================================
// The Alternative (Without Facade)
// ============================================================================

// ANTI-PATTERN: Forcing the client to orchestrate the complex subsystems manually.
// This is error-prone because the client might forget a step or execute them out of order.
pub fn start_computer_manually(cpu: &Cpu, ram: &mut Memory, disk: &HardDrive) {
    // TRADEOFF: Without a Facade, the client has ultimate flexibility, but must know
    // exactly how the subsystems interact. This violates encapsulation.
    cpu.freeze();
    let boot_data = disk.read(0x00, 512);
    ram.load(0x7C00, &boot_data);
    cpu.jump(0x7C00);
    cpu.execute();
}

// ============================================================================
// Approach 2: The Stateful RAII Facade (Session Pattern)
// ============================================================================
// This is a highly idiomatic Rust variant of the Facade.

/// A low-level connection pool that is complex to manage safely.
pub struct ConnectionPool {
    connected: bool,
}

impl ConnectionPool {
    pub fn execute_raw(&self, query: &str) -> Result<(), String> {
        if !self.connected {
            return Err("Not connected".to_string());
        }
        // Execute query
        Ok(())
    }
}

/// A Domain-Specific Facade (Session) over the database pool.
pub struct UserSession<'a> {
    pool: &'a ConnectionPool,
    transaction_open: bool,
}

impl<'a> UserSession<'a> {
    /// Creates a Facade and immediately alters the state of the subsystem (begins a transaction).
    pub fn begin(pool: &'a ConnectionPool) -> Result<Self, String> {
        pool.execute_raw("BEGIN")?;
        Ok(Self {
            pool,
            transaction_open: true,
        })
    }

    /// Simplified domain API hiding the raw SQL.
    pub fn add_user(&self, username: &str) -> Result<(), String> {
        self.pool
            .execute_raw(&format!("INSERT INTO users (name) VALUES ('{}')", username))
    }

    pub fn commit(mut self) -> Result<(), String> {
        // TRADEOFF: The Session pattern means the connection is tied up for the
        // duration of the Session's lifetime. In highly concurrent async systems,
        // holding a connection across an await point can lead to pool starvation.
        self.pool.execute_raw("COMMIT")?;
        self.transaction_open = false; // Prevent rollback
        Ok(()) // `self` is consumed here, Facade ceases to exist.
    }
}

// **PRODUCTION NOTE:** The RAII Drop pattern is the ultimate safety net for Facades
// that manage stateful subsystems.
impl<'a> Drop for UserSession<'a> {
    fn drop(&mut self) {
        if self.transaction_open {
            let _ = self.pool.execute_raw("ROLLBACK");
            println!("Facade (UserSession): Implicitly rolled back transaction on drop.");
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_computer_facade() {
        let cpu = Cpu { temperature: 35 };
        let mut ram = Memory {
            capacity: 16_000_000,
        };
        let disk = HardDrive { sector_size: 512 };

        // Scope the facade
        {
            let mut computer = ComputerFacade::new(&cpu, &mut ram, &disk);
            computer.start();
            // GOTCHA: While `computer` exists, we cannot use `ram` directly because
            // it is mutably borrowed by the Facade.
            // ram.capacity = 0; // Compile error
        }

        // Facade is dropped, we regain full access to RAM.
        ram.capacity = 0;
        assert_eq!(ram.capacity, 0);
    }

    #[test]
    fn test_session_facade_commit() {
        let pool = ConnectionPool { connected: true };
        let session = UserSession::begin(&pool).unwrap();

        assert!(session.add_user("alice").is_ok());

        // Consumes the session
        assert!(session.commit().is_ok());
    }

    #[test]
    fn test_session_facade_rollback() {
        let pool = ConnectionPool { connected: true };
        {
            let session = UserSession::begin(&pool).unwrap();
            assert!(session.add_user("bob").is_ok());
            // Session goes out of scope without calling `commit()`.
            // Drop trait executes "ROLLBACK".
        }
    }
}

// ============================================================================
// Footer
// ============================================================================
//
// How this pattern appears in std/major crates:
// - **sqlx / diesel:** `Transaction` structs are Facades over connection pools that rollback on Drop.
// - **git2:** `git2::Repository` acts as a high-level Facade over the complex libgit2 C API.
//
// What the GoF/OOP equivalent is and why it doesn't translate directly:
// In OOP, a Facade is often just a class full of methods that permanently owns its dependencies.
// In Rust, Facades are often *temporary, scoped wrappers* (Sessions) bound by lifetimes (`'a`).
// This allows Rust to use the compiler to prevent concurrent mutation of the subsystems while
// the Facade is orchestrating them.
//
// When to reach for this vs simpler alternatives:
// Use a Facade when a subsystem requires an exact sequence of complex operations (like booting
// a computer or managing a database transaction) and you want to ensure clients can't mess
// up the order of those operations.
//
// Suggested combinations with other patterns in this collection:
// - **Builder Pattern**: A Builder can be used to construct the complex subsystems before passing them to the Facade.
// - **Proxy Pattern**: A Facade simplifies an API; a Proxy controls access to it. They are often used together (e.g., `MutexGuard` is a proxy that acts like a facade over raw memory).
