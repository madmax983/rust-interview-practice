// Suppress pedantic and nursery lints for design pattern examples.
#![allow(clippy::pedantic, clippy::nursery, unused)]

//! # Active Record vs Repository Pattern
//!
//! Replaces: **Active Record** (OOP), **Data Access Objects** (DAO)
//!
//! Real Rust usage: `sqlx`, `diesel`, `sea-orm` (which implements Active Record but with significant Rust-specific adjustments)
//!
//! ## Why this pattern exists in Rust specifically
//! In OOP (like Ruby on Rails or Django), the Active Record pattern ties data and database behavior together. A `User` object holds both its fields (`id`, `name`) and methods to interact with the database (`user.save()`, `User.find(1)`).
//!
//! In Rust, Active Record is usually an anti-pattern. If a struct owns a database connection to save itself, it requires `Arc<Mutex<Connection>>` (runtime overhead and locking issues) or lifetime annotations (`&'a Connection`) that poison every function using the data.
//!
//! Rust transforms this into the **Repository Pattern** or **Data Mapper**. Data structs are "dumb" (pure data), and behavior lives in separate structs (like a `UserRepository` or pure functions) that borrow the database connection explicitly.
//!
//! ## Meta-Pattern: Make Illegal States Unrepresentable
//! By decoupling data from the database connection, we can use typestates to represent whether a record exists in the database or is newly created, preventing `save()` on an already identical record, or `update()` on an uninserted record.
//!
//! ## Architecture
//!
//! **Approach 1: The Active Record Anti-Pattern (Shared State Hell)**
//! ```text
//! [ User Data ] + [ Arc<Mutex<DbPool>> ] ---(save())---> Database
//! ```
//!
//! **Approach 2: The Repository Pattern (Idiomatic Rust)**
//! ```text
//! [ DbPool ] --(&DbPool)--> [ UserRepository::save(&user) ]
//!                                  ^
//!                                  |
//!                            [ User Data ]
//! ```
//!
//! **Invariants:**
//! - Data structures (`User`) implement `Serialize`/`Deserialize` and carry zero connection overhead.
//! - The database connection's lifetime is strictly scoped to the transaction/query execution.
//! - Typestates prevent querying missing data.
//!
//! ## When to use
//! - Always prefer the **Repository** or pure function approach when interacting with databases or external APIs.
//! - Only use Active Record in Rust if you are building an explicit ORM framework that abstracts away the underlying engine safely (like `sea-orm`), though even there it is heavily modified.
//!
//! ## Anti-patterns
//! - Storing an `Arc<Mutex<Database>>` inside every instance of your domain model just to call `.save()`.

use std::sync::{Arc, Mutex};

// ============================================================================
// Approach 1: Active Record (The Anti-Pattern)
// ============================================================================

// A mock database connection pool.
#[derive(Clone, Default)]
pub struct MockDbPool;

impl MockDbPool {
    pub fn execute(&self, query: &str) {
        // In a real app, this runs SQL.
    }
}

// ANTI-PATTERN: The User struct contains a shared mutable reference to the database.
// TRADEOFF: Every time we create a User, we clone the Arc. It makes the struct impossible
// to easily serialize (e.g., via Serde) and mixes domain logic with infrastructure.
pub struct ActiveRecordUser {
    pub id: Option<u64>,
    pub name: String,
    // The hidden infrastructure dependency.
    db: Arc<Mutex<MockDbPool>>,
}

impl ActiveRecordUser {
    pub fn new(name: String, db: Arc<Mutex<MockDbPool>>) -> Self {
        Self { id: None, name, db }
    }

    pub fn save(&mut self) {
        let pool = self.db.lock().unwrap();
        if self.id.is_none() {
            // "Insert" logic
            pool.execute(&format!(
                "INSERT INTO users (name) VALUES ('{}')",
                self.name
            ));
            self.id = Some(1); // Mock generated ID
        } else {
            // "Update" logic
            pool.execute(&format!(
                "UPDATE users SET name = '{}' WHERE id = {}",
                self.name,
                self.id.unwrap()
            ));
        }
    }
}

// ============================================================================
// Approach 2: Repository Pattern with Typestate (Idiomatic Rust)
// ============================================================================

