//! Finite owner-controlled length/SHA-256 fixture, distinct from byte copy.
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
fn main() {
    let mut input = Vec::new();
    std::io::stdin()
        .take(65537)
        .read_to_end(&mut input)
        .unwrap();
    if input.len() > 65536 {
        std::process::exit(24);
    }
    if input == b"controlled-failure" {
        std::process::exit(23);
    }
    if input == b"dual-stream" {
        std::io::stdout().write_all(&[b'x'; 64]).unwrap();
        std::io::stderr().write_all(&[b'y'; 64]).unwrap();
        return;
    }
    println!("{}:{:x}", input.len(), Sha256::digest(&input));
}
