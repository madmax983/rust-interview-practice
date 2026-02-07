//! # Serialization Patterns with Serde
//!
//! Comprehensive serde patterns for serialization/deserialization.
//! Requires the `serde-patterns` feature flag.
//!
//! **Enable with:** `cargo build --features serde-patterns`

#![cfg(feature = "serde-patterns")]

use serde::{Deserialize, Serialize};

// ============================================================================
// Serialization Basics
// ============================================================================

/// Basic struct with Serialize and Deserialize.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct User {
    name: String,
    age: u32,
    email: String,
}

#[allow(dead_code)]
fn demonstrate_basic_serde() {
    let user = User {
        name: "Alice".to_string(),
        age: 30,
        email: "alice@example.com".to_string(),
    };

    // JSON
    let json = serde_json::to_string(&user).unwrap();
    println!("JSON: {json}");

    let user_from_json: User = serde_json::from_str(&json).unwrap();
    assert_eq!(user, user_from_json);

    // Pretty JSON
    let pretty_json = serde_json::to_string_pretty(&user).unwrap();
    println!("Pretty JSON:\n{pretty_json}");

    // YAML
    let yaml = serde_yaml::to_string(&user).unwrap();
    println!("YAML:\n{yaml}");

    // TOML
    let toml = toml::to_string(&user).unwrap();
    println!("TOML:\n{toml}");

    // Bincode (binary)
    let binary = bincode::serialize(&user).unwrap();
    println!("Binary length: {} bytes", binary.len());
}

// ============================================================================
// Field Attributes
// ============================================================================

/// Demonstrating field-level serde attributes.
#[derive(Debug, Serialize, Deserialize)]
struct Config {
    // Rename field in serialized form
    #[serde(rename = "userName")]
    user_name: String,

    // Skip serialization entirely
    #[serde(skip)]
    password_hash: String,

    // Skip serialization if value is default
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,

    // Use default value if missing during deserialization
    #[serde(default)]
    enabled: bool,

    // Use custom default function
    #[serde(default = "default_timeout")]
    timeout: u64,

    // Multiple aliases for compatibility
    #[serde(alias = "addr", alias = "address")]
    server_address: String,
}

fn default_timeout() -> u64 {
    30
}

#[allow(dead_code)]
fn demonstrate_field_attributes() {
    let config = Config {
        user_name: "admin".to_string(),
        password_hash: "secret".to_string(),
        description: None,
        enabled: true,
        timeout: 60,
        server_address: "localhost:8080".to_string(),
    };

    let json = serde_json::to_string_pretty(&config).unwrap();
    println!("Config JSON:\n{json}");

    // Deserialize with missing fields (uses defaults)
    let json_minimal = r#"{"userName": "admin", "server_address": "localhost"}"#;
    let config: Config = serde_json::from_str(json_minimal).unwrap();
    assert_eq!(config.timeout, 30); // default_timeout()
    assert!(!config.enabled); // bool default is false
}

// ============================================================================
// Container Attributes
// ============================================================================

/// Container-level attributes affect the entire struct/enum.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")] // Convert snake_case to camelCase
struct ApiResponse {
    user_id: u64,
    first_name: String,
    last_name: String,
    created_at: String,
}

#[allow(dead_code)]
fn demonstrate_container_attributes() {
    let response = ApiResponse {
        user_id: 42,
        first_name: "Alice".to_string(),
        last_name: "Smith".to_string(),
        created_at: "2024-01-01".to_string(),
    };

    let json = serde_json::to_string_pretty(&response).unwrap();
    println!("API Response (camelCase):\n{json}");
    // Output: {"userId": 42, "firstName": "Alice", ...}
}

/// Other rename_all options.
#[allow(dead_code)]
fn demonstrate_rename_all_options() {
    // rename_all options:
    // - "lowercase"
    // - "UPPERCASE"
    // - "PascalCase"
    // - "camelCase"
    // - "snake_case"
    // - "SCREAMING_SNAKE_CASE"
    // - "kebab-case"
    // - "SCREAMING-KEBAB-CASE"
}

