//! Finite trusted transport fixture. No host isolation or provider behavior.
use std::io::{self, Read, Write};
fn main() -> io::Result<()> {
    let mode = std::env::args().nth(1).unwrap_or_default();
    let mut input = Vec::new();
    io::stdin().take(1025).read_to_end(&mut input)?;
    if input != b"fixture-input" {
        std::process::exit(3);
    }
    match mode.as_str() {
        "valid" => {
            io::stdout().write_all(b"{\"n\":1}\n[true,false]\n")?;
            io::stderr().write_all(b"diagnostic")?;
        }
        "split" => {
            for byte in "\"é☃\"\nnull".as_bytes() {
                io::stdout().write_all(&[*byte])?;
                io::stdout().flush()?;
            }
        }
        "malformed" => io::stdout().write_all(b"{\"n\":1}\n{")?,
        "nonzero" => {
            io::stdout().write_all(b"{}\n")?;
            std::process::exit(7);
        }
        "flood-out" => {
            for _ in 0..256 {
                io::stdout().write_all(b"0\n")?;
                io::stderr().write_all(b"e")?;
            }
        }
        "flood-err" => {
            for _ in 0..256 {
                io::stderr().write_all(b"e")?;
                io::stdout().write_all(b"0\n")?;
            }
        }
        "empty" => {}
        "wait" => std::thread::sleep(std::time::Duration::from_millis(100)),
        _ => std::process::exit(4),
    }
    Ok(())
}
