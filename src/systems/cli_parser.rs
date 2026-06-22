//! # CLI Argument Parser Implementation
//!
//! A configuration-based command-line argument parser that handles short flags,
//! long options, and positional arguments.
//!
//! **Replaces Crates:** `clap`, `structopt`, `argh`
//!
//! **Real-world Usage:**
//! - Any command-line tool (e.g., `git`, `docker`, `cargo`).
//! - Microservice entrypoints needing configuration.
//!
//! **Why build it yourself?**
//! Parsing `std::env::args()` manually is error-prone. Building a parser teaches you about
//! stateful iteration (handling `--port 8080` where one argument defines the next),
//! parsing combined flags (`-xvf`), and distinguishing options from positional arguments,
//! especially the POSIX standard `--` separator.

use std::collections::{HashMap, HashSet};

// =========================================================================================
// Architecture
// =========================================================================================
//
// Configuration:
// We define `ArgType` (Flag, Option).
// We map short names (e.g., 'p') and long names (e.g., "port") to a single logical key.
//
// Flow:
// 1. Iterate through `std::env::args()` (or a mocked vector of strings).
// 2. Check for `--` to treat all subsequent args as positional.
// 3. Handle Long Options (`--port=8080` or `--port 8080`).
// 4. Handle Short Options (`-p8080`, `-p 8080`, or combined flags `-xvf`).
// 5. Handle Positional Arguments (anything not starting with `-`, or after `--`).
//
// Invariants:
// 1. A required option MUST have a value.
// 2. Flags are boolean and do not consume the next argument.
// 3. Combined short flags (`-aux`) are all flags, EXCEPT the last one which may be an option taking a value (`-p8080`).
//
// Complexity:
// ┌───────────┬──────────────┬────────┐
// │ Operation │ Time         │ Space  │
// ├───────────┼──────────────┼────────┤
// │ parse     │ O(N * M)     │ O(N)   │
// └───────────┴──────────────┴────────┘
// N = number of arguments, M = length of combined short flags. Space for storing results.

#[derive(Debug, Clone, PartialEq)]
pub enum ArgType {
    /// A boolean flag (e.g., `--verbose`, `-v`)
    Flag,
    /// An option taking a value (e.g., `--port 80`, `-p80`)
    Option,
}

#[derive(Debug, Clone)]
pub struct ArgConfig {
    pub key: String,
    pub short: Option<char>,
    pub long: Option<String>,
    pub arg_type: ArgType,
}

#[derive(Debug, Default, PartialEq)]
pub struct ParseResult {
    /// Stores the presence of boolean flags. Key is the logical `key`.
    pub flags: HashSet<String>,
    /// Stores options with their values. Key is the logical `key`.
    pub options: HashMap<String, String>,
    /// Stores positional arguments in order.
    pub positional: Vec<String>,
}

pub struct CliParser {
    configs: Vec<ArgConfig>,
    // Quick lookups
    short_map: HashMap<char, usize>,
    long_map: HashMap<String, usize>,
}

impl Default for CliParser {
    fn default() -> Self {
        Self::new()
    }
}

impl CliParser {
    pub fn new() -> Self {
        Self {
            configs: Vec::new(),
            short_map: HashMap::new(),
            long_map: HashMap::new(),
        }
    }

    /// Registers a new argument configuration.
    pub fn add_arg(&mut self, config: ArgConfig) {
        let idx = self.configs.len();
        if let Some(short) = config.short {
            self.short_map.insert(short, idx);
        }
        if let Some(long) = &config.long {
            self.long_map.insert(long.clone(), idx);
        }
        self.configs.push(config);
    }

    /// Parses a sequence of string arguments.
    pub fn parse<I, S>(&self, args: I) -> Result<ParseResult, String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut result = ParseResult::default();
        let mut arg_iter = args.into_iter();
        let mut positional_only = false;

