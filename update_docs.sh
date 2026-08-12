sed -i 's~// RUST INSIGHT: Memory says:.*~// RUST INSIGHT:\n// Explicitly typing the match arm returns to a byte slice `\&#91;u8\&#93;` allows us to return string literals of different lengths (e.g. b"tree" vs b"commit").~g' src/systems/git_core.rs
sed -i '/"To resolve E0308: match arms/d' src/systems/git_core.rs
sed -i '/to return byte string literals of different lengths/d' src/systems/git_core.rs
sed -i '/explicitly type the resulting variable/d' src/systems/git_core.rs
