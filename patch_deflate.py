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
    /// Compresses byte data into our custom DEFLATE format.
    fn compress(data: &[u8]) -> Vec<u8> {
"""

content = re.sub(r'/// Simplified DEFLATE compressor\.\npub struct Deflate;\n\nimpl Deflate \{\n    /// Compresses byte data into our custom DEFLATE format\.\n    pub fn compress\(data: &\[u8\]\) -> Vec<u8> \{', trait_def, content)

# Change pub fn decompress to fn decompress
content = content.replace("    pub fn decompress(data: &[u8]) -> Result<Vec<u8>, &'static str> {", "    fn decompress(data: &[u8]) -> Result<Vec<u8>, &'static str> {")

# Move private methods out of the trait impl into a separate impl block
end_of_decompress_idx = content.find("    fn lz77_compress(data: &[u8]) -> Vec<Lz77Token> {")
if end_of_decompress_idx != -1:
    content = content[:end_of_decompress_idx] + "}\n\nimpl Deflate {\n" + content[end_of_decompress_idx:]

with open('src/systems/deflate.rs', 'w') as f:
    f.write(content)
