//! Audited, finite, provider-free fixture. No subprocesses or network operations.
use std::{fs::{self, OpenOptions}, io::{Read, Write}, os::unix::fs::OpenOptionsExt, path::Path};

fn write_new(path: &Path, bytes: &[u8]) {
    let mut file = OpenOptions::new().write(true).create_new(true).mode(0o600).open(path).unwrap();
    file.write_all(bytes).unwrap();
    file.sync_all().unwrap();
}
fn main() {
    assert!(std::env::vars_os().next().is_none());
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    assert_eq!(args.len(), 5);
    let cwd = fs::canonicalize(&args[1]).unwrap();
    assert_eq!(std::env::current_dir().unwrap(), cwd);
    let mut input = Vec::new();
    std::io::stdin().take(65_537).read_to_end(&mut input).unwrap();
    assert!(input.len() <= 65_536);
    assert!(input.starts_with(b"format 21\ncortex-instruction-v1\n"));
    // A second launch fails even if the controller accidentally changes its ID.
    write_new(Path::new(&args[4]), b"launched\n");
    let output = Path::new(&args[2]);
    match args[0].to_str().unwrap() {
        "produce" => {
            let mut bytes = b"cortex-artifact-v1\n".to_vec();
            for number in 2..10 {
                if (2..number).all(|factor| number % factor != 0) {
                    bytes.extend_from_slice(format!("{number}\n").as_bytes());
                }
            }
            write_new(output, &bytes);
        }
        "consume" => {
            let bytes = fs::read(&args[3]).unwrap();
            assert!(bytes.len() <= 256);
            let mut lines = std::str::from_utf8(&bytes).unwrap().lines();
            assert_eq!(lines.next(), Some("cortex-artifact-v1"));
            let values: Vec<u64> = lines.map(|line| line.parse().unwrap()).collect();
            let sum: u64 = values.iter().sum();
            let squares: u64 = values.iter().map(|value| value * value).sum();
            write_new(output, format!("cortex-consumer-v1\ncount={}\nsum={sum}\nsquares={squares}\n", values.len()).as_bytes());
            write_new(&output.with_extension("consumed"), &bytes);
        }
        _ => panic!("unsupported fixture mode"),
    }
    println!("{{\"fixture\":\"gpe-func-2t-v1\",\"input_bytes\":{}}}", input.len());
}
