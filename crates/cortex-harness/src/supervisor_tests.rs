//! Tests use only this repository's finite, provider-free development executable.
use super::*;
use std::{
    os::unix::fs::PermissionsExt,
    sync::{Arc, OnceLock, atomic::AtomicU64},
};
fn source(name: &str) -> SourceReference {
    SourceReference::operator_reported("fixture-test", name, "1").unwrap()
}
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        static N: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "cortex-harness-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn binary() -> &'static [u8] {
    static BINARY: OnceLock<Vec<u8>> = OnceLock::new();
    BINARY.get_or_init(|| {
        let root = Temp::new();
        let target = root.0.join("compiled-fixture");
        let output = Command::new("rustc")
            .args(["--edition=2024", "-C", "debuginfo=0", "-o"])
            .arg(&target)
            .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/process.rs"))
            .output()
            .unwrap();
        assert!(output.status.success(), "owned fixture compilation failed");
        let bytes = fs::read(target).unwrap();
        assert!(bytes.len() <= 16_000_000);
        bytes
    })
}
fn install_binary(path: &Path) {
    fs::write(path, binary()).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
}

fn pin(root: &Temp, mode: &str, input: Vec<u8>) -> PinnedFixture {
    let exe = root.0.join("fixture");
    install_binary(&exe);
    let spec = ProcessSpec {
        reference: source("launch"),
        executable: exe,
        executable_identity: source("exact-test-executable"),
        cwd: root.0.clone(),
        args: vec![mode.into()],
        prepared_input: OperationId::new("prepared").unwrap(),
        input_len: input.len() as u64,
        host: source("host"),
        profile: source("profile"),
        privacy: source("privacy"),
        approval: source("explicit-owned-fixture"),
        limits: ProcessLimits {
            wall_ms: 1000,
            cleanup_ms: 100,
            input_bytes: 2_000_000,
            output_bytes: 2_000_000,
            frame_bytes: 1024,
            authority_poll_ms: 10,
            executable_bytes: 16_000_000,
            arguments: 2,
            argument_bytes: 4096,
        },
    };
    let executable =
        PinnedExecutable::inspect(&spec.executable, spec.limits.executable_bytes).unwrap();
    let cwd = stamp(&fs::metadata(&spec.cwd).unwrap());
    PinnedFixture {
        spec,
        descriptor: support::descriptor(),
        requirements: support::requirements(),
        input,
        executable,
        cwd,
    }
}
fn ids() -> CaptureIds {
    CaptureIds {
        started_operation: OperationId::new("started").unwrap(),
        finished_operation: OperationId::new("finished").unwrap(),
        started_observation: ObservationId::new("harness", 1).unwrap(),
        finished_observation: ObservationId::new("harness", 2).unwrap(),
        basis: source("native-fixture-observation"),
        admission_attempts: 2,
    }
}
fn run(
    pin: &PinnedFixture,
    authority: impl FnMut(u64) -> Result<(), InterruptionReason>,
) -> ProcessObservation {
    let running = spawn(pin, Instant::now()).unwrap();
    supervise(
        running,
        pin,
        &ids(),
        &AtomicBool::new(false),
        authority,
        None,
    )
}
#[test]
fn exact_input_jsonl_environment_clear_and_backpressure_are_observed() {
    let root = Temp::new();
    let pin = pin(&root, "backpressure", vec![b'i'; 512 * 1024]);
    let result = run(&pin, |_| Ok(()));
    assert_eq!(
        result.completion,
        ProcessCompletion::Exited {
            code: Some(0),
            signal: None
        }
    );
    assert_eq!(
        result.quiescence,
        Quiescence::DirectChildReapedStreamsClosed
    );
    assert_eq!(result.input_bytes, 512 * 1024);
    assert_eq!(result.stderr_bytes, 128 * 1024);
    assert_eq!(result.structured_records, 1);
    assert!(!result.malformed_output);
    let process = result.process.unwrap();
    assert!(process.start_ticks > 0);
    assert!(!Path::new(&format!("/proc/{}", process.pid)).exists());
}
#[test]
fn timeout_cancellation_and_canonical_authority_loss_stop_owned_direct_child() {
    for expected in [
        InterruptionReason::Timeout,
        InterruptionReason::AuthorityLost,
        InterruptionReason::AuthorityUnknown,
    ] {
        let root = Temp::new();
        let mut pin = pin(&root, "sleep", b"input".to_vec());
        pin.spec.limits.wall_ms = 40;
        let result = run(&pin, |ms| {
            if expected != InterruptionReason::Timeout && ms >= 10 {
                Err(expected)
            } else {
                Ok(())
            }
        });
        assert_eq!(
            result.completion,
            ProcessCompletion::Interrupted { reason: expected }
        );
        assert_eq!(
            result.quiescence,
            Quiescence::DirectChildReapedStreamsClosed
        );
        assert!(result.elapsed_ms < 1000);
    }
    let root = Temp::new();
    let pin = pin(&root, "sleep", b"input".to_vec());
    let cancellation = Arc::new(AtomicBool::new(false));
    let flag = cancellation.clone();
    let sender = thread::spawn(move || {
        thread::sleep(Duration::from_millis(20));
        flag.store(true, Ordering::Release);
    });
    let result = supervise(
        spawn(&pin, Instant::now()).unwrap(),
        &pin,
        &ids(),
        &cancellation,
        |_| Ok(()),
        None,
    );
    sender.join().unwrap();
    assert_eq!(
        result.completion,
        ProcessCompletion::Interrupted {
            reason: InterruptionReason::Cancelled
        }
    );
    assert_eq!(
        result.quiescence,
        Quiescence::DirectChildReapedStreamsClosed
    );
}
#[test]
fn output_limits_and_malformed_frames_are_not_success_or_fake_eof() {
    for mode in ["flood", "stderr", "malformed", "partial"] {
        let root = Temp::new();
        let mut pin = pin(&root, mode, b"input".to_vec());
        pin.spec.limits.output_bytes = 64;
        pin.spec.limits.frame_bytes = 64;
        let result = run(&pin, |_| Ok(()));
        let reason = if matches!(mode, "flood" | "stderr") {
            InterruptionReason::OutputLimit
        } else {
            InterruptionReason::MalformedOutput
        };
        assert_eq!(result.completion, ProcessCompletion::Interrupted { reason });
        assert_ne!(
            result.outcome(&pin.spec),
            Some(cortex_core::program::operational::EffectOutcome::Succeeded)
        );
        if mode != "partial" {
            assert_eq!(result.quiescence, Quiescence::Unknown);
        }
        assert!(result.stdout_bytes + result.stderr_bytes <= 65);
    }
}
#[test]
fn finite_descendant_retaining_streams_is_unknown_then_owned_fixture_finishes() {
    let root = Temp::new();
    let mut pin = pin(&root, "descendant", b"input".to_vec());
    let marker = root.0.join("child-pid");
    pin.spec.args.push(marker.to_string_lossy().into_owned());
    pin.spec.limits.wall_ms = 30;
    pin.spec.limits.cleanup_ms = 30;
    let result = run(&pin, |_| Ok(()));
    assert_eq!(result.input_bytes, pin.spec.input_len);
    assert_eq!(result.quiescence, Quiescence::Unknown);
    assert_eq!(
        result.completion,
        ProcessCompletion::Interrupted {
            reason: InterruptionReason::Timeout
        }
    );
    let pid = fs::read_to_string(marker).unwrap().parse::<u32>().unwrap();
    observe_finite_descendant_finished(pid);
}
fn observe_finite_descendant_finished(pid: u32) {
    thread::sleep(Duration::from_millis(350));
    // A briefly unreaped orphan zombie is no longer a running escaped fixture.
    if let Ok(stat) = fs::read_to_string(format!("/proc/{pid}/stat")) {
        assert_eq!(
            stat.rsplit_once(") ").unwrap().1.split_whitespace().next(),
            Some("Z")
        );
    }
}

