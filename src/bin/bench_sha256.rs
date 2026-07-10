fn main() {
    let mut hasher = rust_interview_practice::cryptography::sha256::Sha256::new();
    let data = vec![b'a'; 10_000_000];
    hasher.update(&data);
    let hash = hasher.finalize();
    println!("{hash:?}");
}
