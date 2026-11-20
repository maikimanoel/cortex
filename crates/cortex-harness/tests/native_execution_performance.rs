//! P1 component-only observation harness. No canonical GPE throughput,
//! strict-domain isolation or software-development productivity is measured.
#![cfg(target_os = "linux")]
use cortex_core::program::supervision::{ProcessCompletion, ProcessLimits, TransportLimits};
use cortex_harness::native_execution::{FileRole, FileSpec, PinnedFile, transport};
use sha2::{Digest, Sha256};
use std::{
    fs,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
fn limits() -> ProcessLimits {
    ProcessLimits {
        wall_ms: 2000,
        cleanup_ms: 1000,
        input_bytes: 4096,
        output_bytes: 512,
        frame_bytes: 512,
        authority_poll_ms: 2,
        executable_bytes: 64 * 1024 * 1024,
        arguments: 0,
        argument_bytes: 0,
        transport: Some(TransportLimits {
            startup_ms: 1000,
            idle_ms: 1000,
            stdout_bytes: 512,
            stderr_bytes: 0,
            records: 1,
            buffer_bytes: 16384,
        }),
    }
}
fn launch(path: &str) -> Child {
    let bytes = fs::read(path).unwrap();
    let pin = PinnedFile::inspect(&FileSpec {
        source: path.into(),
        destination: "/bin/fixture".into(),
        sha256: Sha256::digest(&bytes).into(),
        max_bytes: 64 * 1024 * 1024,
        role: FileRole::Executable,
    })
    .unwrap();
    pin.verify().unwrap();
    let c = Command::new(path)
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    println!(
        "component_launch pid={} binary_sha256={:x}",
        c.id(),
        Sha256::digest(&bytes)
    );
    c
}
fn collect(c: &mut Child, input: &[u8]) -> Vec<u8> {
    let out = transport::collect(
        c,
        input,
        &limits(),
        Instant::now() + Duration::from_secs(2),
        || Ok(()),
    )
    .unwrap();
    assert_eq!(
        out.completion,
        ProcessCompletion::Exited {
            code: Some(0),
            signal: None
        }
    );
    assert!(out.input_complete && out.stdout_complete && out.stderr_complete);
    println!(
        "component_reaped pid={} pump_ns={}",
        c.id(),
        out.elapsed.as_nanos()
    );
    out.stdout
}
fn arm(overlapped: bool) -> u128 {
    let start = Instant::now();
    let a_input = b"independent-a";
    let b_input = b"independent-b";
    let mut a = launch(env!("CARGO_BIN_EXE_cortex-native-copy-fixture"));
    let (a_out, b_out) = if overlapped {
        let mut b = launch(env!("CARGO_BIN_EXE_cortex-native-summary-fixture"));
        std::thread::scope(|scope| {
            let worker = scope.spawn(|| collect(&mut a, a_input));
            let b = collect(&mut b, b_input);
            (worker.join().unwrap(), b)
        })
    } else {
        let a = collect(&mut a, a_input);
        let mut b = launch(env!("CARGO_BIN_EXE_cortex-native-summary-fixture"));
        (a, collect(&mut b, b_input))
    };
    assert_eq!(a_out, a_input);
    assert_eq!(
        b_out,
        format!(
            "{}:{:x}
",
            b_input.len(),
            Sha256::digest(b_input)
        )
        .as_bytes()
    );
    let mut dependent_input = a_out;
    dependent_input.extend_from_slice(&b_out);
    let mut dependent = launch(env!("CARGO_BIN_EXE_cortex-native-summary-fixture"));
    let result = collect(&mut dependent, &dependent_input);
    assert_eq!(
        result,
        format!(
            "{}:{:x}
",
            dependent_input.len(),
            Sha256::digest(&dependent_input)
        )
        .as_bytes()
    );
    let elapsed = start.elapsed().as_nanos();
    println!(
        "{}",
        serde_json::json!({"class":"P1 component","overlapped":overlapped,"elapsed_ns":elapsed,
        "offered":3,"started":3,"valid_completed":3,"failed":0,"unfinished":0,"canonical_admissions":"NOT_RUN",
        "strict_domain_qualification":"BLOCKED","cpu_time":null,"memory_peak":null})
    );
    elapsed
}
#[test]
#[ignore = "36 counted trusted component launches; predeclare budget and run explicitly"]
fn paired_component_transport_observations() {
    // Warmups are separate and retained, not silently included in measured data.
    println!("warmup");
    arm(false);
    arm(true);
    for pair in 0..5 {
        println!("measured_pair={pair}");
        let (sequential, overlapped) = if pair % 2 == 0 {
            (arm(false), arm(true))
        } else {
            let overlap = arm(true);
            (arm(false), overlap)
        };
        println!(
            "{}",
            serde_json::json!({"pair":pair,"sequential_ns":sequential,"overlapped_ns":overlapped,
            "valid_per_arm":3,"scope":"P1 trusted component only"})
        );
    }
}