/// Deny unknown fields for strict validation.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StrictConfig {
    name: String,
    port: u16,
}

#[allow(dead_code)]
fn demonstrate_deny_unknown_fields() {
    let json_valid = r#"{"name": "server", "port": 8080}"#;
    let config: StrictConfig = serde_json::from_str(json_valid).unwrap();
    println!("Valid config: {config:?}");

    // This would fail:
    // let json_invalid = r#"{"name": "server", "port": 8080, "unknown": true}"#;
    // let config: StrictConfig = serde_json::from_str(json_invalid).unwrap();
    // Error: unknown field `unknown`
}

// ============================================================================
// Enum Serialization
// ============================================================================

/// Tagged enum (default).
#[derive(Debug, Serialize, Deserialize)]
enum Message {
    Text(String),
    Number(i32),
    Coords { x: f64, y: f64 },
}

/// Externally tagged (default behavior).
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
enum Event {
    Click { x: i32, y: i32 },
    KeyPress { key: String },
    Scroll { delta: i32 },
}

/// Internally tagged.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
enum Command {
    Move { x: i32, y: i32 },
    Resize { width: u32, height: u32 },
    Close,
}

/// Untagged enum - tries each variant until one works.
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum Value {
    String(String),
    Number(i32),
    Boolean(bool),
}

#[allow(dead_code)]
fn demonstrate_enum_serialization() {
    // Default (externally tagged)
    let msg = Message::Coords { x: 1.0, y: 2.0 };
    let json = serde_json::to_string(&msg).unwrap();
    println!("Message: {json}");
    // Output: {"Coords":{"x":1.0,"y":2.0}}

    // Internally tagged with "type" field
    let event = Event::Click { x: 10, y: 20 };
    let json = serde_json::to_string(&event).unwrap();
    println!("Event: {json}");
    // Output: {"type":"Click","x":10,"y":20}

    // Tagged with separate content
    let cmd = Command::Move { x: 5, y: 10 };
    let json = serde_json::to_string(&cmd).unwrap();
    println!("Command: {json}");
    // Output: {"type":"Move","data":{"x":5,"y":10}}

    // Untagged - no type discriminator
    let val = Value::Number(42);
    let json = serde_json::to_string(&val).unwrap();
    println!("Value: {json}");
    // Output: 42
}

// ============================================================================
// Flattening for Composition
// ============================================================================

/// Base struct to be flattened.
#[derive(Debug, Serialize, Deserialize)]
struct Metadata {
    created_at: String,
    updated_at: String,
}

/// Struct that flattens another struct.
#[derive(Debug, Serialize, Deserialize)]
struct Document {
    id: u64,
    title: String,

    #[serde(flatten)]
    metadata: Metadata,
}

#[allow(dead_code)]
fn demonstrate_flatten() {
    let doc = Document {
        id: 1,
        title: "My Document".to_string(),
        metadata: Metadata {
            created_at: "2024-01-01".to_string(),
            updated_at: "2024-01-02".to_string(),
        },
    };

    let json = serde_json::to_string_pretty(&doc).unwrap();
    println!("Flattened document:\n{json}");
    // Output has created_at and updated_at at top level:
    // {
    //   "id": 1,
    //   "title": "My Document",
    //   "created_at": "2024-01-01",
    //   "updated_at": "2024-01-02"
    // }
}

// ============================================================================
// Custom Serialization
// ============================================================================

use serde::de::Deserializer;
use serde::ser::Serializer;

/// Custom serialization for a field.
#[derive(Debug, Serialize, Deserialize)]
struct Person {
    name: String,

    // Serialize age as string, deserialize from string
    #[serde(serialize_with = "serialize_age", deserialize_with = "deserialize_age")]
    age: u32,
}

fn serialize_age<S>(age: &u32, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&age.to_string())
}

fn deserialize_age<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    s.parse().map_err(serde::de::Error::custom)
}

#[allow(dead_code)]
fn demonstrate_custom_serialization() {
    let person = Person {
        name: "Bob".to_string(),
        age: 25,
    };

    let json = serde_json::to_string(&person).unwrap();
    println!("Person: {json}");
    // Output: {"name":"Bob","age":"25"} (age is string!)

    let parsed: Person = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.age, 25);
}

