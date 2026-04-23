import re

with open('src/systems/deflate.rs', 'r') as f:
    content = f.read()

# Add Compressor trait
trait_def = """
/// A generic trait for data compression algorithms.
/// This allows swappable strategies (e.g., Deflate, LZW, Snappy) without changing consumer code.
pub trait Compressor {
    /// Compresses byte data into a specific format.
    fn compress(data: &[u8]) -> Vec<u8>;

    /// Decompresses data compressed by this algorithm.
    fn decompress(data: &[u8]) -> Result<Vec<u8>, &'static str>;
}

/// Simplified DEFLATE compressor.
pub struct Deflate;

impl Compressor for Deflate {
"""

content = re.sub(r'/// Simplified DEFLATE compressor\.\npub struct Deflate;\n\nimpl Deflate \{', trait_def, content)

# Write back
with open('src/systems/deflate.rs', 'w') as f:
    f.write(content)
