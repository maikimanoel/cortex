//! Trusted component qualification only: these tests do not claim a strict
//! executable domain, delegated controls, isolation or canonical GPE completion.
#![cfg(target_os = "linux")]
use cortex_core::program::supervision::{
    InterruptionReason, ProcessCompletion, ProcessLimits, TransportLimits,
};
use cortex_harness::native_execution::{
    ExecutionError, FileRole, FileSpec, PinnedFile,
    artifacts::{self, ArtifactSpec},
    transport,
};
use sha2::{Digest, Sha256};
use std::{
    fs,
    os::unix::{
        fs::{PermissionsExt, symlink},
        process::ExitStatusExt,
    },
    path::PathBuf,
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
fn directory() -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "cxd-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&p).unwrap();
    fs::set_permissions(&p, fs::Permissions::from_mode(0o700)).unwrap();
    p
}
fn pin(path: PathBuf, bytes: &[u8]) -> FileSpec {
    FileSpec {
        source: path,
        destination: "/bin/fixture".into(),
        sha256: Sha256::digest(bytes).into(),
        max_bytes: 65536,
        role: FileRole::Executable,
    }
}
#[test]
fn approved_bytes_are_sealed_and_source_replacement_cannot_rewrite_them() {
    let root = directory();
    let path = root.join("approved");
    fs::write(&path, b"approved").unwrap();
    let spec = pin(path.clone(), b"approved");
    let original = PinnedFile::inspect(&spec).unwrap();
    assert_eq!(original.len(), 8);
    fs::write(&path, b"replaced").unwrap();
    assert!(matches!(
        PinnedFile::inspect(&spec),
        Err(ExecutionError::Identity)
    ));
    assert_eq!(original.len(), 8);
    assert_eq!(original.verify(), Ok(()));
    let link = root.join("link");
    symlink(&path, &link).unwrap();
    assert!(PinnedFile::inspect(&pin(link, b"replaced")).is_err());
}
#[test]
fn artifacts_use_declared_paths_real_bytes_and_independent_bounds() {
    let root = directory();
    fs::write(root.join("result"), b"abc").unwrap();
    let fd = fs::File::open(&root).unwrap();
    let spec = ArtifactSpec {
        path: "result".into(),
        max_bytes: 3,
        sha256: Some([
            0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae,
            0x22, 0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61,
            0xf2, 0x00, 0x15, 0xad,
        ]),
    };
    let got = artifacts::capture(&fd, std::slice::from_ref(&spec), 3).unwrap();
    assert_eq!(got[0].bytes, b"abc");
    assert_eq!(got[0].sha256, spec.sha256.unwrap());
    for name in ["../result", "/result", "link"] {
        if name == "link" {
            symlink(root.join("result"), root.join("link")).unwrap();
        }
        let mut bad = spec.clone();
        bad.path = name.into();
        assert!(artifacts::capture(&fd, &[bad], 3).is_err());
    }
    let mut bad = spec.clone();
    bad.max_bytes = 2;
    assert!(artifacts::capture(&fd, &[bad], 3).is_err());
    let mut bad = spec.clone();
    bad.sha256 = Some([0; 32]);
    assert!(artifacts::capture(&fd, &[bad], 3).is_err());
    fs::hard_link(root.join("result"), root.join("alias")).unwrap();
    assert!(artifacts::capture(&fd, &[spec], 3).is_err());
}
fn limits(output: u64) -> ProcessLimits {
    ProcessLimits {
        wall_ms: 2000,
        cleanup_ms: 1000,
        input_bytes: 65536,
        output_bytes: output,
        frame_bytes: output,
        authority_poll_ms: 2,
        executable_bytes: 64 * 1024 * 1024,
        arguments: 0,
        argument_bytes: 0,
        transport: Some(TransportLimits {
            startup_ms: 1000,
            idle_ms: 1000,
            stdout_bytes: output,
            stderr_bytes: 0,
            records: 1,
            buffer_bytes: 131072,
        }),
    }
}
fn child(path: &str) -> std::process::Child {
    // Hash the exact prebuilt owner-controlled fixture before its component-only
    // launch. This is not canonical permission or a strict isolation path.
    let bytes = fs::read(path).unwrap();
    let mut spec = pin(PathBuf::from(path), &bytes);
    spec.max_bytes = 64 * 1024 * 1024;
    let _pin = PinnedFile::inspect(&spec).unwrap();
    println!(
        "component_launch_attempt binary_sha256={:x}",
        Sha256::digest(&bytes)
    );
    let child = Command::new(path)
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    println!("component_started pid={}", child.id());
    child
}
#[test]
#[ignore = "counted trusted component launches; execute explicitly, not strict-domain qualification"]
fn two_distinct_pinned_executables_have_independent_exact_oracles() {
    let mut a = child(env!("CARGO_BIN_EXE_cortex-native-copy-fixture"));
    let out = transport::collect(
        &mut a,
        b"abc",
        &limits(64),
        Instant::now() + Duration::from_secs(2),
        || Ok(()),
    )
    .unwrap();
    assert_eq!(out.stdout, b"abc");
    assert!(out.input_complete && out.stdout_complete && out.stderr_complete);
    assert_eq!(
        out.completion,
        ProcessCompletion::Exited {
            code: Some(0),
            signal: None
        }
    );
    let mut b = child(env!("CARGO_BIN_EXE_cortex-native-summary-fixture"));
    let out = transport::collect(
        &mut b,
        b"abc",
        &limits(128),
        Instant::now() + Duration::from_secs(2),
        || Ok(()),
    )
    .unwrap();
    assert_eq!(
        out.stdout,
        b"3:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad\n"
    );
    assert_eq!(
        out.completion,
        ProcessCompletion::Exited {
            code: Some(0),
            signal: None
        }
    );
}
#[test]
#[ignore = "counted trusted component launch; execute explicitly"]
fn overflow_is_bounded_partial_failure_instead_of_success() {
    let mut c = child(env!("CARGO_BIN_EXE_cortex-native-copy-fixture"));
    let out = transport::collect(
        &mut c,
        &vec![b'x'; 8192],
        &limits(32),
        Instant::now() + Duration::from_secs(2),
        || Ok(()),
    )
    .unwrap();
    assert_eq!(out.stdout.len(), 32);
    assert!(!out.stdout_complete);
    assert_eq!(
        out.completion,
        ProcessCompletion::Interrupted {
            reason: InterruptionReason::StdoutLimit
        }
    );
    assert!(c.try_wait().unwrap().is_some());
}
#[test]
#[ignore = "counted trusted component launches; execute explicitly"]
fn nonzero_exit_and_cancellation_remain_distinct_observations() {
    let mut c = child(env!("CARGO_BIN_EXE_cortex-native-summary-fixture"));
    let out = transport::collect(
        &mut c,
        b"controlled-failure",
        &limits(128),
        Instant::now() + Duration::from_secs(2),
        || Ok(()),
    )
    .unwrap();
    assert_eq!(
        out.completion,
        ProcessCompletion::Exited {
            code: Some(23),
            signal: None
        }
    );
    let mut c = child(env!("CARGO_BIN_EXE_cortex-native-copy-fixture"));
    let out = transport::collect(
        &mut c,
        b"abc",
        &limits(128),
        Instant::now() + Duration::from_secs(2),
        || Err(InterruptionReason::Cancelled),
    )
    .unwrap();
    assert_eq!(
        out.completion,
        ProcessCompletion::Interrupted {
            reason: InterruptionReason::Cancelled
        }
    );
    assert!(c.try_wait().unwrap().unwrap().signal().is_some());
}

