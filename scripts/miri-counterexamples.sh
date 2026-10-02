#!/usr/bin/env bash
# Run every unsafe_semantics counterexample under Miri with both aliasing models
# and check the verdict matches the one recorded in the catalog.
#
#   scripts/miri-counterexamples.sh            # all counterexamples
#   scripts/miri-counterexamples.sh NAME...    # just these
#
# Uses the default toolchain if it has Miri, otherwise `+nightly`.
set -euo pipefail
cd "$(dirname "$0")/.."

if cargo miri --version >/dev/null 2>&1; then
    MIRI=(cargo miri)
else
    MIRI=(cargo +nightly miri)
fi

"${MIRI[@]}" setup >/dev/null 2>&1 || "${MIRI[@]}" setup

# The catalog listing is plain data, so it is safe to produce natively.
mapfile -t catalog < <(cargo run --quiet --bin miri_counterexample -- --list)

failures=0
log="$(mktemp)"
trap 'rm -f "$log"' EXIT

for line in "${catalog[@]}"; do
    read -r name expected_sb expected_tb <<<"$line"
    if [[ $# -gt 0 && ! " $* " == *" $name "* ]]; then
        continue
    fi
    for model in stacked tree; do
        if [[ $model == stacked ]]; then
            flags="" expected=$expected_sb
        else
            flags="-Zmiri-tree-borrows" expected=$expected_tb
        fi
        if MIRIFLAGS="$flags" "${MIRI[@]}" run --quiet --bin miri_counterexample -- "$name" >"$log" 2>&1; then
            actual=ok
        elif grep -q "Undefined Behavior" "$log"; then
            actual=ub
        else
            echo "ERROR  $name [$model]: Miri failed without reporting UB:"
            cat "$log"
            failures=$((failures + 1))
            continue
        fi
        if [[ $actual == "$expected" ]]; then
            printf 'ok     %-32s %-8s %s\n' "$name" "$model" "$actual"
        else
            printf 'FAIL   %-32s %-8s expected %s, Miri says %s\n' "$name" "$model" "$expected" "$actual"
            cat "$log"
            failures=$((failures + 1))
        fi
    done
done

if [[ $failures -gt 0 ]]; then
    echo "$failures verdict(s) disagree with the catalog"
    exit 1
fi
echo "all verdicts match the catalog"