#[test]
fn descendant_fixture_requires_complete_exact_input_and_eof_before_spawning() {
    let root = Temp::new();
    let mut pin = pin(&root, "descendant", b"input".to_vec());
    let marker = root.0.join("child-pid");
    let ready = root.0.join("awaiting-input");
    pin.spec.args.extend([
        marker.to_string_lossy().into_owned(),
        ready.to_string_lossy().into_owned(),
    ]);
    pin.spec.limits.arguments = 3;
    let mut running = spawn(&pin, Instant::now()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(1);
    while !ready.exists() {
        assert!(
            running.child.poll().unwrap().is_none(),
            "fixture exited before input readiness"
        );
        assert!(
            Instant::now() < deadline,
            "fixture never reached input readiness"
        );
        thread::sleep(POLL);
    }
    assert_eq!(fs::read(ready).unwrap(), b"awaiting-input-eof");
    running.stdin.write_all(b"in").unwrap();
    // Exercise a delayed, incomplete package: neither a partial read nor time
    // passing may let the fixture spawn its descendant before EOF.
    thread::sleep(Duration::from_millis(40));
    assert!(running.child.poll().unwrap().is_none());
    assert!(!marker.exists());
    running.stdin.write_all(b"put").unwrap();
    thread::sleep(Duration::from_millis(40));
    assert!(running.child.poll().unwrap().is_none());
    assert!(!marker.exists());
    running.stdin.shutdown(Shutdown::Write).unwrap();
    while running.child.poll().unwrap().is_none() {
        assert!(
            Instant::now() < deadline,
            "fixture did not finish after exact input and EOF"
        );
        thread::sleep(POLL);
    }
    assert!(running.child.reaped.unwrap().success());
    let pid = fs::read_to_string(marker).unwrap().parse::<u32>().unwrap();
    observe_finite_descendant_finished(pid);
}

#[test]
fn executable_replacement_symlinks_and_changed_bytes_refuse_pin_reuse() {
    let root = Temp::new();
    let pin = pin(&root, "normal", b"input".to_vec());
    pin.executable.check().unwrap();
    let old = root.0.join("old");
    fs::rename(&pin.spec.executable, &old).unwrap();
    fs::copy(&old, &pin.spec.executable).unwrap();
    assert!(matches!(
        pin.executable.check(),
        Err(HarnessError::TargetChanged)
    ));
    let link = root.0.join("link");
    std::os::unix::fs::symlink(&old, &link).unwrap();
    assert!(matches!(
        PinnedExecutable::inspect(&link, 16_000_000),
        Err(HarnessError::TargetChanged)
    ));
    let pin = PinnedExecutable::inspect(&pin.spec.executable, 16_000_000).unwrap();
    let mut bytes = fs::read(&pin.path).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 1;
    fs::write(&pin.path, &bytes).unwrap();
    assert!(matches!(pin.check(), Err(HarnessError::TargetChanged)));
}
#[test]
fn framing_enforces_exact_frame_boundary_without_retaining_diagnostics() {
    let mut framing = Framing {
        bytes: vec![],
        records: 0,
        malformed: false,
    };
    framing.feed(b"{}\n", 2).unwrap();
    assert_eq!(framing.records, 1);
    assert!(framing.bytes.is_empty());
    assert_eq!(
        framing.feed(b"123\n", 2),
        Err(InterruptionReason::MalformedOutput)
    );
    assert!(framing.malformed);
    let display = format!("{}", HarnessError::Unsupported(Control::Network));
    assert!(!display.contains("private-malformed-payload"));
}

#[path = "../../cortex-evidence/tests/support/program.rs"]
mod fixture;
#[path = "../../cortex-evidence/tests/support/supervision.rs"]
mod support;
use cortex_storage::program_store::ReadLimits;
struct DurableFixture {
    root: Temp,
    store: ProgramStore,
    key: StoreKey,
    pin: PinnedFixture,
    dispatch: Operation,
    scope: EffectScope,
}
fn durable(mode: &str) -> DurableFixture {
    let root = Temp::new();
    let repo = root.0.join("repo");
    fs::create_dir(&repo).unwrap();
    let exe = repo.join("fixture");
    install_binary(&exe);
    let (mut kernel, scope, mut spec) = support::configured_kernel(Some(&repo));
    spec.executable = exe;
    spec.args = vec![mode.into()];
    if mode == "count" {
        spec.args
            .push(root.0.join("launch-count").to_string_lossy().into_owned());
    }
    spec.limits.executable_bytes = 16_000_000;
    spec.limits.argument_bytes = 4096;
    let pin = inspect_fixture(
        &spec,
        &kernel.prepared_inputs()[0].1,
        kernel.state().definition().limits(),
    )
    .unwrap();
    support::prepare(&mut kernel, &scope, &spec);
    let key = StoreKey::new(&[73_u8; 32]).unwrap();
    let store = ProgramStore::create_supervised_fixture(
        &root.0.join("store"),
        &key,
        kernel.initial().clone(),
        ReadLimits::new(16_000_000, Duration::from_millis(200)).unwrap(),
    )
    .unwrap();
    for operation in kernel.operations() {
        store.admit(&key, operation.clone()).unwrap();
    }
    let dispatch = support::dispatch_operation(&kernel, &scope);
    DurableFixture {
        root,
        store,
        key,
        pin,
        dispatch,
        scope,
    }
}
#[test]
fn canonical_public_dispatch_captures_identity_and_replay_never_relaunches() {
    let f = durable("count");
    let result = execute_fixture(
        &f.store,
        &f.key,
        f.dispatch.clone(),
        ids(),
        &f.pin,
        &AtomicBool::new(false),
    )
    .unwrap();
    assert!(!result.dispatch.duplicate);
    assert!(result.started.is_some());
    assert!(result.recorded.is_some(), "{:?}", result.recording_failure);
    assert!(result.recording_failure.is_none());
    let observation = result.observation.unwrap();
    assert_eq!(
        observation.completion,
        ProcessCompletion::Exited {
            code: Some(0),
            signal: None
        }
    );
    assert_eq!(
        observation.quiescence,
        Quiescence::DirectChildReapedStreamsClosed
    );
    assert_eq!(
        fs::read(f.root.0.join("launch-count")).unwrap(),
        b"started\n"
    );
    let before = f.store.load(&f.key, Access::ReadOnly).unwrap();
    assert_eq!(
        before.fixture_effects().effects()[0]
            .process_observations
            .len(),
        2
    );
    assert!(before.fixture_effects().effects()[0].pending());
    assert!(before.records().is_empty());
    let duplicate = execute_fixture(
        &f.store,
        &f.key,
        f.dispatch,
        ids(),
        &f.pin,
        &AtomicBool::new(false),
    )
    .unwrap();
    assert!(duplicate.dispatch.duplicate);
    assert!(duplicate.observation.is_none());
    assert_eq!(
        fs::read(f.root.0.join("launch-count")).unwrap(),
        b"started\n"
    );
    let after = f.store.load(&f.key, Access::ReadOnly).unwrap();
    assert_eq!(before.revision(), after.revision());
}
#[test]
fn canonical_revocation_is_observed_without_an_atomic_cancellation_request() {
    let f = durable("sleep");
    thread::scope(|scope| {
        let revoker = scope.spawn(|| {
            let deadline = Instant::now() + Duration::from_secs(2);
            loop {
                let kernel = f.store.load(&f.key, Access::ReadOnly).unwrap();
                if kernel.fixture_effects().effects()[0]
                    .process_observations
                    .iter()
                    .any(|o| o.completion == ProcessCompletion::Spawned)
                {
                    let event = EffectEvent::Revoke {
                        grant: support::proposed_grant(&f.scope).id,
                        at: kernel.fixture_effects().last_tick() + 1,
                        basis: source("explicit-revocation"),
                    };
                    let op = support::operation(
                        &kernel,
                        "revoke-during-process",
                        WorkflowCommand::SupervisedEffect(Box::new(event)),
                    );
                    f.store.admit(&f.key, op).unwrap();
                    break;
                }
                assert!(
                    Instant::now() < deadline,
                    "no durable process identity observed"
                );
                thread::sleep(Duration::from_millis(2));
            }
        });
        let result = execute_fixture(
            &f.store,
            &f.key,
            f.dispatch.clone(),
            ids(),
            &f.pin,
            &AtomicBool::new(false),
        )
        .unwrap();
        revoker.join().unwrap();
        assert!(result.recorded.is_some(), "{:?}", result.recording_failure);
        let observation = result.observation.unwrap();
        assert_eq!(
            observation.completion,
            ProcessCompletion::Interrupted {
                reason: InterruptionReason::AuthorityLost
            }
        );
        assert_eq!(
            observation.quiescence,
            Quiescence::DirectChildReapedStreamsClosed
        );
    });
    let kernel = f.store.load(&f.key, Access::ReadOnly).unwrap();
    assert!(kernel.fixture_effects().effects()[0].pending());
    assert_eq!(kernel.fixture_effects().revoked().len(), 1);
}
#[test]
fn changed_pinned_target_refuses_before_durable_dispatch() {
    let f = durable("count");
    let old = f.pin.spec.executable.with_file_name("old-fixture");
    fs::rename(&f.pin.spec.executable, &old).unwrap();
    fs::copy(&old, &f.pin.spec.executable).unwrap();
    let before = f.store.load(&f.key, Access::ReadOnly).unwrap().revision();
    assert!(matches!(
        execute_fixture(
            &f.store,
            &f.key,
            f.dispatch,
            ids(),
            &f.pin,
            &AtomicBool::new(false)
        ),
        Err(HarnessError::TargetChanged)
    ));
    assert_eq!(
        before,
        f.store.load(&f.key, Access::ReadOnly).unwrap().revision()
    );
    assert!(!f.root.0.join("launch-count").exists());
}
#[test]
fn required_host_denials_cannot_be_enabled_by_fabricated_supported_descriptors() {
    use cortex_core::program::executor::*;
    let f = durable("normal");
    let kernel = f.store.load(&f.key, Access::ReadOnly).unwrap();
    let original = &kernel.prepared_inputs()[0].1;
    for control in [
        Control::Network,
        Control::Filesystem,
        Control::Descendants,
        Control::Revocation,
        Control::WorkspaceIdentity,
        Control::Resource(Resource::RamBytes),
        Control::Resource(Resource::CpuMillis),
    ] {
        let mut observations = original.descriptor().observations().to_vec();
        observations.push(CapabilityObservation {
            control,
            kind: ObservationKind::ControlledFixture,
            support: Support::Supported,
            basis: source("claim-alone-is-not-host-support"),
        });
        let descriptor = ExecutorDescriptor::new(
            source("new-descriptor"),
            original.descriptor().identity().clone(),
            Transport::StandardInputJsonLines,
            observations,
            3,
        )
        .unwrap();
        let mut required = original.requirements().required().to_vec();
        required.push(Requirement {
            control,
            kind: ObservationKind::ControlledFixture,
        });
        let requirements = Requirements::new(source("new-required"), required, 3).unwrap();
        let input = prepare(
            kernel.state(),
            original.package(),
            &descriptor,
            &requirements,
            1_000_000,
        )
        .unwrap();
        assert!(
            matches!(inspect_fixture(&f.pin.spec,&input,kernel.state().definition().limits()),Err(HarnessError::Unsupported(rejected)) if rejected==control)
        );
    }
}

#[test]
fn complete_final_json_frame_at_eof_is_a_legitimate_neighbor() {
    let root = Temp::new();
    let pin = pin(&root, "eof-record", b"input".to_vec());
    let result = run(&pin, |_| Ok(()));
    assert_eq!(
        result.completion,
        ProcessCompletion::Exited {
            code: Some(0),
            signal: None
        }
    );
    assert_eq!(result.structured_records, 1);
    assert!(!result.malformed_output);
    assert_eq!(
        result.quiescence,
        Quiescence::DirectChildReapedStreamsClosed
    );
}
#[test]
fn cooperative_authority_read_delay_consumes_the_existing_wall_budget() {
    let root = Temp::new();
    let mut pin = pin(&root, "normal", b"input".to_vec());
    pin.spec.limits.wall_ms = 20;
    let result = run(&pin, |_| {
        thread::sleep(Duration::from_millis(40));
        Ok(())
    });
    assert_eq!(
        result.completion,
        ProcessCompletion::Interrupted {
            reason: InterruptionReason::Timeout
        }
    );
    assert_ne!(
        result.outcome(&pin.spec),
        Some(cortex_core::program::operational::EffectOutcome::Succeeded)
    );
}

#[test]
fn a_pin_cannot_cross_to_different_required_controls_with_identical_package_bytes() {
    use cortex_core::program::executor::*;
    let f = durable("count");
    let original = f.store.load(&f.key, Access::ReadOnly).unwrap();
    let store = ProgramStore::create_supervised_fixture(
        &f.root.0.join("other-store"),
        &f.key,
        original.initial().clone(),
        ReadLimits::new(16_000_000, Duration::from_millis(200)).unwrap(),
    )
    .unwrap();
    for operation in original.operations() {
        let mut changed = operation.clone();
        if let WorkflowCommand::PrepareExecutorInput(ref mut request) = changed.command {
            let mut observations = request.descriptor.observations().to_vec();
            observations.push(CapabilityObservation {
                control: Control::Network,
                kind: ObservationKind::ControlledFixture,
                support: Support::Supported,
                basis: source("unsupported-host-claim"),
            });
            request.descriptor = ExecutorDescriptor::new(
                request.descriptor.reference().clone(),
                request.descriptor.identity().clone(),
                request.descriptor.transport(),
                observations,
                3,
            )
            .unwrap();
            let mut requirements = request.requirements.required().to_vec();
            requirements.push(Requirement {
                control: Control::Network,
                kind: ObservationKind::ControlledFixture,
            });
            request.requirements =
                Requirements::new(request.requirements.reference().clone(), requirements, 3)
                    .unwrap();
        }
        store.admit(&f.key, changed).unwrap();
    }
    let before = store.load(&f.key, Access::ReadOnly).unwrap().revision();
    assert!(matches!(
        execute_fixture(
            &store,
            &f.key,
            f.dispatch,
            ids(),
            &f.pin,
            &AtomicBool::new(false)
        ),
        Err(HarnessError::TargetChanged)
    ));
    assert_eq!(
        before,
        store.load(&f.key, Access::ReadOnly).unwrap().revision()
    );
    assert!(!f.root.0.join("launch-count").exists());
}

#[test]
fn a_deadline_consumed_before_the_post_commit_launch_boundary_starts_no_child() {
    let f = durable("count");
    f.store.admit(&f.key, f.dispatch.clone()).unwrap();
    let kernel = f.store.load(&f.key, Access::ReadOnly).unwrap();
    let old = Instant::now()
        .checked_sub(Duration::from_millis(f.pin.spec.limits.wall_ms))
        .unwrap();
    let outcome = launch_after_commit(
        &f.pin,
        &kernel,
        &support::intent(&f.scope).id,
        &f.scope,
        3,
        old,
        &AtomicBool::new(false),
    )
    .unwrap();
    assert!(outcome.is_none());
    assert!(!f.root.0.join("launch-count").exists());
    assert_eq!(
        kernel.fixture_effects().effects()[0].status,
        cortex_core::program::operational::EffectStatus::DispatchUncertain
    );
}

#[test]
fn already_used_capture_identity_refuses_before_launch_or_dispatch() {
    let f = durable("count");
    let mut capture = ids();
    capture.started_operation = OperationId::new("prepared").unwrap();
    let before = f.store.load(&f.key, Access::ReadOnly).unwrap().revision();
    assert!(matches!(
        execute_fixture(
            &f.store,
            &f.key,
            f.dispatch,
            capture,
            &f.pin,
            &AtomicBool::new(false)
        ),
        Err(HarnessError::Contract)
    ));
    assert_eq!(
        before,
        f.store.load(&f.key, Access::ReadOnly).unwrap().revision()
    );
    assert!(!f.root.0.join("launch-count").exists());
}
