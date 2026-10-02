//! Runs one aliasing counterexample from
//! `rust_interview_practice::unsafe_semantics::counterexamples`.
//!
//! ```bash
//! cargo run --bin miri_counterexample -- --list
//! cargo +nightly miri run --bin miri_counterexample -- protector_violation
//! MIRIFLAGS=-Zmiri-tree-borrows cargo +nightly miri run --bin miri_counterexample -- out_of_range_raw
//! cargo run --bin miri_counterexample -- out_of_range_raw --sound
//! ```
//!
//! The unsound half of a counterexample has undefined behaviour, so this binary
//! refuses to run it unless it is being interpreted by Miri.

use std::process::ExitCode;

use rust_interview_practice::unsafe_semantics::counterexamples::{CATALOG, find};

fn usage() -> ExitCode {
    eprintln!("usage: miri_counterexample --list | <name> [--sound]");
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (name, sound) = match args.as_slice() {
        [flag] if flag == "--list" => {
            // Machine-readable: `<name> <stacked> <tree>` per line.
            for c in CATALOG {
                println!("{} {} {}", c.name, c.stacked, c.tree);
            }
            return ExitCode::SUCCESS;
        }
        [name] => (name, false),
        [name, flag] if flag == "--sound" => (name, true),
        _ => return usage(),
    };
    let Some(example) = find(name) else {
        eprintln!("unknown counterexample `{name}`; try --list");
        return ExitCode::from(2);
    };

    let value = if sound {
        (example.sound)()
    } else if cfg!(miri) {
        // SAFETY: none — this is deliberately UB, and we are running under Miri,
        // which will detect it and abort with a diagnostic instead of executing it.
        unsafe { (example.unsound)() }
    } else {
        eprintln!(
            "refusing to run `{name}` natively: it has undefined behaviour.\n\
             run it under Miri instead: cargo +nightly miri run --bin miri_counterexample -- {name}"
        );
        return ExitCode::from(2);
    };
    println!("{name}: {} -> {value}", example.lesson);
    ExitCode::SUCCESS
}