// ============================================================================
// Serializing Third-Party Types
// ============================================================================

/// Remote derive pattern for types you don't own.
///
/// ```ignore
/// // Define a "remote" type matching the structure
/// #[derive(Serialize, Deserialize)]
/// #[serde(remote = "ThirdPartyType")]
/// struct ThirdPartyTypeDef {
///     field: String,
/// }
///
/// // Use with serialize_with/deserialize_with
/// struct MyStruct {
///     #[serde(with = "ThirdPartyTypeDef")]
///     value: ThirdPartyType,
/// }
/// ```
#[allow(dead_code)]
fn demonstrate_remote_derive() {
    // Example pattern only - requires matching third-party type structure
}

// ============================================================================
// Multiple Formats
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
struct Data {
    items: Vec<String>,
    count: usize,
}

#[allow(dead_code)]
fn demonstrate_multiple_formats() {
    let data = Data {
        items: vec!["a".to_string(), "b".to_string(), "c".to_string()],
        count: 3,
    };

    // JSON
    let json = serde_json::to_string_pretty(&data).unwrap();
    println!("JSON:\n{json}");

    // YAML
    let yaml = serde_yaml::to_string(&data).unwrap();
    println!("\nYAML:\n{yaml}");

    // TOML
    let toml = toml::to_string(&data).unwrap();
    println!("\nTOML:\n{toml}");

    // Bincode (binary - most efficient)
    let binary = bincode::serialize(&data).unwrap();
    println!("\nBincode: {} bytes", binary.len());
    let data_from_binary: Data = bincode::deserialize(&binary).unwrap();
    assert_eq!(data.count, data_from_binary.count);
}

// ============================================================================
// Versioning and Schema Evolution
// ============================================================================

/// V1 of a struct.
#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
struct UserV1 {
    name: String,
    age: u32,
}

/// V2 with new optional field (backwards compatible).
#[derive(Debug, Serialize, Deserialize)]
struct UserV2 {
    name: String,
    age: u32,

    #[serde(default)]
    email: Option<String>,
}

/// V3 with renamed field (using alias for compatibility).
#[derive(Debug, Serialize, Deserialize)]
struct UserV3 {
    name: String,

    #[serde(alias = "age")] // Accept old name
    user_age: u32,

    #[serde(default)]
    email: Option<String>,
}

#[allow(dead_code)]
fn demonstrate_versioning() {
    // V1 data can be read as V2
    let json_v1 = r#"{"name":"Alice","age":30}"#;
    let user_v2: UserV2 = serde_json::from_str(json_v1).unwrap();
    assert_eq!(user_v2.email, None);

    // V1 data can be read as V3 (alias support)
    let user_v3: UserV3 = serde_json::from_str(json_v1).unwrap();
    assert_eq!(user_v3.user_age, 30);

    println!("V2: {user_v2:?}");
    println!("V3: {user_v3:?}");
}

// ============================================================================
// Validation During Deserialization
// ============================================================================

/// Custom deserialization with validation.
#[derive(Debug, Deserialize)]
struct ValidatedEmail {
    #[serde(deserialize_with = "deserialize_email")]
    email: String,
}

fn deserialize_email<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;

    if s.contains('@') {
        Ok(s)
    } else {
        Err(serde::de::Error::custom("Invalid email format"))
    }
}

#[allow(dead_code)]
fn demonstrate_validation() {
    let json_valid = r#"{"email":"alice@example.com"}"#;
    let valid: ValidatedEmail = serde_json::from_str(json_valid).unwrap();
    println!("Valid email: {}", valid.email);

    // This would fail:
    // let json_invalid = r#"{"email":"not-an-email"}"#;
    // let invalid: ValidatedEmail = serde_json::from_str(json_invalid).unwrap();
    // Error: Invalid email format
}

// ============================================================================
// Common Date/Time Patterns
// ============================================================================

