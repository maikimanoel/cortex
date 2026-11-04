#![cfg(target_os = "linux")]
//! GPE-B I1/O: real CLI output, key deadline and absence of recovery mutations.
#[path = "../../cortex-evidence/tests/support/program.rs"]
mod fixture;
use cortex_core::program::*;
use cortex_evidence::program::Command as Input;
use cortex_storage::{
    history_store::{Access, StoreKey},
    program_store::{ProgramStore, ReadLimits},
};
use fixture::*;
use std::{
    fs,
    io::Write,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::PathBuf,
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        loop {
            let p = std::env::temp_dir().join(format!(
                "cortex-program-cli-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&p) {
                Ok(()) => return Self(p),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(e) => panic!("fixture: {e}"),
            }
        }
    }
    fn store(&self) -> PathBuf {
        self.0.join("store")
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn key() -> StoreKey {
    StoreKey::new(&[0x57; 32]).unwrap()
}
fn limits() -> ReadLimits {
    ReadLimits::new(16_000_000, Duration::from_secs(20)).unwrap()
}
fn create(f: &Temp) -> ProgramStore {
    let mut k = initialized();
    release(&mut k, "AB");
    admit(
        &mut k,
        "claim",
        Input::Claim(attempt("A-active", "A", "operator")),
    );
    admit(
        &mut k,
        "unknown",
        Input::Attempt {
            id: AttemptId::new("A-active").unwrap(),
            generation: rev(1),
            status: AttemptStatus::Unknown,
            basis: source("interrupted-metadata"),
        },
    );
    let s = ProgramStore::create(&f.store(), &key(), k.initial().clone(), limits()).unwrap();
    for o in k.operations() {
        s.admit(&key(), o.clone()).unwrap();
    }
    s
}
fn command(f: &Temp) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_cortex"));
    c.args(["program", "inspect", "--store"])
        .arg(f.store())
        .args([
            "--program",
            "fixture-program",
            "--key-fd",
            "0",
            "--max-bytes",
            "16000000",
            "--read-timeout-ms",
            "10000",
        ]);
    c
}
fn run(mut command: Command, bytes: &[u8]) -> Output {
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(bytes).unwrap();
    let start = Instant::now();
    while child.try_wait().unwrap().is_none() {
        if start.elapsed() > Duration::from_secs(20) {
            child.kill().unwrap();
            panic!("CLI deadline");
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    child.wait_with_output().unwrap()
}
fn snapshot(f: &Temp) -> Vec<(PathBuf, Vec<u8>, u64, i64, i64)> {
    let mut v: Vec<_> = fs::read_dir(f.store())
        .unwrap()
        .map(|e| {
            let p = e.unwrap().path();
            let m = fs::metadata(&p).unwrap();
            (
                p.clone(),
                fs::read(p).unwrap(),
                m.ino(),
                m.mtime(),
                m.mtime_nsec(),
            )
        })
        .collect();
    v.sort_by(|a, b| a.0.cmp(&b.0));
    v
}
#[test]
fn human_and_json_inspection_expose_causes_without_modifying_store() {
    let f = Temp::new();
    let s = create(&f);
    let before = snapshot(&f);
    let mut c = command(&f);
    c.arg("--json");
    let output = run(c, &[0x57; 32]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(v["schema"], "cortex.program.v1");
    assert_eq!(v["objective"], "Coordinate the connected fixture program");
    assert_eq!(v["execution_authority"], "UNAVAILABLE");
    assert_eq!(v["live_candidate_verification"], "NOT_RUN");
    assert_eq!(v["attempts"][0]["status"], "UNKNOWN");
    let missions = v["missions"].as_array().unwrap();
    let b = missions.iter().find(|m| m["id"] == "B").unwrap();
    let f_view = missions.iter().find(|m| m["id"] == "F").unwrap();
    assert_eq!(b["eligible"], true);
    assert_eq!(b["dispatched"], false);
    assert_eq!(f_view["dependency_ready"], false);
    assert!(
        f_view["blockers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|b| b["kind"] == "DEPENDENCY")
    );
    let human = run(command(&f), &[0x57; 32]);
    assert!(human.status.success());
    let text = String::from_utf8(human.stdout).unwrap();
    assert!(text.contains("UNKNOWN"));
    assert!(text.contains("dependency_ready=false"));
    assert_eq!(snapshot(&f), before);
    assert_eq!(
        s.load(&key(), Access::ReadOnly).unwrap().state().attempts()[0].status,
        AttemptStatus::Unknown
    );
}
#[test]
fn wrong_key_program_and_short_or_long_key_are_safe_refusals() {
    let f = Temp::new();
    create(&f);
    let before = snapshot(&f);
    for bytes in [vec![0x66; 32], vec![0x57; 31], vec![0x57; 33]] {
        let mut c = command(&f);
        c.arg("--json");
        let o = run(c, &bytes);
        assert_eq!(o.status.code(), Some(3));
        let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
        assert_eq!(v["status"], "REFUSED");
        assert!(!String::from_utf8_lossy(&o.stdout).contains(f.0.to_str().unwrap()));
    }
    let mut c = Command::new(env!("CARGO_BIN_EXE_cortex"));
    c.args(["program", "inspect", "--store"])
        .arg(f.store())
        .args([
            "--program",
            "wrong-program",
            "--key-fd",
            "0",
            "--max-bytes",
            "16000000",
            "--read-timeout-ms",
            "10000",
            "--json",
        ]);
    let o = run(c, &[0x57; 32]);
    assert_eq!(o.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&o.stdout).contains("PROGRAM_MISMATCH"));
    assert_eq!(snapshot(&f), before);
}
#[test]
fn journal_recovery_required_is_read_only_and_explicit() {
    let f = Temp::new();
    create(&f);
    let journal = f.store().join("program.db-journal");
    fs::write(&journal, b"synthetic-pending-journal").unwrap();
    fs::set_permissions(&journal, fs::Permissions::from_mode(0o600)).unwrap();
    let before = snapshot(&f);
    let mut c = command(&f);
    c.arg("--json");
    let output = run(c, &[0x57; 32]);
    assert_eq!(output.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&output.stdout).contains("RecoveryRequired"));
    assert_eq!(snapshot(&f), before);
}
#[test]
fn blocked_key_descriptor_has_finite_deadline_without_reading_environment() {
    let f = Temp::new();
    create(&f);
    let mut c = Command::new(env!("CARGO_BIN_EXE_cortex"));
    c.args(["program", "inspect", "--store"])
        .arg(f.store())
        .args([
            "--program",
            "fixture-program",
            "--key-fd",
            "0",
            "--max-bytes",
            "16000000",
            "--read-timeout-ms",
            "50",
            "--json",
        ]);
    let mut child = c
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let input = child.stdin.take().unwrap();
    let start = Instant::now();
    while child.try_wait().unwrap().is_none() {
        if start.elapsed() > Duration::from_secs(5) {
            child.kill().unwrap();
            panic!("key timeout was not enforced");
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    drop(input);
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&output.stdout).contains("KEY_DEADLINE"));
}
#[test]
fn missing_required_limits_duplicates_and_effectful_commands_are_usage_errors() {
    for args in [
        vec!["program", "inspect"],
        vec!["program", "run"],
        vec!["program", "inspect", "--key", "secret"],
        vec!["program", "inspect", "--json", "--json"],
    ] {
        let o = Command::new(env!("CARGO_BIN_EXE_cortex"))
            .args(args)
            .output()
            .unwrap();
        assert_eq!(o.status.code(), Some(2));
        assert!(!String::from_utf8_lossy(&o.stdout).contains("secret"));
    }
    let o = Command::new(env!("CARGO_BIN_EXE_cortex"))
        .args(["program", "--help"])
        .output()
        .unwrap();
    assert!(o.status.success());
    assert!(String::from_utf8_lossy(&o.stdout).contains("read-timeout-ms"));
}
