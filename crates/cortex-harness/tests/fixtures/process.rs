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
    if mode == "controlled-retain" {
        // This child owns the readiness channel and inherited stream writers.
        // Disconnect/timeout is a finite fallback if the test controller fails.
        let mut channel = std::os::unix::net::UnixStream::connect(args.next().unwrap()).unwrap();
        channel.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        channel.set_write_timeout(Some(Duration::from_secs(2))).unwrap();
        let stat = std::fs::read_to_string("/proc/self/stat").unwrap();
        let fields: Vec<_> = stat.rsplit_once(')').unwrap().1.split_whitespace().collect();
        let parent: u64 = fields[1].parse().unwrap();
        let ticks: u64 = fields[19].parse().unwrap();
        for value in [u64::from(std::process::id()), parent, ticks] {
            channel.write_all(&value.to_be_bytes()).unwrap();
        }
        let mut release = [0];
        if channel.read_exact(&mut release).is_ok() && release == [1] {
            channel.write_all(&[2]).unwrap();
        }
        return;
    }
    if mode == "controlled-descendant" {
        let child = Command::new(std::env::current_exe().unwrap())
            .args(["controlled-retain", &args.next().unwrap()])
            .env_clear().stdin(Stdio::null()).stdout(Stdio::inherit()).stderr(Stdio::inherit())
            .spawn().unwrap();
        // The stream-retention test has an explicit empty package. Nonempty
        // input/EOF-before-spawn remains the separate legacy descendant case.
        let mut input = Vec::new();
        std::io::stdin().read_to_end(&mut input).unwrap();
        assert!(input.is_empty());
        drop(child);
        return;
    }
    if mode == "retain" {
        thread::sleep(Duration::from_millis(250));
        return;
    }
    if mode == "prefill-output" {
        // A finite owned fixture lets the guard regression observe queued bytes
        // before supervision begins, without pressure or scheduler assumptions.
        println!("{{}}");
        std::io::stdout().flush().unwrap();
        std::fs::write(args.next().unwrap(), b"queued").unwrap();
        thread::sleep(Duration::from_millis(250));
        return;
    }
    let descendant_marker = if mode == "descendant" {
        let marker = args.next().unwrap();
        if let Some(ready) = args.next() {
            std::fs::write(ready, b"awaiting-input-eof").unwrap();
        }
        Some(marker)
    } else {
        None
    };
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
    if mode == "revocation-ready" {
        // Input is sent only after the supervisor records Spawned. Report this
        // owned incarnation without contending with its startup journal writes.
        let mut channel = std::os::unix::net::UnixStream::connect(args.next().unwrap()).unwrap();
        channel
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        channel
            .set_write_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let stat = std::fs::read_to_string("/proc/self/stat").unwrap();
        let fields: Vec<_> = stat
            .rsplit_once(')')
            .unwrap()
            .1
            .split_whitespace()
            .collect();
        let ticks: u64 = fields[19].parse().unwrap();
        for value in [u64::from(std::process::id()), ticks] {
            channel.write_all(&value.to_be_bytes()).unwrap();
        }
        // Authority loss should terminate this process. A lost test controller
        // or timeout is a finite fallback, never evidence of revocation.
        let _ = channel.read(&mut [0]);
        return;
    }
    if let Some(marker) = descendant_marker {
        // The stream-retention scenario begins only after the entire explicit
        // package and EOF have arrived. Early parent exit would instead exercise
        // the supervisor's truthful InputClosed path.
        assert_eq!(input, b"input");
        let child = Command::new(std::env::current_exe().unwrap())
            .arg("retain")
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        std::fs::write(marker, child.id().to_string()).unwrap();
        return;
    }
    match mode.as_str() {
        "no-record" => {}
        "one-record" => println!("{{}}"),
        "two-records" => print!("{{}}\n{{}}\n"),
        "two-eof-records" => print!("{{}}\n{{}}"),
        "idle-after-record" => {
            println!("{{}}");
            std::io::stdout().flush().unwrap();
            thread::sleep(Duration::from_millis(250));
        }
        "paced-records" => {
            for _ in 0..4 {
                println!("{{}}");
                std::io::stdout().flush().unwrap();
                thread::sleep(Duration::from_millis(20));
            }
        }
        "delayed-record" => {
            thread::sleep(Duration::from_millis(250));
            println!("{{}}");
        }
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
