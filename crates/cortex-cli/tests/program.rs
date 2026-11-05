#![cfg(target_os = "linux")]
//! GPE-B I1/O: real CLI output, key deadline and absence of recovery mutations.
#[path = "../../cortex-evidence/tests/support/program.rs"]
mod fixture;
#[path = "../../cortex-evidence/tests/support/supervision.rs"]
mod supervised_fixture;
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
fn supervised_inspection_preserves_unknown_and_cancellation_without_launch_or_private_spec() {
    use cortex_core::program::{operational::*, supervision::*};
    let f = Temp::new();
    let (mut k, scope, mut spec) = supervised_fixture::configured_kernel(None);
    spec.args = vec!["private-fixture-argument-sentinel".into()];
    supervised_fixture::prepare(&mut k, &scope, &spec);
    let input = supervised_fixture::dispatch_operation(&k, &scope);
    k.admit(input).unwrap();
    for (id, event) in [
        (
            "spawned",
            EffectEvent::ProcessObserved {
                effect: supervised_fixture::intent(&scope).id,
                scope: scope.clone(),
                at: 4,
                observation: supervised_fixture::observation(
                    &spec,
                    1,
                    ProcessCompletion::Spawned,
                    Quiescence::Unknown,
                ),
            },
        ),
        (
            "cancel",
            EffectEvent::Cancel {
                effect: supervised_fixture::intent(&scope).id,
                scope: scope.clone(),
                at: 5,
                basis: source("operator-stop"),
            },
        ),
        (
            "unknown",
            EffectEvent::ProcessObserved {
                effect: supervised_fixture::intent(&scope).id,
                scope: scope.clone(),
                at: 6,
                observation: supervised_fixture::observation(
                    &spec,
                    2,
                    ProcessCompletion::Unknown,
                    Quiescence::Unknown,
                ),
            },
        ),
    ] {
        supervised_fixture::effect(&mut k, id, event);
    }
    let s =
        ProgramStore::create_supervised_fixture(&f.store(), &key(), k.initial().clone(), limits())
            .unwrap();
    for op in k.operations() {
        s.admit(&key(), op.clone()).unwrap();
    }
    let before = snapshot(&f);
    let mut c = command(&f);
    c.arg("--json");
    let result = run(c, &[0x57; 32]);
    assert!(result.status.success());
    let text = String::from_utf8(result.stdout).unwrap();
    assert!(!text.contains("private-fixture-argument-sentinel"));
    assert!(!text.contains("/explicit/native-fixture"));
    assert!(!text.contains("Bounded JSON-lines fixture output"));
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert!(value["fixture_projection"].is_null());
    assert_eq!(
        value["supervised_projection"]["schema"],
        "cortex.supervised-fixtures.v1"
    );
    assert_eq!(value["live_executor_authority"], "UNAVAILABLE");
    assert_eq!(
        value["supervised_projection"]["isolation_backend_qualified"],
        false
    );
    let effect = &value["supervised_projection"]["effects"][0];
    assert_eq!(effect["status"], "Unknown");
    assert_eq!(effect["pending_reconciliation"], true);
    assert_eq!(effect["cancellation_requested"], true);
    assert_eq!(effect["dispatch_intent_durable"], true);
    assert_eq!(effect["observations"][1]["completion"]["kind"], "UNKNOWN");
    assert_eq!(value["resource_reservations"].as_array().unwrap().len(), 1);
    assert_eq!(
        value["resource_reservations"][0]["hard_host_reservation"],
        "NOT_ESTABLISHED"
    );
    let human = run(command(&f), &[0x57; 32]);
    assert!(human.status.success());
    let human = String::from_utf8(human.stdout).unwrap();
    assert!(human.contains("Supervised local fixture effect: Unknown"));
    assert!(human.contains("no live executor or sandbox guarantee"));
    assert_eq!(snapshot(&f), before);
    assert_eq!(
        s.load(&key(), Access::ReadOnly).unwrap().revision(),
        k.revision()
    );
}

