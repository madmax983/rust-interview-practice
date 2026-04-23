with open('src/systems/deflate.rs', 'r') as f:
    content = f.read()

content = content.replace("    pub fn compress(data: &[u8]) -> Vec<u8> {", "    fn compress(data: &[u8]) -> Vec<u8> {")
content = content.replace("    pub fn decompress(data: &[u8]) -> Result<Vec<u8>, &'static str> {", "    fn decompress(data: &[u8]) -> Result<Vec<u8>, &'static str> {")

# There is a doc comment issue
content = content.replace("/// Naive LZ77 sliding window compression.\n}\n\nimpl Deflate {\n    fn lz77_compress(data: &[u8]) -> Vec<Lz77Token> {", "}\n\nimpl Deflate {\n    /// Naive LZ77 sliding window compression.\n    fn lz77_compress(data: &[u8]) -> Vec<Lz77Token> {")
content = content.replace("    /// Naive LZ77 sliding window compression.\n    fn lz77_compress(data: &[u8]) -> Vec<Lz77Token> {", "}\n\nimpl Deflate {\n    /// Naive LZ77 sliding window compression.\n    fn lz77_compress(data: &[u8]) -> Vec<Lz77Token> {")

with open('src/systems/deflate.rs', 'w') as f:
    f.write(content)
