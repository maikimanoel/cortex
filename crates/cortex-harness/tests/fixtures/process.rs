//! Owned finite process fixture, compiled by the supervisor mechanism tests.
use std::{
    io::{Read, Write},
    process::{Command, Stdio},
    thread,
    time::Duration,
};
fn main() {
    let mut args = std::env::args().skip(1);
    let mode = args.next().unwrap();
    if std::env::vars_os().next().is_some() {
        std::process::exit(40);
    }
    if mode == "retain" {
        thread::sleep(Duration::from_millis(250));
        return;
    }
    if mode == "descendant" {
        let child = Command::new(std::env::current_exe().unwrap())
            .arg("retain")
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        std::fs::write(args.next().unwrap(), child.id().to_string()).unwrap();
        return;
    }
    if mode == "count" {
        use std::fs::OpenOptions;
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(args.next().unwrap())
            .unwrap()
            .write_all(b"started\n")
            .unwrap();
    }
    if mode == "sleep" {
        thread::sleep(Duration::from_millis(800));
        return;
    }
    if mode == "backpressure" {
        std::io::stderr()
            .write_all(&vec![b'e'; 128 * 1024])
            .unwrap();
    }
    let mut input = Vec::new();
    std::io::stdin().read_to_end(&mut input).unwrap();
    match mode.as_str() {
        "malformed" => print!("private-malformed-payload\n"),
        "partial" => print!("{{\"incomplete\":"),
        "eof-record" => print!("{{\"complete\":true}}"),
        "flood" => {
            for _ in 0..2000 {
                if std::io::stdout()
                    .write_all(b"{\"fixture\":true}\n")
                    .is_err()
                {
                    break;
                }
            }
        }
        "stderr" => {
            let _ = std::io::stderr().write_all(&vec![b's'; 8192]);
        }
        _ => println!("{{\"input\":{},\"fixture\":true}}", input.len()),
    }
}
