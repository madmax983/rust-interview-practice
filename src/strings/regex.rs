//! # Regular Expression Engine (NFA-based)
//!
//! # Header
//!
//! *   **Implements**: Regular Expression Engine (Thompson's Construction)
//! *   **Replaces Crates**: `regex` (partial), `pcre2`
//! *   **Real-world Usage**: Text editors (grep, sed), Lexers, Network packet filtering (DPI).
//! *   **Why build it yourself?**: To understand how regex engines can guarantee O(MN) performance and avoid "Catastrophic Backtracking" vulnerabilities present in many recursive engines.
//!
//! # Architecture
//!
//! This implementation follows Ken Thompson's Construction to build a Non-deterministic Finite Automaton (NFA)
//! from a Regular Expression.
//!
//! **Components:**
//! 1.  **Parser**: Converts Infix Regex (`a(b|c)*`) -> Postfix (`abc|*`). Uses Shunting-Yard.
//! 2.  **Compiler**: Converts Postfix -> NFA (Graph of States). Uses Thompson's Construction.
//! 3.  **VM (Executor)**: Simulates the NFA on the input string step-by-step.
//!
//! **Data Structure (NFA):**
//! We use an index-based Arena (`Vec<State>`) to avoid self-referential structs and simplify memory management.
//!
//! ```text
//! State Enum:
//! - Literal(char, out): Match char, go to `out`.
//! - Wildcard(out): Match any char, go to `out`.
//! - Split(out1, out2): Epsilon transition to `out1` or `out2`.
//! - Match: Accept state.
//! ```
//!
//! **Invariants:**
//! *   The NFA has exactly one start state.
//! *   `Match` state is the final destination.
//! *   No cycles of only epsilon transitions that cause infinite recursion (handled by visited set).
//!
//! **Complexity:**
//!
//! | Operation | Time | Space |
//! | :--- | :--- | :--- |
//! | Compile | O(M) | O(M) |
//! | Match | O(MN) | O(M) |
//!
//! M = regex length, N = text length.


/// Represents a state in the NFA.
#[derive(Debug, Clone, PartialEq)]
pub enum State {
    /// Match a specific character and transition to the next state index.
    Literal(char, usize),
    /// Match any character (wildcard `.`) and transition to next.
    Wildcard(usize),
    /// Split execution into two paths (epsilon transition).
    Split(usize, usize),
    /// Successfully matched the regex.
    Match,
}

/// A compiled Regular Expression.
#[derive(Debug, Clone)]
pub struct Regex {
    /// The NFA states (arena).
    pub nfa: Vec<State>,
    /// The starting state index.
    pub start: usize,
}

impl Regex {
    /// Compiles a regex pattern into an NFA.
    pub fn new(pattern: &str) -> Result<Self, String> {
        let postfix = infix_to_postfix(pattern)?;
        compile(&postfix)
    }

    /// Returns true if the text matches the regex pattern.
    /// Note: This performs a full string match (anchored).
    pub fn is_match(&self, text: &str) -> bool {
        let mut clist = Vec::with_capacity(self.nfa.len());
        let mut nlist = Vec::with_capacity(self.nfa.len());

        // Use generation-based visited check to avoid clearing a large vec every step.
        let mut visited = vec![0usize; self.nfa.len()];
        let mut generation = 1;

        // Initial states (epsilon closure of start)
        self.add_state(&mut clist, &mut visited, generation, self.start);
        generation += 1;

        for c in text.chars() {
            if clist.is_empty() {
                // Optimization: No active states, can't match.
                return false;
            }

            for &s_idx in &clist {
                match &self.nfa[s_idx] {
                    State::Literal(ch, next) if *ch == c => {
                        self.add_state(&mut nlist, &mut visited, generation, *next);
                    }
                    State::Wildcard(next) => {
                        self.add_state(&mut nlist, &mut visited, generation, *next);
                    }
                    _ => {
                        // Match state or non-matching Literal do nothing here.
                    }
                }
            }

            std::mem::swap(&mut clist, &mut nlist);
            nlist.clear();
            generation += 1;
        }

        // Check if any current state is a Match state
        clist.iter().any(|&idx| matches!(self.nfa[idx], State::Match))
    }