        while let Some(arg_str) = arg_iter.next() {
            let arg = arg_str.as_ref();

            if positional_only {
                result.positional.push(arg.to_string());
                continue;
            }

            if arg == "--" {
                positional_only = true;
                continue;
            }

            if let Some(kv) = arg.strip_prefix("--") {
                // Long Option
                if kv.is_empty() {
                    return Err("Invalid argument: '--'".to_string());
                }

                let (key, value) = if let Some(idx) = kv.find('=') {
                    (&kv[..idx], Some(&kv[idx + 1..]))
                } else {
                    (kv, None)
                };

                let config_idx = self
                    .long_map
                    .get(key)
                    .ok_or_else(|| format!("Unknown option: --{}", key))?;
                let config = &self.configs[*config_idx];

                match config.arg_type {
                    ArgType::Flag => {
                        if value.is_some() {
                            return Err(format!("Flag --{} does not take a value", key));
                        }
                        result.flags.insert(config.key.clone());
                    }
                    ArgType::Option => {
                        let val = if let Some(v) = value {
                            v.to_string()
                        } else {
                            // Take the next argument as the value
                            arg_iter
                                .next()
                                .ok_or_else(|| format!("Option --{} requires a value", key))?
                                .as_ref()
                                .to_string()
                        };
                        result.options.insert(config.key.clone(), val);
                    }
                }
            } else if arg.starts_with('-') && arg.len() > 1 {
                // Short Option(s)
                // ⚡ BOLT OPTIMIZATION: Avoid intermediate `.collect::<Vec<char>>()` and `.collect::<String>()` allocations.
                // We iterate over `char_indices` to process short options and efficiently slice `arg` for attached values.
                let char_indices = arg[1..].char_indices();

                for (idx, c) in char_indices {
                    let config_idx = self
                        .short_map
                        .get(&c)
                        .ok_or_else(|| format!("Unknown short option: -{}", c))?;
                    let config = &self.configs[*config_idx];

                    match config.arg_type {
                        ArgType::Flag => {
                            result.flags.insert(config.key.clone());
                        }
                        ArgType::Option => {
                            // If it's an Option, it either takes the rest of this string as value
                            // (e.g., `-p8080`) OR the next argument.
                            let rest = &arg[1 + idx + c.len_utf8()..];
                            if !rest.is_empty() {
                                // Value is the rest of the string
                                result.options.insert(config.key.clone(), rest.to_string());
                                break; // Consumed the rest of the characters
                            } else {
                                // Value is the next argument
                                let val = arg_iter
                                    .next()
                                    .ok_or_else(|| format!("Option -{} requires a value", c))?
                                    .as_ref()
                                    .to_string();
                                result.options.insert(config.key.clone(), val);
                            }
                        }
                    }
                }
            } else {
                // Positional Argument
                result.positional.push(arg.to_string());
            }
        }

        Ok(result)
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `clap`: A massive, feature-rich library that handles subcommands, help text generation,
//   environment variable fallbacks, type parsing, and validation.
//   It uses macros/derive to automatically build structs from CLI args.
//
// Missing vs. Production:
// - **Type Parsing**: Our parser returns Strings. Production parsers convert to `u32`, `PathBuf`, etc.
// - **Subcommands**: E.g., `git commit`. We only parse a flat list.
// - **Help Generation**: No auto-generated `-h/--help` text.
// - **Required/Default Values**: No built-in validation for required fields.
//
// Next Steps:
// 1. Add `Type` parsing support (via a `FromStr` closure or trait).
// 2. Implement subcommands by allowing hierarchical parsers.

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_parser() -> CliParser {
        let mut parser = CliParser::new();
        parser.add_arg(ArgConfig {
            key: "verbose".to_string(),
            short: Some('v'),
            long: Some("verbose".to_string()),
            arg_type: ArgType::Flag,
        });
        parser.add_arg(ArgConfig {
            key: "port".to_string(),
            short: Some('p'),
            long: Some("port".to_string()),
            arg_type: ArgType::Option,
        });
        parser.add_arg(ArgConfig {
            key: "host".to_string(),
            short: None,
            long: Some("host".to_string()),
            arg_type: ArgType::Option,
        });
        parser.add_arg(ArgConfig {
            key: "extract".to_string(),
            short: Some('x'),
            long: None,
            arg_type: ArgType::Flag,
        });
        parser
    }

    #[test]
    fn test_long_options() {
        let parser = setup_parser();
        let args = vec!["--verbose", "--port", "8080", "--host=localhost"];
        let result = parser.parse(args).unwrap();

        assert!(result.flags.contains("verbose"));
        assert_eq!(result.options.get("port").unwrap(), "8080");
        assert_eq!(result.options.get("host").unwrap(), "localhost");
    }

    #[test]
    fn test_short_options() {
        let parser = setup_parser();
        // Combined flags (-xv) and option taking the next arg (-p 80)
        let args = vec!["-xv", "-p", "80"];
        let result = parser.parse(args).unwrap();

        assert!(result.flags.contains("extract"));
        assert!(result.flags.contains("verbose"));
        assert_eq!(result.options.get("port").unwrap(), "80");
    }

    #[test]
    fn test_short_option_attached_value() {
        let parser = setup_parser();
        // Option value attached to short option (-p8080)
        let args = vec!["-vp8080"];
        let result = parser.parse(args).unwrap();

        assert!(result.flags.contains("verbose"));
        assert_eq!(result.options.get("port").unwrap(), "8080");
    }

    #[test]
    fn test_positional_args() {
        let parser = setup_parser();
        let args = vec!["--verbose", "file1.txt", "-p", "80", "file2.txt"];
        let result = parser.parse(args).unwrap();

        assert_eq!(result.positional, vec!["file1.txt", "file2.txt"]);
    }

    #[test]
    fn test_double_dash_separator() {
        let parser = setup_parser();
        let args = vec!["-v", "--", "-p", "8080", "--verbose"];
        let result = parser.parse(args).unwrap();

        assert!(result.flags.contains("verbose"));
        assert!(!result.options.contains_key("port"));
        assert_eq!(result.positional, vec!["-p", "8080", "--verbose"]);
    }

    #[test]
    fn test_errors() {
        let parser = setup_parser();

        // Unknown option
        assert!(parser.parse(vec!["--unknown"]).is_err());
        assert!(parser.parse(vec!["-u"]).is_err());

        // Missing value for option
        assert!(parser.parse(vec!["--port"]).is_err());
        assert!(parser.parse(vec!["-p"]).is_err());

        // Value provided to flag
        assert!(parser.parse(vec!["--verbose=true"]).is_err());
    }
}
