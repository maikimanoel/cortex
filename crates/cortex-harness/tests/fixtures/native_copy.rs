//! Finite owner-controlled byte-copy fixture. No filesystem or network access.
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
    std::io::stdout().write_all(&input).unwrap();
}