    // Helper to add state to list, resolving epsilon transitions.
    fn add_state(&self, list: &mut Vec<usize>, visited: &mut Vec<usize>, generation: usize, idx: usize) {
        if visited[idx] == generation {
            return;
        }
        visited[idx] = generation;

        match &self.nfa[idx] {
            State::Split(out1, out2) => {
                self.add_state(list, visited, generation, *out1);
                self.add_state(list, visited, generation, *out2);
            }
            _ => {
                list.push(idx);
            }
        }
    }
}

// RUST INSIGHT: We use private Use Area characters to represent operators in Postfix.
// This allows any standard character (including `|`, `*`, `.`) to be treated as a literal
// if it appears in the postfix string, solving the escaping ambiguity.
const OP_UNION: char = '\u{E001}';
const OP_CONCAT: char = '\u{E002}';
const OP_STAR: char = '\u{E003}';
const OP_PLUS: char = '\u{E004}';
const OP_QUEST: char = '\u{E005}';
const OP_WILDCARD: char = '\u{E006}';

/// Converts an infix regex pattern to postfix notation using the Shunting-Yard algorithm.
/// Explicitly inserts concatenation operators where needed.
/// Handles escaping via `\`.
fn infix_to_postfix(re: &str) -> Result<String, String> {
    let mut output = String::new();
    let mut operators: Vec<char> = Vec::new();

    let mut previous_was_concat_source = false;
    let mut chars_iter = re.chars();

    while let Some(c) = chars_iter.next() {
         let mut is_escaped = false;
         let char_to_process = if c == '\\' {
             is_escaped = true;
             match chars_iter.next() {
                 Some(escaped) => escaped,
                 None => return Err("Trailing backslash".to_string()),
             }
         } else {
             c
         };

         if is_escaped {
             // It is a literal even if it looks like an operator
             handle_literal(char_to_process, &mut output, &mut operators, &mut previous_was_concat_source);
         } else {
             match char_to_process {
                 '|' => {
                     previous_was_concat_source = false;
                     while let Some(&op) = operators.last() {
                        if op == '(' { break; }
                        if precedence(op) >= precedence(OP_UNION) {
                            output.push(operators.pop().unwrap());
                        } else {
                            break;
                        }
                     }
                     operators.push(OP_UNION);
                 },
                 '*' => handle_unary_op(OP_STAR, &mut output, &mut operators, &mut previous_was_concat_source),
                 '+' => handle_unary_op(OP_PLUS, &mut output, &mut operators, &mut previous_was_concat_source),
                 '?' => handle_unary_op(OP_QUEST, &mut output, &mut operators, &mut previous_was_concat_source),
                 '(' => {
                     if previous_was_concat_source {
                         handle_concat(&mut output, &mut operators);
                     }
                     operators.push('(');
                     previous_was_concat_source = false;
                 },
                 ')' => {
                     while let Some(op) = operators.pop() {
                         if op == '(' { break; }
                         output.push(op);
                     }
                     previous_was_concat_source = true;
                 },
                 '.' => {
                     // Wildcard is treated as a literal in terms of concatenation source,
                     // but pushes a special OP_WILDCARD token to output.
                     if previous_was_concat_source {
                         handle_concat(&mut output, &mut operators);
                     }
                     output.push(OP_WILDCARD);
                     previous_was_concat_source = true;
                 }
                 _ => {
                     handle_literal(char_to_process, &mut output, &mut operators, &mut previous_was_concat_source);
                 }
             }
         }
    }

    while let Some(op) = operators.pop() {
        if op == '(' { return Err("Mismatched parentheses".to_string()); }
        output.push(op);
    }

    Ok(output)
}

fn handle_unary_op(op_char: char, output: &mut String, operators: &mut Vec<char>, previous_was_concat_source: &mut bool) {
    while let Some(&op) = operators.last() {
       if op == '(' { break; }
       if precedence(op) >= precedence(op_char) {
           output.push(operators.pop().unwrap());
       } else {
           break;
       }
    }
    operators.push(op_char);
    *previous_was_concat_source = true;
}

fn handle_literal(c: char, output: &mut String, operators: &mut Vec<char>, previous_was_concat_source: &mut bool) {
     if *previous_was_concat_source {
         handle_concat(output, operators);
     }
     output.push(c);
     *previous_was_concat_source = true;
}

