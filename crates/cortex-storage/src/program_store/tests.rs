//! GPE-B S1/O1/F1: exact reconstruction, real competing processes and crash cuts.
use super::*;
#[path = "../../../cortex-evidence/tests/support/program.rs"]
mod fixture;
use cortex_core::binding::{CandidateBindingContract, CandidateRequirement, RepositoryIdentity};
use cortex_core::program::*;
use cortex_evidence::program::Command as Input;
use fixture::*;
use std::{
    io::Write,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        loop {
            let p = std::env::temp_dir().join(format!(
                "cortex-program-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&p) {
                Ok(()) => return Self(p),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(e) => panic!("fixture directory: {e}"),
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
    ProgramStore::create(&f.store(), &key(), definition(), limits()).unwrap()
}
fn claim() -> Operation {
    operation(
        &CoordinationKernel::new(definition()).unwrap(),
        "claim-A",
        Input::Claim(attempt("A-attempt", "A", "operator")),
    )
}
fn snapshot(f: &Temp) -> Vec<(PathBuf, Vec<u8>, u64, i64, i64)> {
    let mut out: Vec<_> = fs::read_dir(f.store())
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
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}
fn store_kernel(f: &Temp, k: &CoordinationKernel) -> ProgramStore {
    let s = ProgramStore::create(&f.store(), &key(), k.initial().clone(), limits()).unwrap();
    for o in k.operations() {
        s.admit(&key(), o.clone()).unwrap();
    }
    s
}

#[test]
fn amendment_checks_each_declared_source_root_before_appending() {
    let f = Temp::new();
    let store = create(&f);
    let original = store.load(&key(), Access::ReadOnly).unwrap();
    let before = snapshot(&f);
    let alias = f.0.join("source-alias");
    std::os::unix::fs::symlink(&f.0, &alias).unwrap();
    for (which, root) in [
        (0, f.0.clone()),
        (1, f.0.clone()),
        (2, f.0.clone()),
        (0, alias),
    ] {
        let next = with_contracts(&definition_revision(2, false), |contract| {
            let repo = contract.candidate().repository();
            let mut roots = [
                repo.root().to_owned(),
                repo.git_dir().to_owned(),
                repo.common_dir().to_owned(),
            ];
            roots[which] = root.clone();
            CandidateBindingContract::new(
                contract.task().clone(),
                rev(2),
                CandidateRequirement::new(
                    RepositoryIdentity::new(roots[0].clone(), roots[1].clone(), roots[2].clone())
                        .unwrap(),
                    contract.candidate().commit().clone(),
                    contract.candidate().worktree(),
                ),
            )
        });
        let input = operation(
            &original,
            "source-amendment",
            Input::Amend {
                next: Box::new(next),
                non_impact: cortex_evidence::program::NonImpact {
                    missions: vec![],
                    basis: source("all-mission-contracts-changed"),
                },
            },
        );
        let failure = store.admit(&key(), input).unwrap_err();
        assert_eq!(failure.completion, Completion::Rejected);
        assert!(
            matches!(failure.error, ProgramStoreError::Storage(e) if e.kind() == ErrorKind::InvalidLocation)
        );
        assert_eq!(snapshot(&f), before);
        assert_eq!(store.load(&key(), Access::ReadOnly).unwrap().revision(), 0);
    }
    let neighbor = with_contracts(&definition_revision(2, false), |contract| {
        let root = f.0.join("independent-source");
        CandidateBindingContract::new(
            contract.task().clone(),
            rev(2),
            CandidateRequirement::new(
                RepositoryIdentity::new(root.clone(), root.join(".git"), root.join(".git"))
                    .unwrap(),
                contract.candidate().commit().clone(),
                contract.candidate().worktree(),
            ),
        )
    });
    store
        .admit(
            &key(),
            operation(
                &original,
                "source-amendment",
                Input::Amend {
                    next: Box::new(neighbor),
                    non_impact: cortex_evidence::program::NonImpact {
                        missions: vec![],
                        basis: source("all-mission-contracts-changed"),
                    },
                },
            ),
        )
        .unwrap();
    assert_eq!(
        store
            .load(&key(), Access::ReadOnly)
            .unwrap()
            .state()
            .definitions()
            .len(),
        2
    );
}

#[test]
fn moved_store_inside_declared_source_is_refused_without_inspection_writes() {
    let source_root = Temp::new();
    let outside = Temp::new();
    let definition = with_contracts(&definition(), |contract| {
        CandidateBindingContract::new(
            contract.task().clone(),
            contract.revision(),
            CandidateRequirement::new(
                RepositoryIdentity::new(
                    source_root.0.clone(),
                    source_root.0.join(".git"),
                    source_root.0.join(".git"),
                )
                .unwrap(),
                contract.candidate().commit().clone(),
                contract.candidate().worktree(),
            ),
        )
    });
    ProgramStore::create(&outside.store(), &key(), definition, limits()).unwrap();
    fs::create_dir(source_root.store()).unwrap();
    fs::set_permissions(source_root.store(), fs::Permissions::from_mode(0o700)).unwrap();
    for name in [".lock", "program.db"] {
        fs::copy(outside.store().join(name), source_root.store().join(name)).unwrap();
    }
    let before = snapshot(&source_root);
    assert!(ProgramStore::open(&source_root.store(), &key(), Access::ReadOnly, limits()).is_err());
    assert!(ProgramStore::inspect(&source_root.store(), &key(), limits()).is_err());
    assert_eq!(snapshot(&source_root), before);
    assert!(ProgramStore::inspect(&outside.store(), &key(), limits()).is_ok());
}

#[test]
fn equivalent_scope_path_spellings_replay_exact_bytes_and_historical_duplicates() {
    for suffix in ["/", "//"] {
        let original = definition();
        let equivalent = with_contracts(&original, |contract| {
            let repo = contract.candidate().repository();
            let spelling = |p: &Path| PathBuf::from(format!("{}{suffix}", p.display()));
            CandidateBindingContract::new(
                contract.task().clone(),
                contract.revision(),
                CandidateRequirement::new(
                    RepositoryIdentity::new(
                        spelling(repo.root()),
                        spelling(repo.git_dir()),
                        spelling(repo.common_dir()),
                    )
                    .unwrap(),
                    contract.candidate().commit().clone(),
                    contract.candidate().worktree(),
                ),
            )
        });
        assert!(original.graph().missions() == equivalent.graph().missions());
        let mut kernel = CoordinationKernel::new(equivalent).unwrap();
        let release = prepare_release(&mut kernel, "AB", true);
        admit(&mut kernel, "release", Input::Release(Box::new(release)));
        let f = Temp::new();
        let store = ProgramStore::create(&f.store(), &key(), original, limits()).unwrap();
        for operation in kernel.operations() {
            store.admit(&key(), operation.clone()).unwrap();
        }
        let loaded = store.load(&key(), Access::ReadOnly).unwrap();
        assert!(loaded.operations() == kernel.operations());
        for (loaded, original) in loaded.operations().iter().zip(kernel.operations()) {
            assert_eq!(
                codec::operation(loaded, 16_000_000).unwrap().as_slice(),
                codec::operation(original, 16_000_000).unwrap().as_slice()
            );
        }
        let historical = kernel.operations().last().unwrap().clone();
        store
            .admit(
                &key(),
                operation(
                    &loaded,
                    "amend",
                    Input::Amend {
                        next: Box::new(definition_revision(2, true)),
                        non_impact: cortex_evidence::program::NonImpact {
                            missions: vec![task("D"), task("E")],
                            basis: source("unchanged-components"),
                        },
                    },
                ),
            )
            .unwrap();
        let before = snapshot(&f);
        assert!(store.admit(&key(), historical).unwrap().duplicate);
        assert_eq!(snapshot(&f), before);
    }
}

#[test]
fn evidence_graph_packages_and_unknown_attempts_survive_exact_encrypted_reopen() {
    let f = Temp::new();
    let mut k = initialized();
    release(&mut k, "AB");
    release(&mut k, "DE");
    admit(
        &mut k,
        "claim",
        Input::Claim(attempt("A-attempt", "A", "operator")),
    );
    admit(
        &mut k,
        "unknown",
        Input::Attempt {
            id: AttemptId::new("A-attempt").unwrap(),
            generation: rev(1),
            status: AttemptStatus::Unknown,
            basis: source("lost-coordinator"),
        },
    );
    let s = store_kernel(&f, &k);
    let before = snapshot(&f);
    let opened = ProgramStore::open(&f.store(), &key(), Access::ReadOnly, limits()).unwrap();
    assert_eq!(s.id(), opened.id());
    let loaded = opened.load(&key(), Access::ReadOnly).unwrap();
    assert!(loaded.operations() == k.operations());
    assert!(loaded.records() == k.records());
    assert!(loaded.decisions() == k.decisions());
    assert_eq!(loaded.state().attempts()[0].status, AttemptStatus::Unknown);
    assert!(
        loaded
            .state()
            .readiness()
            .mission(&task("B"))
            .unwrap()
            .eligible()
    );
    assert!(
        loaded
            .state()
            .readiness()
            .mission(&task("E"))
            .unwrap()
            .eligible()
    );
    assert_eq!(snapshot(&f), before);
    let data = fs::read(f.store().join("program.db")).unwrap();
    assert!(!data.starts_with(b"SQLite format"));
    assert!(!data.windows(16).any(|b| b == b"fixture-operator"));
    assert_eq!(fs::metadata(f.store()).unwrap().mode() & 0o777, 0o700);
}
#[test]
fn atomic_expected_version_refusal_duplicate_and_conflicting_identity() {
    let f = Temp::new();
    let s = create(&f);
    let first = claim();
    let r = s.admit(&key(), first.clone()).unwrap();
    assert_eq!(r.ordinal, 1);
    assert!(s.admit(&key(), first.clone()).unwrap().duplicate);
    let before = snapshot(&f);
    let mut stale = first.clone();
    stale.id = OperationId::new("stale").unwrap();
    let e = s.admit(&key(), stale).unwrap_err();
    assert_eq!(e.completion, Completion::Rejected);
    assert_eq!(snapshot(&f), before);
    let mut changed = first.clone();
    changed.expected = 1;
    assert!(s.admit(&key(), changed.clone()).is_err());
    assert_eq!(
        s.reconcile(&key(), &definition(), &changed, Access::ReadOnly)
            .unwrap(),
        Reconciliation::Conflict
    );
    assert!(matches!(
        s.reconcile(&key(), &definition(), &first, Access::ReadOnly)
            .unwrap(),
        Reconciliation::Present(_)
    ));
}
#[test]
fn amendment_roundtrip_preserves_unaffected_releases_and_prior_basis() {
    let f = Temp::new();
    let mut k = initialized();
    release(&mut k, "AB");
    release(&mut k, "DE");
    admit(
        &mut k,
        "amend",
        Input::Amend {
            next: Box::new(definition_revision(2, true)),
            non_impact: cortex_evidence::program::NonImpact {
                missions: vec![task("D"), task("E")],
                basis: source("complete-independent-coverage"),
            },
        },
    );
    let s = store_kernel(&f, &k);
    let loaded = s.load(&key(), Access::ReadOnly).unwrap();
    assert!(loaded.operations() == k.operations());
    assert_eq!(loaded.state().definitions().len(), 2);
    assert!(
        loaded
            .state()
            .readiness()
            .mission(&task("E"))
            .unwrap()
            .eligible()
    );
    assert!(
        !loaded
            .state()
            .readiness()
            .mission(&task("B"))
            .unwrap()
            .eligible()
    );
}
#[test]
fn missing_replaced_wrong_key_and_wrong_format_do_not_become_empty_program() {
    let f = Temp::new();
    let s = create(&f);
    assert!(
        ProgramStore::open(
            &f.store(),
            &StoreKey::new(&[0x66; 32]).unwrap(),
            Access::ReadOnly,
            limits()
        )
        .is_err()
    );
    let other = Temp::new();
    let replacement = create(&other);
    assert_ne!(replacement.id(), s.id());
    fs::copy(
        other.store().join("program.db"),
        f.store().join("program.db"),
    )
    .unwrap();
    assert!(matches!(
        s.load(&key(), Access::ReadOnly),
        Err(ProgramStoreError::TargetChanged)
    ));
    fs::rename(f.store().join("program.db"), f.0.join("saved.db")).unwrap();
    assert!(
        s.reconcile(&key(), &definition(), &claim(), Access::ReadOnly)
            .is_err()
    );
}
#[test]
fn format_schema_ordinal_and_payload_corruption_are_refused() {
    for mutation in [
        "UPDATE program_meta SET format=2",
        "CREATE TABLE unexpected(x TEXT)",
        "UPDATE program_inputs SET ordinal=3",
        "UPDATE program_inputs SET payload=x'5b5d'",
        "UPDATE program_inputs SET operation='different'",
    ] {
        let f = Temp::new();
        let s = create(&f);
        s.admit(&key(), claim()).unwrap();
        let c = history_store::connect(&s.location, &key(), Access::Recover).unwrap();
        c.execute_batch(mutation).unwrap();
        drop(c);
        assert!(
            s.load(&key(), Access::ReadOnly).is_err(),
            "accepted corrupted structure"
        );
    }
}
#[test]
fn caller_byte_capacity_and_read_deadline_refuse_without_appending() {
    let f = Temp::new();
    let d = definition();
    let initial = codec::definition(&d).unwrap();
    let s = ProgramStore::create(
        &f.store(),
        &key(),
        d,
        ReadLimits::new(initial.len() + 1, Duration::from_secs(20)).unwrap(),
    )
    .unwrap();
    let before = snapshot(&f);
    let e = s.admit(&key(), claim()).unwrap_err();
    assert_eq!(e.completion, Completion::Rejected);
    assert!(matches!(e.error, ProgramStoreError::Capacity));
    assert_eq!(snapshot(&f), before);
    assert!(
        ProgramStore::open(
            &f.store(),
            &key(),
            Access::ReadOnly,
            ReadLimits::new(16_000_000, Duration::from_nanos(1)).unwrap()
        )
        .is_err()
    );
}
#[test]
fn symlinks_extra_files_wrong_mode_and_source_location_are_rejected() {
    let f = Temp::new();
    let s = create(&f);
    let link = f.0.join("alias");
    std::os::unix::fs::symlink(f.store(), &link).unwrap();
    assert!(ProgramStore::open(&link, &key(), Access::ReadOnly, limits()).is_err());
    fs::write(f.store().join("unrelated"), b"preserve").unwrap();
    assert!(s.load(&key(), Access::ReadOnly).is_err());
    assert_eq!(fs::read(f.store().join("unrelated")).unwrap(), b"preserve");
    fs::set_permissions(f.store(), fs::Permissions::from_mode(0o755)).unwrap();
    assert!(s.load(&key(), Access::ReadOnly).is_err());
}
#[test]
fn removal_requires_matching_scope_and_does_not_recreate_missing_data() {
    let f = Temp::new();
    let s = create(&f);
    s.admit(&key(), claim()).unwrap();
    assert!(
        s.remove(
            &key(),
            RemovalRequest::whole_store(StoreId::from_bytes([1; 16]))
        )
        .is_err()
    );
    s.remove(&key(), RemovalRequest::whole_store(s.id()))
        .unwrap();
    assert!(!f.store().join("program.db").exists());
    assert!(s.load(&key(), Access::ReadOnly).is_err());
    assert!(ProgramStore::create(&f.store(), &key(), definition(), limits()).is_err());
}
fn child(f: &Temp, mode: &str) -> Child {
    let mut c = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "program_store::tests::process_child",
            "--ignored",
            "--nocapture",
        ])
        .env("CORTEX_PROGRAM_TEST_ROOT", &f.0)
        .env("CORTEX_PROGRAM_TEST_MODE", mode)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    c.stdin.take().unwrap().write_all(&[0x57; 32]).unwrap();
    c
}
fn wait_file(path: &Path, child: &mut Child) {
    let start = Instant::now();
    while !path.exists() {
        assert!(
            child.try_wait().unwrap().is_none(),
            "child ended before boundary"
        );
        assert!(
            start.elapsed() < Duration::from_secs(20),
            "child boundary timeout"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}
fn wait_child(mut child: Child) {
    let start = Instant::now();
    while child.try_wait().unwrap().is_none() {
        if start.elapsed() > Duration::from_secs(30) {
            child.kill().unwrap();
            panic!("child deadline");
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let o = child.wait_with_output().unwrap();
    assert!(
        o.status.success(),
        "child failed: {}",
        String::from_utf8_lossy(&o.stderr)
    );
}
#[test]
#[ignore = "protocol child; selected explicitly by parent process tests with stdin key"]
fn process_child() {
    let Some(root) = std::env::var_os("CORTEX_PROGRAM_TEST_ROOT") else {
        return;
    };
    let root = PathBuf::from(root);
    let mode = std::env::var("CORTEX_PROGRAM_TEST_MODE").unwrap();
    let mut bytes = [0u8; 32];
    std::io::stdin().read_exact(&mut bytes).unwrap();
    let key = StoreKey::new(&bytes).unwrap();
    let s = ProgramStore::open(&root.join("store"), &key, Access::Recover, limits()).unwrap();
    if mode.starts_with("supervised-") {
        supervised::process_child(&root, &s, &key, &mode);
        return;
    }
    if mode.starts_with("effect-") {
        operational::process_child(&root, &s, &key, &mode);
        return;
    }
    if mode == "reopen" {
        let k = s.load(&key, Access::ReadOnly).unwrap();
        assert_eq!(k.state().attempts()[0].status, AttemptStatus::Unknown);
        return;
    }
    if mode == "fault" {
        fs::write(root.join("armed"), b"arm").unwrap();
        let failure = s
            .admit_observed(&key, claim(), |stage, c| {
                if matches!(stage, Stage::BeforeWrite) {
                    c.execute_batch("PRAGMA cache_size=1").unwrap();
                }
            })
            .unwrap_err();
        let completion = failure.completion;
        let reconciled = s
            .reconcile(&key, &definition(), &claim(), Access::Recover)
            .unwrap();
        if matches!(completion, Completion::Rejected | Completion::RolledBack) {
            assert_eq!(reconciled, Reconciliation::Absent);
        } else {
            assert!(matches!(
                reconciled,
                Reconciliation::Absent | Reconciliation::Present(_)
            ));
        }
        fs::write(
            root.join("fault-result"),
            format!("{completion:?} {reconciled:?}"),
        )
        .unwrap();
        return;
    }
    if mode.starts_with("race-") {
        let label = mode.trim_start_matches("race-");
        let mut o = claim();
        o.id = OperationId::new(label).unwrap();
        if let Input::Claim(a) = &mut o.command {
            a.id = AttemptId::new(label).unwrap();
        }
        fs::write(root.join(format!("ready-{label}")), b"ready").unwrap();
        let start = Instant::now();
        while !root.join("go").exists() {
            assert!(start.elapsed() < Duration::from_secs(20));
            std::thread::sleep(Duration::from_millis(5));
        }
        let r = s.admit(&key, o);
        fs::write(
            root.join(format!("result-{label}")),
            if r.is_ok() { "committed" } else { "rejected" },
        )
        .unwrap();
        return;
    }
    s.admit_observed(&key, claim(), |stage, c| {
        if matches!(stage, Stage::BeforeWrite) {
            c.execute_batch("PRAGMA cache_size=1").unwrap();
        }
        if matches!(
            (mode.as_str(), stage),
            ("before", Stage::BeforeWrite)
                | ("during", Stage::BeforeCommit)
                | ("after", Stage::AfterCommit)
        ) {
            fs::write(root.join("ready"), b"boundary").unwrap();
            let start = Instant::now();
            while !root.join("continue").exists() {
                assert!(start.elapsed() < Duration::from_secs(30));
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    })
    .unwrap();
}
#[test]
fn separate_process_reconstructs_unknown_without_fabricating_completion() {
    let f = Temp::new();
    let s = create(&f);
    s.admit(&key(), claim()).unwrap();
    let k = s.load(&key(), Access::ReadOnly).unwrap();
    s.admit(
        &key(),
        operation(
            &k,
            "unknown",
            Input::Attempt {
                id: AttemptId::new("A-attempt").unwrap(),
                generation: rev(1),
                status: AttemptStatus::Unknown,
                basis: source("interrupted"),
            },
        ),
    )
    .unwrap();
    wait_child(child(&f, "reopen"));
}
#[test]
fn simultaneous_process_claims_admit_exactly_one_current_owner() {
    let f = Temp::new();
    let s = create(&f);
    let mut a = child(&f, "race-one");
    let mut b = child(&f, "race-two");
    wait_file(&f.0.join("ready-one"), &mut a);
    wait_file(&f.0.join("ready-two"), &mut b);
    fs::write(f.0.join("go"), b"go").unwrap();
    wait_child(a);
    wait_child(b);
    let results = [
        fs::read_to_string(f.0.join("result-one")).unwrap(),
        fs::read_to_string(f.0.join("result-two")).unwrap(),
    ];
    assert_eq!(
        results.iter().filter(|r| r.as_str() == "committed").count(),
        1
    );
    let k = s.load(&key(), Access::ReadOnly).unwrap();
    assert_eq!(k.revision(), 1);
    assert_eq!(k.state().attempts().len(), 1);
    s.admit(
        &key(),
        operation(
            &k,
            "independent",
            Input::Claim(attempt("independent", "D", "operator")),
        ),
    )
    .unwrap();
    assert_eq!(
        s.load(&key(), Access::ReadOnly)
            .unwrap()
            .state()
            .attempts()
            .len(),
        2
    );
}
#[test]
fn process_kill_at_write_and_commit_cuts_reconciles_exact_presence_or_absence() {
    for mode in ["before", "during", "after"] {
        let f = Temp::new();
        let s = create(&f);
        let mut c = child(&f, mode);
        wait_file(&f.0.join("ready"), &mut c);
        if mode == "during" {
            assert!(
                fs::metadata(f.store().join("program.db-journal"))
                    .unwrap()
                    .len()
                    > 0
            );
        }
        c.kill().unwrap();
        let output = c.wait_with_output().unwrap();
        assert!(!output.status.success());
        if s.location.journal_exists().unwrap() {
            let before = snapshot(&f);
            assert!(
                matches!(s.load(&key(),Access::ReadOnly),Err(ProgramStoreError::Storage(e)) if e.kind()==ErrorKind::RecoveryRequired)
            );
            assert_eq!(snapshot(&f), before);
        }
        let found = s
            .reconcile(&key(), &definition(), &claim(), Access::Recover)
            .unwrap();
        if mode == "after" {
            assert!(matches!(found, Reconciliation::Present(_)));
            assert!(s.admit(&key(), claim()).unwrap().duplicate);
        } else {
            assert_eq!(found, Reconciliation::Absent);
            s.admit(&key(), claim()).unwrap();
        }
        assert_eq!(s.load(&key(), Access::ReadOnly).unwrap().revision(), 1);
    }
}

#[test]
fn injected_native_write_and_sync_failures_retain_completion_and_exact_reconciliation() {
    let build = Temp::new();
    let library = build.0.join("fault.so");
    let output = Command::new("cc")
        .args([
            "-shared", "-fPIC", "-O2", "-Wall", "-Wextra", "-Werror", "-o",
        ])
        .arg(&library)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/storage_fault.c"))
        .arg("-ldl")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "fault interposer build: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    for (op, file) in [
        ("write", "program.db-journal"),
        ("sync", "program.db-journal"),
        ("write", "program.db"),
        ("sync", "program.db"),
    ] {
        let f = Temp::new();
        let s = create(&f);
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "program_store::tests::process_child",
                "--ignored",
                "--nocapture",
            ])
            .env("CORTEX_PROGRAM_TEST_ROOT", &f.0)
            .env("CORTEX_PROGRAM_TEST_MODE", "fault")
            .env("LD_PRELOAD", &library)
            .env("CORTEX_FAULT_TARGET", f.store().join(file))
            .env("CORTEX_FAULT_OP", op)
            .env("CORTEX_FAULT_NTH", "1")
            .env("CORTEX_FAULT_ARM", f.0.join("armed"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(&[0x57; 32]).unwrap();
        let start = Instant::now();
        while child.try_wait().unwrap().is_none() {
            if start.elapsed() > Duration::from_secs(30) {
                child.kill().unwrap();
                panic!("fault child deadline");
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        let result = child.wait_with_output().unwrap();
        let diagnostic = String::from_utf8_lossy(&result.stderr);
        assert!(result.status.success(), "fault child: {diagnostic}");
        assert!(
            diagnostic.contains(&format!("cortex-fault {op} 1 1 ")),
            "injection was not reached: {diagnostic}"
        );
        assert!(
            fs::read_to_string(f.0.join("fault-result"))
                .unwrap()
                .contains("Absent")
        );
        assert_eq!(s.load(&key(), Access::ReadOnly).unwrap().revision(), 0);
    }
}
#[test]
fn pending_removal_resumes_only_with_exact_store_incarnation() {
    let f = Temp::new();
    let s = create(&f);
    let id = s.id();
    s.location.write_intent(id).unwrap();
    assert!(
        matches!(s.load(&key(),Access::ReadOnly),Err(ProgramStoreError::Storage(e)) if e.kind()==ErrorKind::RemovalPending)
    );
    assert!(
        ProgramStore::resume_removal(
            &f.store(),
            &key(),
            RemovalRequest::whole_store(StoreId::from_bytes([1; 16])),
            limits()
        )
        .is_err()
    );
    s.location.unlink("program.db").unwrap();
    ProgramStore::resume_removal(
        &f.store(),
        &key(),
        RemovalRequest::whole_store(id),
        limits(),
    )
    .unwrap();
    assert!(!f.store().join(".removing").exists());
    assert!(!f.store().join("program.db").exists());
}

#[test]
fn plans_packages_handoffs_and_explicit_invalidations_replay_without_loss() {
    use cortex_core::program::context::*;
    let f = Temp::new();
    let mut k = initialized();
    let r = prepare_release(&mut k, "AB", true);
    admit(&mut k, "release", Input::Release(Box::new(r.clone())));
    admit(
        &mut k,
        "plan",
        Input::Plan(Plan {
            revision: rev(1),
            order: ["D", "E", "A", "B", "C", "F"]
                .into_iter()
                .map(task)
                .collect(),
            basis: source("owner-order"),
        }),
    );
    let package = PackageRequest {
        id: PackageId::new("package-A").unwrap(),
        revision: rev(1),
        mission: task("A"),
        baseline: rev(1),
        generation: rev(1),
        sources: vec![SourceInput {
            source: source("source-v1"),
            candidate: binding("A", "a", 1).candidate().clone(),
            text: "PRIVATE_PACKAGE_SENTINEL_123456789".into(),
        }],
        methods: vec![MethodProposal {
            method: source("native.candidate-correspondence"),
            applicability: "Exact mission candidate".into(),
            qualification: None,
            exceptions: "None".into(),
        }],
        omissions: "Explicit source only; no live recapture".into(),
        deliverable: "Inspectable contract proposal".into(),
        stop_rules: "Stop at missing data or unsupported operations".into(),
    };
    admit(&mut k, "package", Input::Package(package));
    admit(
        &mut k,
        "claim",
        Input::Claim(attempt("original", "A", "operator")),
    );
    admit(
        &mut k,
        "handoff",
        Input::Handoff {
            old: AttemptId::new("original").unwrap(),
            recipient: grant(1, "second"),
            next: attempt("next", "A", "second"),
        },
    );
    admit(
        &mut k,
        "explicit-invalidation",
        Input::InvalidateEvidence {
            observation: r.bindings[0].evidence().observation().clone(),
            reason: "Newly observed verifier defect".into(),
        },
    );
    let s = store_kernel(&f, &k);
    let loaded = s.load(&key(), Access::ReadOnly).unwrap();
    assert!(loaded.packages() == k.packages());
    assert!(loaded.state().plan() == k.state().plan());
    assert!(loaded.state().attempts() == k.state().attempts());
    assert!(loaded.records() == k.records());
    assert_eq!(
        loaded.state().attempts()[0].status,
        AttemptStatus::HandedOver
    );
    assert!(
        !loaded
            .state()
            .readiness()
            .mission(&task("B"))
            .unwrap()
            .eligible()
    );
    let bytes = fs::read(f.store().join("program.db")).unwrap();
    assert!(
        !bytes
            .windows(b"PRIVATE_PACKAGE_SENTINEL_123456789".len())
            .any(|b| b == b"PRIVATE_PACKAGE_SENTINEL_123456789")
    );
}
#[test]
fn failed_skipped_and_unknown_original_evidence_survives_storage() {
    use cortex_core::binding::ObservationId;
    use cortex_evidence::{ExecutionObservation, unified::*};
    let f = Temp::new();
    let mut k = CoordinationKernel::new(definition()).unwrap();
    let c = binding("verify-fixture", "a", 1);
    let kinds = vec![
        ObservationKind::Execution(
            VerificationOutcome::new(
                ExecutionObservation::Exited { code: 1 },
                1,
                Some(
                    OracleEvaluation::new(OracleConclusion::Violated, "Observed violation")
                        .unwrap(),
                ),
            )
            .unwrap(),
        ),
        ObservationKind::Execution(
            VerificationOutcome::new(ExecutionObservation::NotRun, 0, None).unwrap(),
        ),
        ObservationKind::Execution(
            VerificationOutcome::new(ExecutionObservation::Cancelled, 0, None).unwrap(),
        ),
        ObservationKind::Execution(
            VerificationOutcome::new(ExecutionObservation::TimedOut, 0, None).unwrap(),
        ),
        ObservationKind::Execution(
            VerificationOutcome::new(ExecutionObservation::Unavailable, 0, None).unwrap(),
        ),
        ObservationKind::ScopeInspection(ScopeFinding::Absent),
        ObservationKind::Inference,
    ];
    for (i, kind) in kinds.into_iter().enumerate() {
        let report = ReportedObservation::new(
            ObservationId::new("outcomes", i as u64 + 1).unwrap(),
            capture(&c, 1),
            profile(),
            ObservationTimes::new(std::time::UNIX_EPOCH, std::time::UNIX_EPOCH).unwrap(),
            kind,
            "Retain original disposition",
        )
        .unwrap();
        admit(&mut k, &format!("outcome-{i}"), Input::Observe(report));
    }
    let s = store_kernel(&f, &k);
    let loaded = s.load(&key(), Access::ReadOnly).unwrap();
    assert!(loaded.records() == k.records());
    assert!(loaded.records().iter().all(|r| r.assessment().is_none()));
}

mod operational;
mod supervised;