#[test]
fn supervised_reported_fixture_input_is_visible_without_implying_acceptance() {
    use cortex_core::program::{operational::*, supervision::*};
    let f = Temp::new();
    let (mut k, scope, spec) = supervised_fixture::prepared_kernel(None);
    let input = supervised_fixture::dispatch_operation(&k, &scope);
    k.admit(input).unwrap();
    for (n, completion, quiescence) in [
        (1, ProcessCompletion::Spawned, Quiescence::Unknown),
        (
            2,
            ProcessCompletion::Exited {
                code: Some(0),
                signal: None,
            },
            Quiescence::DirectChildReapedStreamsClosed,
        ),
    ] {
        let observation = supervised_fixture::observation(&spec, n, completion, quiescence);
        supervised_fixture::effect(
            &mut k,
            &format!("capture-{n}"),
            EffectEvent::ProcessObserved {
                effect: supervised_fixture::intent(&scope).id,
                scope: scope.clone(),
                at: 3 + n,
                observation,
            },
        );
    }
    let s =
        ProgramStore::create_supervised_fixture(&f.store(), &key(), k.initial().clone(), limits())
            .unwrap();
    for op in k.operations() {
        s.admit(&key(), op.clone()).unwrap();
    }
    let before = snapshot(&f);
    let mut c = command(&f);
    c.arg("--json");
    let output = run(c, &[0x57; 32]);
    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["packages"][0]["submitted"], true);
    assert_eq!(value["packages"][0]["provider_submission"], "NOT_SUPPORTED");
    let effect = &value["supervised_projection"]["effects"][0];
    assert_eq!(effect["status"], "Reported");
    assert_eq!(effect["pending_reconciliation"], true);
    assert_eq!(effect["mission_acceptance"], "NOT_INFERRED");
    assert_eq!(effect["observations"][1]["raw_output_retained"], false);
    assert_eq!(value["integration"], "NOT_SUPPORTED");
    assert_eq!(snapshot(&f), before);
    let last = k.fixture_effects().effects()[0]
        .process_observations
        .last()
        .unwrap();
    let usage = supervised_fixture::usage(last);
    let reconciliation = operation(
        &k,
        "settle",
        Input::SupervisedEffect(Box::new(EffectEvent::Reconcile {
            effect: supervised_fixture::intent(&scope).id,
            scope: scope.clone(),
            at: 6,
            resolution: Reconciliation::Observed {
                outcome: EffectOutcome::Succeeded,
                usage,
            },
            basis: source("explicit-exact-fixture-settlement"),
        })),
    );
    s.admit(&key(), reconciliation).unwrap();
    let before = snapshot(&f);
    let mut c = command(&f);
    c.arg("--json");
    let output = run(c, &[0x57; 32]);
    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(
        value["resource_reservations"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let accounting = value["supervised_projection"]["accounting"]
        .as_array()
        .unwrap();
    assert_eq!(
        accounting
            .iter()
            .find(|a| a["resource"] == "TimeMillis")
            .unwrap()["accounted"],
        2
    );
    assert_eq!(
        accounting
            .iter()
            .find(|a| a["resource"] == "OutputBytes")
            .unwrap()["accounted"],
        3
    );
    assert_eq!(
        accounting
            .iter()
            .find(|a| a["resource"] == "Workers")
            .unwrap()["accounted"],
        0
    );
    assert_eq!(snapshot(&f), before);
}
#[test]
fn extended_fixture_limits_are_inspectable_without_dispatch_or_store_mutation() {
    use cortex_core::program::supervision::TransportLimits;
    let f = Temp::new();
    let (mut k, scope, mut spec) = supervised_fixture::configured_kernel(None);
    spec.limits.transport = Some(TransportLimits {
        startup_ms: 100,
        idle_ms: 50,
        stdout_bytes: 256,
        stderr_bytes: 0,
        records: 2,
        buffer_bytes: spec.input_len + spec.limits.frame_bytes + 8192,
    });
    supervised_fixture::prepare(&mut k, &scope, &spec);
    let s =
        ProgramStore::create_supervised_fixture(&f.store(), &key(), k.initial().clone(), limits())
            .unwrap();
    for op in k.operations() {
        s.admit(&key(), op.clone()).unwrap();
    }
    let before = snapshot(&f);
    let mut c = command(&f);
    c.arg("--json");
    let result = run(c, &[0x57; 32]);
    assert!(result.status.success());
    let view: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    let effect = &view["supervised_projection"]["effects"][0];
    assert_eq!(effect["dispatch_intent_durable"], false);
    assert!(effect["observations"].as_array().unwrap().is_empty());
    let limits = &effect["configured_process_limits"];
    assert_eq!(limits["wall_ms"], spec.limits.wall_ms);
    assert_eq!(limits["transport"]["startup_ms"], 100);
    assert_eq!(limits["transport"]["idle_ms"], 50);
    assert_eq!(limits["transport"]["stderr_bytes"], 0);
    assert_eq!(limits["transport"]["records"], 2);
    assert_eq!(
        limits["enforcement_scope"],
        "TRUSTED_FIXTURE_COOPERATIVE_TRANSPORT_NOT_HOST_ISOLATION"
    );
    assert_eq!(snapshot(&f), before);
    assert_eq!(
        s.load(&key(), Access::ReadOnly).unwrap().revision(),
        k.revision()
    );
}

#[test]
fn owned_scope_inspection_keeps_requested_observed_and_unknown_facts_separate() {
    use cortex_core::program::{operational::*, supervision::*};
    let f = Temp::new();
    let (mut k, scope, mut spec) = supervised_fixture::configured_scope_kernel(None);
    let request = spec.containment.as_mut().unwrap();
    request.unit = "cortex-qualification-private-unit.scope".into();
    request.delegated_root = "/sys/fs/cgroup/private-owned-root".into();
    let request = request.clone();
    supervised_fixture::prepare(&mut k, &scope, &spec);
    k.admit(supervised_fixture::dispatch_operation(&k, &scope))
        .unwrap();
    for n in 1..=2 {
        let mut observation = supervised_fixture::observation(
            &spec,
            n,
            if n == 1 {
                ProcessCompletion::Spawned
            } else {
                ProcessCompletion::Unknown
            },
            Quiescence::Unknown,
        );
        let mut captured = supervised_fixture::scope_observation(&request, Some(true));
        if n == 2 {
            captured.controls = None;
            captured.populated = None;
            captured.helper_quiescent = false;
            captured.lifecycle_verified = false;
        }
        observation.containment = Some(Box::new(captured));
        supervised_fixture::effect(
            &mut k,
            &format!("scope-capture-{n}"),
            EffectEvent::ProcessObserved {
                effect: supervised_fixture::intent(&scope).id,
                scope: scope.clone(),
                at: 3 + n,
                observation,
            },
        );
    }
    let s =
        ProgramStore::create_supervised_fixture(&f.store(), &key(), k.initial().clone(), limits())
            .unwrap();
    for op in k.operations() {
        s.admit(&key(), op.clone()).unwrap();
    }
    let before = snapshot(&f);
    let mut c = command(&f);
    c.arg("--json");
    let output = run(c, &[0x57; 32]);
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    for private in [
        "private-unit",
        "private-owned-root",
        "123456789abcdef0123456789abcdef0",
    ] {
        assert!(!text.contains(private));
    }
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    let projected = &value["supervised_projection"];
    assert_eq!(projected["isolation_backend_qualified"], false);
    assert_eq!(projected["live_executor_supported"], false);
    let effect = &projected["effects"][0];
    assert_eq!(effect["pending_reconciliation"], true);
    assert_eq!(
        effect["containment_request"]["requested_controls"]["cpu_quota_us"],
        100_000
    );
    assert_eq!(
        effect["containment_request"]["cpu_time_enforcement"],
        "COOPERATIVE_SAMPLING_NOT_HARD_CUMULATIVE_QUOTA"
    );
    let first = &effect["observations"][0]["containment"];
    assert_eq!(
        first["controls_readback"]["charged_memory_bytes"],
        536_870_912_u64
    );
    assert_eq!(first["populated"], true);
    let last = &effect["observations"][1]["containment"];
    assert!(last["controls_readback"].is_null());
    assert!(last["populated"].is_null());
    assert_eq!(last["helper_quiescent"], false);
    assert_eq!(last["lifecycle_verified"], false);
    assert_eq!(last["cpu_usage_us"], 10_000);
    assert_eq!(last["qualification"], "NOT_INFERRED");
    assert_eq!(
        last["measurement_scope"],
        "RETAINED_CAPTURE_NOT_CURRENT_HOST_REINSPECTION"
    );
    assert_eq!(snapshot(&f), before);
    assert_eq!(
        s.load(&key(), Access::ReadOnly).unwrap().revision(),
        k.revision()
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

#[path = "../../cortex-evidence/tests/support/operational.rs"]
mod operational_fixture;
#[test]
fn fixture_unknown_and_transport_preparation_are_explicit_and_inspection_has_no_effects() {
    use cortex_core::program::operational::*;
    let f = Temp::new();
    let (mut k, scope) = operational_fixture::fixture_kernel();
    operational_fixture::effect(
        &mut k,
        "reserve",
        EffectEvent::Reserve {
            intent: operational_fixture::intent(&scope, "fixture-effect"),
            at: 1,
        },
    );
    operational_fixture::effect(
        &mut k,
        "dispatch",
        EffectEvent::Dispatch {
            effect: EffectId::new("fixture-effect").unwrap(),
            scope: scope.clone(),
            at: 2,
            basis: source("fixture-only"),
        },
    );
    operational_fixture::effect(
        &mut k,
        "cancel",
        EffectEvent::Cancel {
            effect: EffectId::new("fixture-effect").unwrap(),
            scope: scope.clone(),
            at: 3,
            basis: source("requested-cancellation"),
        },
    );
    operational_fixture::effect(
        &mut k,
        "unknown",
        EffectEvent::Unknown {
            effect: EffectId::new("fixture-effect").unwrap(),
            scope: scope.clone(),
            at: 3,
            basis: source("lost-reply"),
        },
    );
    admit(
        &mut k,
        "prepare",
        Input::PrepareExecutorInput(Box::new(operational_fixture::preparation(&scope))),
    );
    let store =
        ProgramStore::create_fixture(&f.store(), &key(), k.initial().clone(), limits()).unwrap();
    for operation in k.operations() {
        store.admit(&key(), operation.clone()).unwrap();
    }
    let before = snapshot(&f);
    let mut c = command(&f);
    c.arg("--json");
    let output = run(c, &[0x57; 32]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let view: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(view["execution_authority"], "UNAVAILABLE");
    assert_eq!(view["resource_reservations"], serde_json::json!([]));
    let fixtures = &view["fixture_projection"];
    assert_eq!(fixtures["mode"], "CONTROLLED_FIXTURE_ONLY");
    assert_eq!(fixtures["real_execution_supported"], false);
    assert_eq!(fixtures["effects"][0]["status"], "Unknown");
    assert_eq!(fixtures["effects"][0]["pending"], true);
    assert_eq!(fixtures["effects"][0]["cancellation_requested"], true);
    assert_eq!(fixtures["effects"][0]["authorized"], false);
    assert_eq!(fixtures["effects"][0]["real_dispatched"], false);
    assert_eq!(fixtures["prepared_inputs"][0]["submitted"], false);
    assert!(
        !String::from_utf8_lossy(&output.stdout).contains("Controlled fixture observation only")
    );
    let human = run(command(&f), &[0x57; 32]);
    assert!(human.status.success());
    assert!(
        String::from_utf8_lossy(&human.stdout)
            .contains("Fixture effect fixture-effect: Unknown pending=true")
    );
    assert_eq!(snapshot(&f), before);
    assert_eq!(
        store.load(&key(), Access::ReadOnly).unwrap().revision(),
        k.revision()
    );
}

#[test]
fn recovery_inspection_explains_missing_scope_and_helper_without_changing_the_journal() {
    use cortex_core::program::{operational::*, supervision::*};
    let f = Temp::new();
    let (mut k, scope, spec) = supervised_fixture::configured_scope_kernel(None);
    supervised_fixture::prepare(&mut k, &scope, &spec);
    k.admit(supervised_fixture::dispatch_operation(&k, &scope))
        .unwrap();
    let mut o = supervised_fixture::observation(
        &spec,
        1,
        ProcessCompletion::Interrupted {
            reason: InterruptionReason::InputClosed,
        },
        Quiescence::Unknown,
    );
    o.input_bytes = 0;
    o.unresolved_scope_helpers = vec![ScopeHelperIdentity {
        pid: 12345,
        start_ticks: Some(98765),
    }];
    supervised_fixture::effect(
        &mut k,
        "missing-scope-receipt",
        EffectEvent::ProcessObserved {
            effect: supervised_fixture::intent(&scope).id,
            scope: scope.clone(),
            at: 4,
            observation: o,
        },
    );
    let store =
        ProgramStore::create_supervised_fixture(&f.store(), &key(), k.initial().clone(), limits())
            .unwrap();
    for op in k.operations() {
        store.admit(&key(), op.clone()).unwrap();
    }
    let before = snapshot(&f);
    let mut c = command(&f);
    c.arg("--json");
    let result = run(c, &[0x57; 32]);
    assert!(result.status.success());
    let v: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    let effect = &v["supervised_projection"]["effects"][0];
    assert_eq!(effect["status"], "Unknown");
    let recovery = &effect["recovery"];
    assert_eq!(recovery["reservation_held"], true);
    assert_eq!(recovery["scope_recorded"], false);
    assert_eq!(recovery["identity_checkpoint_recorded"], false);
    assert!(recovery["existing_proof_candidate"].is_null());
    let reasons = recovery["blockers"].as_array().unwrap();
    assert!(reasons.iter().any(|v| v == "MissingScopeIdentity"));
    assert!(reasons.iter().any(|v| v == "UnresolvedHelper"));
    assert_eq!(recovery["authority"], "NONE_INSPECTION_ONLY");
    assert_eq!(recovery["host_reinspection"], false);
    assert_eq!(recovery["current_process_liveness"], "NOT_OBSERVED");
    assert_eq!(snapshot(&f), before);
    assert_eq!(
        store.load(&key(), Access::ReadOnly).unwrap().revision(),
        k.revision()
    );
}

#[test]
fn recovery_proof_candidate_does_not_release_a_valid_pre_spawn_reservation() {
    use cortex_core::program::{operational::*, supervision::*};
    let f = Temp::new();
    let (mut k, scope, spec) = supervised_fixture::configured_kernel(None);
    supervised_fixture::prepare(&mut k, &scope, &spec);
    k.admit(supervised_fixture::dispatch_operation(&k, &scope))
        .unwrap();
    let o = supervised_fixture::observation(
        &spec,
        1,
        ProcessCompletion::StartFailed,
        Quiescence::NoChildStarted,
    );
    supervised_fixture::effect(
        &mut k,
        "pre-spawn-refusal",
        EffectEvent::ProcessObserved {
            effect: supervised_fixture::intent(&scope).id,
            scope: scope.clone(),
            at: 4,
            observation: o,
        },
    );
    let store =
        ProgramStore::create_supervised_fixture(&f.store(), &key(), k.initial().clone(), limits())
            .unwrap();
    for op in k.operations() {
        store.admit(&key(), op.clone()).unwrap();
    }
    let before = snapshot(&f);
    let mut c = command(&f);
    c.arg("--json");
    let result = run(c, &[0x57; 32]);
    assert!(result.status.success());
    let v: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    let effect = &v["supervised_projection"]["effects"][0];
    assert_eq!(effect["recovery"]["existing_proof_candidate"], "ABSENT");
    assert_eq!(effect["recovery"]["reservation_held"], true);
    assert_eq!(effect["pending_reconciliation"], true);
    assert_eq!(snapshot(&f), before);
}