fn handle_concat(output: &mut String, operators: &mut Vec<char>) {
     while let Some(&op) = operators.last() {
        if op == '(' { break; }
        if precedence(op) >= precedence(OP_CONCAT) {
            output.push(operators.pop().unwrap());
        } else {
            break;
        }
     }
     operators.push(OP_CONCAT);
}

fn precedence(op: char) -> u8 {
    match op {
        OP_UNION => 1,
        OP_CONCAT => 2,
        OP_STAR | OP_PLUS | OP_QUEST => 3,
        _ => 0,
    }
}

#[derive(Clone, Copy, Debug)]
enum Patch {
    Literal(usize),
    Wildcard(usize),
    Split2(usize),
}

struct Fragment {
    start: usize,
    outs: Vec<Patch>,
}

fn compile(postfix: &str) -> Result<Regex, String> {
    let mut nfa: Vec<State> = Vec::new();
    let mut stack: Vec<Fragment> = Vec::new();

    for c in postfix.chars() {
        match c {
            OP_WILDCARD => {
                let idx = nfa.len();
                nfa.push(State::Wildcard(0));
                stack.push(Fragment {
                    start: idx,
                    outs: vec![Patch::Wildcard(idx)],
                });
            }
            OP_CONCAT => {
                if stack.len() < 2 {
                    return Err("Invalid regex: missing operands for concatenation".to_string());
                }
                let frag2 = stack.pop().unwrap();
                let frag1 = stack.pop().unwrap();

                patch(&mut nfa, &frag1.outs, frag2.start);
                stack.push(Fragment {
                    start: frag1.start,
                    outs: frag2.outs,
                });
            }
            OP_UNION => {
                if stack.len() < 2 {
                    return Err("Invalid regex: missing operands for union".to_string());
                }
                let frag2 = stack.pop().unwrap();
                let frag1 = stack.pop().unwrap();

                let idx = nfa.len();
                nfa.push(State::Split(frag1.start, frag2.start));

                let mut outs = frag1.outs;
                outs.extend(frag2.outs);

                stack.push(Fragment {
                    start: idx,
                    outs,
                });
            }
            OP_STAR => {
                if stack.is_empty() {
                     return Err("Invalid regex: missing operand for *".to_string());
                }
                let frag = stack.pop().unwrap();
                let idx = nfa.len();

                // Split(frag.start, ?)
                nfa.push(State::Split(frag.start, 0));

                patch(&mut nfa, &frag.outs, idx);

                stack.push(Fragment {
                    start: idx,
                    outs: vec![Patch::Split2(idx)],
                });
            }
            OP_PLUS => {
                if stack.is_empty() {
                     return Err("Invalid regex: missing operand for +".to_string());
                }
                let frag = stack.pop().unwrap();
                let idx = nfa.len();

                // Split(frag.start, ?)
                nfa.push(State::Split(frag.start, 0));

                patch(&mut nfa, &frag.outs, idx);

                stack.push(Fragment {
                    start: frag.start,
                    outs: vec![Patch::Split2(idx)],
                });
            }
            OP_QUEST => {
                if stack.is_empty() {
                     return Err("Invalid regex: missing operand for ?".to_string());
                }
                let frag = stack.pop().unwrap();
                let idx = nfa.len();

                // Split(frag.start, ?)
                nfa.push(State::Split(frag.start, 0));

                let mut outs = frag.outs;
                outs.push(Patch::Split2(idx));

                stack.push(Fragment {
                    start: idx,
                    outs,
                });
            }
            _ => {
                // Literal
                let idx = nfa.len();
                nfa.push(State::Literal(c, 0)); // 0 is placeholder
                stack.push(Fragment {
                    start: idx,
                    outs: vec![Patch::Literal(idx)],
                });
            }
        }
    }

    if stack.len() != 1 {
        return Err("Invalid regex: unbalanced stack".to_string());
    }

    let final_frag = stack.pop().unwrap();
    let match_idx = nfa.len();
    nfa.push(State::Match);

    patch(&mut nfa, &final_frag.outs, match_idx);

    Ok(Regex {
        nfa,
        start: final_frag.start,
    })
}

