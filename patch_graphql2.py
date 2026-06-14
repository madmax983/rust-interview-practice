import re

with open("src/systems/graphql.rs", "r") as f:
    content = f.read()

# remove all the benchmark note tests that got mistakenly added inside non-test structs
# and just leave one at the end of the file.
# The easiest way is to remove all occurrences of it
benchmark_note = """
    #[test]
    fn test_benchmark_note() {
        // BENCHMARKING NOTE:
        // To benchmark this executor against `async-graphql`:
        // 1. Use `criterion::Criterion`.
        // 2. Parse a deeply nested query (e.g., `query { users { friends { friends { name } } } }`).
        // 3. Compare the time taken by `Executor::execute` vs `async-graphql::Schema::execute`.
        // Expected result: This dynamic dispatch approach will be significantly slower than
        // `async-graphql`'s statically generated resolvers, especially on large lists, due to virtual method call overhead.
    }"""

content = content.replace(benchmark_note, "")

# And then append it to the end before the last closing brace
content = content.rstrip()
if content.endswith("}"):
    content = content[:-1] + benchmark_note + "\n}\n"

with open("src/systems/graphql.rs", "w") as f:
    f.write(content)