/// Serializing timestamps as strings.
#[derive(Debug, Serialize, Deserialize)]
struct Timestamp {
    // In real code, you'd use chrono or time crate with serde support:
    // #[serde(with = "chrono::serde::ts_seconds")]
    // created_at: DateTime<Utc>,

    // For demonstration:
    created_at: String, // ISO 8601 format
}

#[allow(dead_code)]
fn demonstrate_timestamps() {
    let ts = Timestamp {
        created_at: "2024-01-01T00:00:00Z".to_string(),
    };

    let json = serde_json::to_string(&ts).unwrap();
    println!("Timestamp: {json}");
}

// ============================================================================
// Practical Patterns
// ============================================================================

/// Pattern 1: Config file reading.
#[derive(Debug, Deserialize)]
struct AppConfig {
    database_url: String,
    port: u16,

    #[serde(default = "default_max_connections")]
    max_connections: u32,
}

fn default_max_connections() -> u32 {
    10
}

#[allow(dead_code)]
fn load_config() -> Result<AppConfig, Box<dyn std::error::Error>> {
    let toml_str = r#"
        database_url = "postgres://localhost/db"
        port = 5432
    "#;

    let config: AppConfig = toml::from_str(toml_str)?;
    Ok(config)
}

/// Pattern 2: API response handling.
#[derive(Debug, Deserialize)]
struct ApiUser {
    id: u64,
    username: String,

    #[serde(default)]
    bio: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GenericApiResponse<T> {
    success: bool,
    data: Option<T>,
    error: Option<String>,
}

#[allow(dead_code)]
fn handle_api_response() -> Result<ApiUser, Box<dyn std::error::Error>> {
    let json = r#"{
        "success": true,
        "data": {"id": 1, "username": "alice"},
        "error": null
    }"#;

    let response: GenericApiResponse<ApiUser> = serde_json::from_str(json)?;

    if response.success {
        Ok(response.data.unwrap())
    } else {
        Err(response.error.unwrap().into())
    }
}

/// Pattern 3: Nested structures.
#[derive(Debug, Serialize, Deserialize)]
struct Company {
    name: String,
    employees: Vec<Employee>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Employee {
    name: String,
    role: String,
    salary: u64,
}

#[allow(dead_code)]
fn demonstrate_nested() {
    let company = Company {
        name: "TechCorp".to_string(),
        employees: vec![
            Employee {
                name: "Alice".to_string(),
                role: "Engineer".to_string(),
                salary: 100_000,
            },
            Employee {
                name: "Bob".to_string(),
                role: "Manager".to_string(),
                salary: 120_000,
            },
        ],
    };

    let json = serde_json::to_string_pretty(&company).unwrap();
    println!("Company:\n{json}");
}

// ============================================================================
// Performance Tips
// ============================================================================

#[allow(dead_code)]
fn demonstrate_performance_tips() {
    // Tip 1: Use borrowed types when deserializing for zero-copy
    #[derive(Deserialize)]
    struct BorrowedData<'a> {
        #[serde(borrow)]
        text: &'a str,
    }

    // Tip 2: Use String for owned data
    #[derive(Deserialize)]
    struct OwnedData {
        text: String,
    }

    // Tip 3: Bincode is fastest for Rust-to-Rust communication
    let data = vec![1, 2, 3, 4, 5];
    let _binary = bincode::serialize(&data).unwrap();

    // Tip 4: Use to_writer for streaming large data
    let data = vec!["a", "b", "c"];
    let mut buffer = Vec::new();
    serde_json::to_writer(&mut buffer, &data).unwrap();
}

// ============================================================================
// Usage Notes
// ============================================================================

/// Enable serde patterns:
/// ```bash
/// cargo build --features serde-patterns
/// cargo test --features serde-patterns
/// ```
///
/// Common serde crates:
/// - serde: Core traits
/// - serde_json: JSON support
/// - serde_yaml: YAML support
/// - toml: TOML support
/// - bincode: Binary encoding (fastest)
/// - ron: Rusty Object Notation
/// - postcard: No-std binary format
#[allow(dead_code)]
const SERDE_USAGE: &str = "See module docs for usage";