fn patch(nfa: &mut [State], patches: &[Patch], target: usize) {
    for p in patches {
        match *p {
            Patch::Literal(idx) => {
                if let State::Literal(_, ref mut out) = nfa[idx] {
                    *out = target;
                }
            }
            Patch::Wildcard(idx) => {
                if let State::Wildcard(ref mut out) = nfa[idx] {
                    *out = target;
                }
            }
            Patch::Split2(idx) => {
                if let State::Split(_, ref mut out2) = nfa[idx] {
                    *out2 = target;
                }
            }
        }
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `regex`: Uses a hybrid approach (DFA for speed, NFA for features). Much more complex optimization.
// - `pcre2`: Backtracking engine.
//
// Missing vs. Production:
// - **Character Classes**: `[a-z]` not supported.
// - **Escaping**: Supported (basic).
// - **Anchors**: `^` and `$` not supported explicitly (always anchored).
// - **Capture Groups**: `( ... )` supported for grouping, but not extracting.
// - **DFA**: No DFA compilation for speed.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_literal() {
        let re = Regex::new("a").unwrap();
        assert!(re.is_match("a"));
        assert!(!re.is_match("b"));
        assert!(!re.is_match("aa"));
    }

    #[test]
    fn test_concat() {
        let re = Regex::new("ab").unwrap();
        assert!(re.is_match("ab"));
        assert!(!re.is_match("a"));
        assert!(!re.is_match("b"));
        assert!(!re.is_match("aba"));
    }

    #[test]
    fn test_union() {
        let re = Regex::new("a|b").unwrap();
        assert!(re.is_match("a"));
        assert!(re.is_match("b"));
        assert!(!re.is_match("c"));
        assert!(!re.is_match("ab"));
    }

    #[test]
    fn test_star() {
        let re = Regex::new("a*").unwrap();
        assert!(re.is_match(""));
        assert!(re.is_match("a"));
        assert!(re.is_match("aa"));
        assert!(re.is_match("aaaaa"));
        assert!(!re.is_match("b"));
    }

    #[test]
    fn test_plus() {
        let re = Regex::new("a+").unwrap();
        assert!(!re.is_match(""));
        assert!(re.is_match("a"));
        assert!(re.is_match("aa"));
    }

    #[test]
    fn test_question() {
        let re = Regex::new("a?").unwrap();
        assert!(re.is_match(""));
        assert!(re.is_match("a"));
        assert!(!re.is_match("aa"));
    }

    #[test]
    fn test_wildcard() {
        let re = Regex::new(".").unwrap();
        assert!(re.is_match("a"));
        assert!(re.is_match("b"));
        assert!(!re.is_match(""));
        assert!(!re.is_match("ab"));

        let re = Regex::new(".*").unwrap();
        assert!(re.is_match(""));
        assert!(re.is_match("abc"));
    }

    #[test]
    fn test_precedence() {
        // ab* -> a(b*)
        let re = Regex::new("ab*").unwrap();
        assert!(re.is_match("a"));
        assert!(re.is_match("ab"));
        assert!(re.is_match("abb"));
        assert!(!re.is_match("b")); // b* matches b, but a is missing

        // (ab)* -> matches "", "ab", "abab"
        let re = Regex::new("(ab)*").unwrap();
        assert!(re.is_match(""));
        assert!(re.is_match("ab"));
        assert!(re.is_match("abab"));
        assert!(!re.is_match("a"));
    }

    #[test]
    fn test_complex() {
        // (a|b)*c
        let re = Regex::new("(a|b)*c").unwrap();
        assert!(re.is_match("c"));
        assert!(re.is_match("ac"));
        assert!(re.is_match("bc"));
        assert!(re.is_match("abac"));
        assert!(!re.is_match("ca"));
    }

    #[test]
    fn test_errors() {
        assert!(Regex::new("*").is_err());
        assert!(Regex::new("|").is_err());
        assert!(Regex::new("(").is_err());
        assert!(Regex::new(")").is_err());
    }

    #[test]
    fn test_escaping() {
        let re = Regex::new(r"a\.b").unwrap(); // Matches literal "a.b"
        assert!(re.is_match("a.b"));
        assert!(!re.is_match("axb"));

        let re = Regex::new(r"a\*b").unwrap(); // Matches literal "a*b"
        assert!(re.is_match("a*b"));
        assert!(!re.is_match("aaab"));
    }
}