#[test]
#[ignore = "counted trusted component launches; execute explicitly"]
fn aggregate_bound_and_unaffected_neighbor_are_independent_oracles() {
    let mut c = child(env!("CARGO_BIN_EXE_cortex-native-summary-fixture"));
    let mut l = limits(64);
    l.transport.as_mut().unwrap().stderr_bytes = 64;
    let out = transport::collect(
        &mut c,
        b"dual-stream",
        &l,
        Instant::now() + Duration::from_secs(2),
        || Ok(()),
    )
    .unwrap();
    assert!(out.stdout.len() + out.stderr.len() <= 64);
    assert!(matches!(
        out.completion,
        ProcessCompletion::Interrupted {
            reason: InterruptionReason::OutputLimit
                | InterruptionReason::StdoutLimit
                | InterruptionReason::StderrLimit
        }
    ));
    let mut neighbor = child(env!("CARGO_BIN_EXE_cortex-native-summary-fixture"));
    let mut cancelled = child(env!("CARGO_BIN_EXE_cortex-native-copy-fixture"));
    let result = transport::collect(
        &mut cancelled,
        b"abc",
        &limits(128),
        Instant::now() + Duration::from_secs(2),
        || Err(InterruptionReason::Cancelled),
    )
    .unwrap();
    assert_eq!(
        result.completion,
        ProcessCompletion::Interrupted {
            reason: InterruptionReason::Cancelled
        }
    );
    let valid = transport::collect(
        &mut neighbor,
        b"abc",
        &limits(128),
        Instant::now() + Duration::from_secs(2),
        || Ok(()),
    )
    .unwrap();
    assert_eq!(
        valid.stdout,
        b"3:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad
"
    );
    assert_eq!(
        valid.completion,
        ProcessCompletion::Exited {
            code: Some(0),
            signal: None
        }
    );
}
