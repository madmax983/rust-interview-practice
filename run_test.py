import subprocess
import time

code = """
fn main() {
    let mut hasher = rust_interview_practice::cryptography::sha256::Sha256::new();
    let data = vec![b'a'; 10_000_000];
    hasher.update(&data);
    let hash = hasher.finalize();
    println!("{:?}", hash);
}
"""

with open("tests/bench_sha256.rs", "w") as f:
    f.write(code)

start = time.time()
subprocess.run(["cargo", "test", "--test", "bench_sha256", "--release"])
print(f"Time taken: {time.time() - start:.2f}s")
