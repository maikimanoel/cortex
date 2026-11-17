//! Explicitly selected trusted components, not strict-domain qualification.
#![cfg(target_os = "linux")]
use cortex_core::program::supervision::{
    InterruptionReason, ProcessCompletion, ProcessLimits, TransportLimits,
};
use cortex_harness::native_execution::transport;
use sha2::{Digest, Sha256};
use std::{
    fs,
    process::{Command, Stdio},
    time::{Duration, Instant},
};
fn limits() -> ProcessLimits {
    ProcessLimits {
        wall_ms: 500,
        cleanup_ms: 300,
        input_bytes: 1024,
        output_bytes: 1024,
        frame_bytes: 64,
        authority_poll_ms: 2,
        executable_bytes: 64 * 1024 * 1024,
        arguments: 1,
        argument_bytes: 32,
        transport: Some(TransportLimits {
            startup_ms: 200,
            idle_ms: 200,
            stdout_bytes: 512,
            stderr_bytes: 512,
            records: 8,
            buffer_bytes: 16384,
        }),
    }
}
fn child(mode: &str) -> std::process::Child {
    let path = env!("CARGO_BIN_EXE_cortex-structured-fixture");
    let digest = Sha256::digest(fs::read(path).unwrap());
    println!("new_component_launch_attempt purpose={mode} binary_sha256={digest:x}");
    let child = Command::new(path)
        .arg(mode)
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    println!("new_component_started pid={} purpose={mode}", child.id());
    child
}
#[test]
#[ignore = "eight counted trusted component payloads; explicit source campaign opt-in only"]
fn structured_component_matrix() {
    // Fixed independent expected outcomes; all children are finite and reaped.
    for (mode, records, reason, code) in [
        ("valid", 2, None, Some(0)),
        ("split", 2, None, Some(0)),
        (
            "malformed",
            1,
            Some(InterruptionReason::MalformedOutput),
            None,
        ),
        ("nonzero", 1, None, Some(7)),
        (
            "empty",
            0,
            Some(InterruptionReason::StartupIncomplete),
            None,
        ),
        ("flood-out", 9, Some(InterruptionReason::EventLimit), None),
        ("flood-err", 9, Some(InterruptionReason::EventLimit), None),
        ("wait", 0, Some(InterruptionReason::Cancelled), None),
    ] {
        let start = Instant::now();
        let mut c = child(mode);
        let out = transport::collect_structured(
            &mut c,
            b"fixture-input",
            &limits(),
            start,
            start + Duration::from_millis(500),
            || {
                if mode == "wait" {
                    Err(InterruptionReason::Cancelled)
                } else {
                    Ok(())
                }
            },
        )
        .unwrap();
        assert_eq!(out.structured.records, records, "{mode}");
        if mode == "valid" {
            assert_eq!(out.streams.stdout, b"{\"n\":1}\n[true,false]\n");
            assert_eq!(out.streams.stderr, b"diagnostic");
        }
        if mode == "split" {
            assert_eq!(out.streams.stdout, "\"é☃\"\nnull".as_bytes());
        }

        assert_eq!(out.structured.interruption, reason, "{mode}");
        if let Some(reason) = reason {
            assert_eq!(
                out.streams.completion,
                ProcessCompletion::Interrupted { reason },
                "{mode}"
            );
        } else {
            assert_eq!(
                out.streams.completion,
                ProcessCompletion::Exited { code, signal: None },
                "{mode}"
            );
        }
        assert!(out.streams.stdout.len() + out.streams.stderr.len() <= 1024);
        assert!(c.try_wait().unwrap().is_some(), "{mode}");
        println!("new_component_reaped pid={} purpose={mode}", c.id());
    }
}
#[test]
#[ignore = "three counted trusted component payloads; explicit source campaign opt-in only"]
fn saturation_and_original_deadline_are_distinct() {
    for mode in ["flood-out", "flood-err", "valid"] {
        let start = Instant::now();
        let mut c = child(mode);
        let mut l = limits();
        l.transport.as_mut().unwrap().records = 1024;
        if mode == "flood-out" {
            l.transport.as_mut().unwrap().stdout_bytes = 32;
        }
        if mode == "flood-err" {
            l.transport.as_mut().unwrap().stderr_bytes = 16;
        }
        let out = if mode == "valid" {
            transport::collect_structured(
                &mut c,
                b"fixture-input",
                &l,
                start - Duration::from_millis(201),
                start + Duration::from_millis(300),
                || Ok(()),
            )
            .unwrap()
        } else {
            transport::collect_structured(
                &mut c,
                b"fixture-input",
                &l,
                start,
                start + Duration::from_millis(500),
                || Ok(()),
            )
            .unwrap()
        };
        let expected = if mode == "valid" {
            InterruptionReason::StartupTimeout
        } else if mode == "flood-out" {
            InterruptionReason::StdoutLimit
        } else {
            InterruptionReason::StderrLimit
        };
        assert_eq!(
            out.streams.completion,
            ProcessCompletion::Interrupted { reason: expected }
        );
        assert!(c.try_wait().unwrap().is_some());
        println!("new_component_reaped pid={} purpose={mode}", c.id());
    }
}