// COMPILE-TIME WIN: By using typestates, we physically cannot call `update` on a
// user that hasn't been inserted, nor `insert` on a user that already exists.
pub struct NewUser {
    pub name: String,
}

pub struct ExistingUser {
    pub id: u64,
    pub name: String,
}

// Trait-first design: define the contract for the repository.
pub trait UserRepository {
    // OWNERSHIP INSIGHT: We consume the `NewUser` and return an `ExistingUser`.
    // The compiler prevents us from accidentally reusing the `NewUser` after it's inserted.
    fn insert(&self, user: NewUser) -> ExistingUser;

    // GOTCHA: We take `&ExistingUser`, leaving ownership with the caller,
    // because updating doesn't change the object's identity (its typestate remains `ExistingUser`).
    fn update(&self, user: &ExistingUser);
}

// The Repository owns the data access logic and takes the database connection explicitly.
// OWNERSHIP INSIGHT: The User structs are "dumb" data. They don't know the DB exists.
pub struct PostgresUserRepository {
    pool: MockDbPool,
}

impl PostgresUserRepository {
    pub fn new(pool: MockDbPool) -> Self {
        Self { pool }
    }
}

impl UserRepository for PostgresUserRepository {
    fn insert(&self, user: NewUser) -> ExistingUser {
        self.pool.execute(&format!(
            "INSERT INTO users (name) VALUES ('{}')",
            user.name
        ));

        ExistingUser {
            id: 1, // Mock generated ID
            name: user.name,
        }
    }

    fn update(&self, user: &ExistingUser) {
        self.pool.execute(&format!(
            "UPDATE users SET name = '{}' WHERE id = {}",
            user.name, user.id
        ));
    }
}

// Second implementation to prove the abstraction works (e.g. for testing).
pub struct DefaultUserRepository;

impl Default for DefaultUserRepository {
    fn default() -> Self {
        Self
    }
}

impl UserRepository for DefaultUserRepository {
    fn insert(&self, user: NewUser) -> ExistingUser {
        // Mock insertion logic
        ExistingUser {
            id: 99,
            name: user.name,
        }
    }

    fn update(&self, _user: &ExistingUser) {
        // Mock update logic
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_active_record_anti_pattern() {
        let pool = Arc::new(Mutex::new(MockDbPool));

        let mut user = ActiveRecordUser::new("Alice".to_string(), Arc::clone(&pool));
        assert!(user.id.is_none());

        // Mutates the user and interacts with the DB silently.
        user.save();
        assert_eq!(user.id, Some(1));
    }

    #[test]
    fn test_repository_pattern_postgres() {
        let pool = MockDbPool;
        let repo = PostgresUserRepository::new(pool);

        let new_user = NewUser {
            name: "Bob".to_string(),
        };

        // COMPILE ERROR if uncommented:
        // repo.update(&new_user); // Expected `ExistingUser`, found `NewUser`

        let mut existing_user = repo.insert(new_user);
        assert_eq!(existing_user.id, 1);

        // Update works because typestate is correct.
        existing_user.name = "Robert".to_string();
        repo.update(&existing_user);
    }

    #[test]
    fn test_repository_pattern_in_memory() {
        let repo = DefaultUserRepository::default();

        let new_user = NewUser {
            name: "Alice".to_string(),
        };
        let existing_user = repo.insert(new_user);
        assert_eq!(existing_user.id, 99);

        repo.update(&existing_user);
    }
}

// ============================================================================
// Footer
// ============================================================================
//
// How this pattern appears in std/major crates:
// - **sqlx:** Completely separates queries from data structures. You pass the connection explicitly to `query!(...).execute(&mut conn)`.
// - **diesel:** Uses traits and typed queries, completely separating the `Insertable` structs from the database connection.
//
// What the GoF/OOP equivalent is and why it doesn't translate directly:
// The OOP Active Record pattern encourages objects to manage their own persistence (`this.save()`). In Rust, an object holding a reference to a database connection creates massive lifetime complexity (self-referential structs or viral lifetimes) or forces runtime overhead (`Arc<Mutex>`).
//
// When to reach for this vs simpler alternatives:
// Always default to separating your pure data structs from your I/O operations. It makes testing easier, reduces `Arc<Mutex>` overhead, and aligns perfectly with Rust's ownership model.
