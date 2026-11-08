#![cfg(target_os = "linux")]
//! Independent CLI counterexamples for scoped memory and retained schema contracts.
#[path = "../../cortex-evidence/tests/support/program.rs"]
mod fixture;
#[path = "../../cortex-evidence/tests/support/operational.rs"]
mod operational_fixture;

use cortex_core::program::{
    domain::PoolIdentity,
    memory::{MemoryBasis, MemoryScope},
    operational::{EffectEvent, EffectStatus, Policy},
    scheduling::{Capacity, Demand, Resource},
};
use cortex_evidence::program::{Command as Input, CoordinationKernel};
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
const PRIVATE_IDENTITY: &str = "synthetic-private-memory-identity";
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        loop {
            let path = std::env::temp_dir().join(format!(
                "cortex-memory-cli-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&path) {
                Ok(()) => {
                    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
                    return Self(path);
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(e) => panic!("fixture directory: {e}"),
            }
        }
    }
    fn store(&self) -> PathBuf {
        self.0.join("state")
    }
    fn create(&self, kernel: &CoordinationKernel) -> ProgramStore {
        let store = ProgramStore::create_fixture(
            &self.store(),
            &key(),
            kernel.initial().clone(),
            ReadLimits::new(16_000_000, Duration::from_secs(20)).unwrap(),
        )
        .unwrap();
        for operation in kernel.operations() {
            store.admit(&key(), operation.clone()).unwrap();
        }
        store
    }
    fn command(&self, schema: Option<&str>, machine: bool) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_cortex"));
        command
            .args(["program", "inspect", "--store"])
            .arg(self.store())
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
        if let Some(schema) = schema {
            command.args(["--schema-version", schema]);
        }
        if machine {
            command.arg("--json");
        }
        command
    }
    fn snapshot(&self) -> Vec<(PathBuf, Vec<u8>, u64, i64, i64)> {
        let mut entries: Vec<_> = fs::read_dir(self.store())
            .unwrap()
            .map(|entry| {
                let path = entry.unwrap().path();
                let metadata = fs::metadata(&path).unwrap();
                (
                    path.clone(),
                    fs::read(&path).unwrap(),
                    metadata.ino(),
                    metadata.mtime(),
                    metadata.mtime_nsec(),
                )
            })
            .collect();
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        entries
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn key() -> StoreKey {
    StoreKey::new(&[0x57; 32]).unwrap()
}
fn memory() -> Resource {
    Resource::memory(
        MemoryBasis::CgroupCharged,
        MemoryScope::AccountingPool(PoolIdentity {
            physical: PRIVATE_IDENTITY.into(),
            incarnation: "allocation-1".into(),
            host: PRIVATE_IDENTITY.into(),
            boot: "boot-1".into(),
        }),
    )
    .unwrap()
}
fn policy() -> Policy {
    let mut value = operational_fixture::policy();
    value.capacities.push(Capacity {
        resource: memory(),
        available: Some(4096),
        verification_reserve: 1024,
        declared_enforceable: true,
    });
    value
}
fn run(mut command: Command) -> Output {
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(&[0x57; 32]).unwrap();
    let start = Instant::now();
    while child.try_wait().unwrap().is_none() {
        if start.elapsed() > Duration::from_secs(20) {
            child.kill().unwrap();
            let _ = child.wait();
            panic!("owned CLI exceeded fixture deadline");
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    child.wait_with_output().unwrap()
}
fn inspect_current_and_reject_lossy_schemas(fixture: &Fixture) -> serde_json::Value {
    let before = fixture.snapshot();
    let current = run(fixture.command(Some("3"), true));
    assert!(
        current.status.success(),
        "{}",
        String::from_utf8_lossy(&current.stderr)
    );
    assert!(!String::from_utf8_lossy(&current.stdout).contains(PRIVATE_IDENTITY));
    let default = run(fixture.command(None, true));
    assert!(default.status.success());
    assert_eq!(default.stdout, current.stdout);
    let value: serde_json::Value = serde_json::from_slice(&current.stdout).unwrap();
    assert_eq!(value["schema"], "cortex.program.v3");
    assert_eq!(
        value["fixture_projection"]["schema"],
        "cortex.fixture-effects.v3"
    );
    let accounting = value["fixture_projection"]["accounting"]
        .as_array()
        .unwrap();
    let memory = accounting
        .iter()
        .find(|entry| entry["resource"]["kind"] == "MemoryBytes")
        .unwrap();
    assert_eq!(memory["resource"]["basis"], "CgroupCharged");
    assert_eq!(memory["resource"]["scope_kind"], "ACCOUNTING_POOL");
    assert!(memory["resource"]["scope_id"].is_u64());
    assert_eq!(
        memory["resource"]["measurement_or_enforcement"],
        "NOT_INFERRED"
    );
    for schema in ["1", "2"] {
        for machine in [false, true] {
            let output = run(fixture.command(Some(schema), machine));
            assert_eq!(output.status.code(), Some(3));
            let stdout = String::from_utf8(output.stdout).unwrap();
            let stderr = String::from_utf8(output.stderr).unwrap();
            assert!(
                stdout.contains("SCHEMA_CANNOT_REPRESENT_MEMORY")
                    || stderr.contains("SCHEMA_CANNOT_REPRESENT_MEMORY")
            );
            assert!(!stdout.contains(PRIVATE_IDENTITY));
            assert!(!stderr.contains(PRIVATE_IDENTITY));
            if machine {
                let error: serde_json::Value = serde_json::from_str(&stdout).unwrap();
                assert_eq!(error["schema"], format!("cortex.program.v{schema}"));
                assert!(error.get("fixture_projection").is_none());
            }
        }
    }
    let human = run(fixture.command(None, false));
    assert!(human.status.success());
    let human = String::from_utf8(human.stdout).unwrap();
    assert!(human.contains("CgroupCharged"));
    assert!(!human.contains(PRIVATE_IDENTITY));
    assert_eq!(before, fixture.snapshot());
    value
}

#[test]
fn scoped_memory_journal_requires_representable_schema_and_preserves_read_only_inspection() {
    let mut kernel = initialized();
    operational_fixture::effect(&mut kernel, "memory-policy", EffectEvent::Policy(policy()));
    let fixture = Fixture::new();
    let store = fixture.create(&kernel);
    let value = inspect_current_and_reject_lossy_schemas(&fixture);
    assert_eq!(
        value["fixture_projection"]["effects"],
        serde_json::json!([])
    );
    assert!(
        store
            .load(&key(), Access::ReadOnly)
            .unwrap()
            .has_scoped_memory()
    );
}

#[test]
fn cancelled_and_revoked_history_cannot_be_downgraded_after_reservations_end() {
    let mut kernel = initialized();
    let scope = operational_fixture::fixture_scope(&mut kernel, "A");
    operational_fixture::effect(&mut kernel, "memory-policy", EffectEvent::Policy(policy()));
    let mut grant = operational_fixture::proposed_grant(&scope);
    grant.ceilings.push(Demand {
        resource: memory(),
        amount: Some(512),
    });
    operational_fixture::effect(
        &mut kernel,
        "memory-grant",
        EffectEvent::Grant(grant.clone()),
    );
    let mut intent = operational_fixture::intent(&scope, "memory-effect");
    intent.demand = grant.ceilings.clone();
    operational_fixture::effect(
        &mut kernel,
        "memory-reserve",
        EffectEvent::Reserve {
            intent: intent.clone(),
            at: 1,
        },
    );
    operational_fixture::effect(
        &mut kernel,
        "memory-cancel",
        EffectEvent::Cancel {
            effect: intent.id,
            scope,
            at: 2,
            basis: source("cancel-before-any-dispatch"),
        },
    );
    operational_fixture::effect(
        &mut kernel,
        "memory-revoke",
        EffectEvent::Revoke {
            grant: grant.id,
            at: 3,
            basis: source("revoked-synthetic-grant"),
        },
    );
    assert!(
        kernel
            .fixture_effects()
            .effects()
            .iter()
            .all(|effect| !effect.pending())
    );
    assert_eq!(
        kernel.fixture_effects().effects()[0].status,
        EffectStatus::CancelledBeforeDispatch
    );
    assert_eq!(kernel.fixture_effects().accounted(memory()).unwrap(), 0);
    // A policy cannot erase the retained dimension. There is no legitimate state
    // with this history but a replacement policy that silently loses its basis.
    let mut erased = operational_fixture::policy();
    erased.reference = source("attempted-memory-erasure");
    let before = kernel.revision();
    let operation = operation(
        &kernel,
        "erase-policy",
        Input::FixtureEffect(Box::new(EffectEvent::Policy(erased))),
    );
    assert!(kernel.admit(operation).is_err());
    assert_eq!(kernel.revision(), before);
    let fixture = Fixture::new();
    let store = fixture.create(&kernel);
    let value = inspect_current_and_reject_lossy_schemas(&fixture);
    let effect = &value["fixture_projection"]["effects"][0];
    assert_eq!(effect["pending"], false);
    assert_eq!(effect["grant_revoked"], true);
    assert_eq!(effect["status"], "CancelledBeforeDispatch");
    let retained = effect["reservation"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["resource"]["kind"] == "MemoryBytes")
        .unwrap();
    assert_eq!(retained["quantity"], 512);
    assert_eq!(retained["resource"]["basis"], "CgroupCharged");
    assert_eq!(
        store.load(&key(), Access::ReadOnly).unwrap().revision(),
        before
    );
}
