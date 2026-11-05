//! Finite trusted development processes. No real manager, cgroup or namespace.
#![cfg(target_os = "linux")]
use serde_json::{Value, json};
use std::{
    fs,
    io::{Read, Write},
    os::unix::{
        fs::PermissionsExt,
        net::{UnixListener, UnixStream},
    },
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant},
};
fn read(stream: &mut UnixStream) -> Value {
    stream
        .set_read_timeout(Some(Duration::from_secs(1)))
        .unwrap();
    let mut length = [0; 4];
    stream.read_exact(&mut length).unwrap();
    let n = u32::from_be_bytes(length) as usize;
    assert!(n <= 4096);
    let mut bytes = vec![0; n];
    stream.read_exact(&mut bytes).unwrap();
    serde_json::from_slice(&bytes).unwrap()
}
fn write(stream: &mut UnixStream, value: Value) {
    let bytes = serde_json::to_vec(&value).unwrap();
    stream
        .write_all(&(bytes.len() as u32).to_be_bytes())
        .unwrap();
    stream.write_all(&bytes).unwrap()
}
#[test]
fn real_fixed_bootstrap_child_requires_release_and_handles_controller_loss() {
    static N: AtomicU64 = AtomicU64::new(0);
    for release in [true, false] {
        let root = std::env::temp_dir().join(format!(
            "cortex-bootstrap-child-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let endpoint = root.join("socket");
        let listener = UnixListener::bind(&endpoint).unwrap();
        listener.set_nonblocking(true).unwrap();
        let mut child = Command::new(env!("CARGO_BIN_EXE_cortex-domain-bootstrap"))
            .env_clear()
            .arg(&endpoint)
            .args(["request-1", "1000"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let end = Instant::now() + Duration::from_secs(2);
        let mut stream = loop {
            match listener.accept() {
                Ok((s, _)) => break s,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(Instant::now() < end);
                    assert!(child.try_wait().unwrap().is_none());
                    thread::yield_now()
                }
                Err(e) => panic!("{e}"),
            }
        };
        let hello = read(&mut stream);
        assert_eq!(hello[3], child.id());
        if release {
            write(
                &mut stream,
                json!([1, "release", "request-1", "public-nonce"]),
            );
            assert_eq!(
                read(&mut stream),
                json!([1, "nonce", "request-1", "public-nonce"])
            );
        }
        drop(stream);
        let status = loop {
            if let Some(s) = child.try_wait().unwrap() {
                break s;
            }
            if Instant::now() >= end {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("owned fixed child did not stop")
            }
            thread::yield_now()
        };
        assert_eq!(status.success(), release);
        fs::remove_dir_all(root).unwrap();
    }
}
