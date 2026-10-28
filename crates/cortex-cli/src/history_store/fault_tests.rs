//! CAP-MEM-001/CAP-SEC-002: child-local native call faults, not power-loss tests.
use super::*;

struct Harness {
    fixture: Fixture,
    library: PathBuf,
}
impl Harness {
    fn new() -> Self {
        let fixture = Fixture::new();
        let library = fixture.0.join("fault.so");
        let output = Command::new("cc")
            .args([
                "-shared", "-fPIC", "-std=c11", "-Wall", "-Wextra", "-Werror",
            ])
            .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/storage_fault.c"))
            .args(["-o"])
            .arg(&library)
            .arg("-ldl")
            .output()
            .unwrap();
        assert!(output.status.success(), "{:?}", output.stderr);
        Self { fixture, library }
    }
    fn run(&self, mode: &str, target: &Path, op: &str, nth: u32, limit: Option<u64>) -> String {
        let _ = fs::remove_file(self.fixture.0.join("armed"));
        let exe = std::env::current_exe().unwrap();
        let mut cmd = if let Some(bytes) = limit {
            // Literal shell program, structured argv, synthetic fixture only. The
            // child ignores SIGXFSZ so RLIMIT_FSIZE yields a real kernel EFBIG.
            let mut c = Command::new("/usr/bin/bash");
            c.args([
                "-c",
                "trap '' XFSZ; exec \"$@\"",
                "cortex-fault-test",
                "/usr/bin/prlimit",
            ])
            .arg(format!("--fsize={bytes}:{bytes}"))
            .arg("--")
            .arg(&exe);
            c
        } else {
            Command::new(&exe)
        };
        let mut child = cmd
            .args([
                "--exact",
                "history_store::tests::faults::fault_child",
                "--ignored",
                "--nocapture",
            ])
            .env("LD_PRELOAD", &self.library)
            .env("CORTEX_FAULT_CONTROL", &self.fixture.0)
            .env("CORTEX_FAULT_MODE", mode)
            .env("CORTEX_FAULT_TARGET", target)
            .env("CORTEX_FAULT_OP", op)
            .env("CORTEX_FAULT_NTH", nth.to_string())
            .env("CORTEX_FAULT_ARM", self.fixture.0.join("armed"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(&[0x41; 32]).unwrap();
        let start = Instant::now();
        while child.try_wait().unwrap().is_none() {
            if start.elapsed() > Duration::from_secs(15) {
                child.kill().unwrap();
                let _ = child.wait();
                panic!("fault child exceeded its deadline");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let output = child.wait_with_output().unwrap();
        let success = output.status.success();
        let text = format!(
            "{}{}",
            String::from_utf8(output.stdout).unwrap(),
            String::from_utf8(output.stderr).unwrap()
        );
        assert!(!text.contains(SECRET));
        assert!(!text.contains(&"41".repeat(32)));
        assert!(!text.contains(&"A".repeat(32)));
        assert!(success, "fault child failed: {text}");
        text
    }
}

#[test]
#[ignore = "isolated fault child; parent tests execute this with --ignored and an stdin key"]
fn fault_child() {
    let Some(root) = std::env::var_os("CORTEX_FAULT_CONTROL") else {
        return;
    };
    let root = PathBuf::from(root);
    let path = root.join("store");
    let mode = std::env::var("CORTEX_FAULT_MODE").unwrap();
    let mut bytes = [0; 32];
    std::io::stdin().read_exact(&mut bytes).unwrap();
    let key = StoreKey::new(&bytes).unwrap();
    let arm = || {
        fs::File::create(root.join("armed")).unwrap();
    };
    match mode.as_str() {
        "create" => {
            arm();
            match HistoryStore::create(&path, &key, context()) {
                Ok(_) => println!("RESULT CREATED"),
                Err(e) => println!("RESULT {:?}", e),
            }
        }
        "recover" => {
            arm();
            match HistoryStore::open(&path, &key, Access::Recover) {
                Ok(s) => println!(
                    "RESULT HISTORY {}",
                    s.load(&key, Access::Recover).unwrap().records().len()
                ),
                Err(e) => println!("RESULT {:?}", e),
            }
        }
        "prepare" => {
            let s = HistoryStore::open(&path, &key, Access::Recover).unwrap();
            let text = "x".repeat(4096);
            let large = CanonicalRecord::correction(
                RecordId::new("requirement").unwrap(),
                RecordVersionId::new("b").unwrap(),
                RecordKind::Requirement,
                context().id().clone(),
                RecordVersionId::new("a").unwrap(),
                RecordContent::new(
                    SECRET,
                    vec![SourceReference::operator_reported(&text, &text, &text).unwrap(); 32],
                )
                .unwrap(),
                RecordTime::new(DomainInterval::new(at(2), Some(at(4))).unwrap(), at(1)).unwrap(),
            );
            s.admit_observed(&key, large, |stage, c| match stage {
                Stage::BeforeWrite => {
                    c.execute_batch("PRAGMA cache_size=1; PRAGMA cache_spill=1;")
                        .unwrap();
                }
                // Exit without Rust destructors/transaction rollback. The parent
                // separately asserts main-page changes and a hot journal header.
                Stage::BeforeCommit => std::process::exit(0),
                _ => {}
            })
            .unwrap();
            panic!("preparation did not reach the interruption boundary");
        }
        "admit" | "remove" => {
            let s = HistoryStore::open(&path, &key, Access::Recover).unwrap();
            arm();
            if mode == "admit" {
                match s.admit(&key, entry("b", Some("a"), 2, Some(4), 1)) {
                    Ok(r) => println!("RESULT COMMITTED {}", r.ordinal()),
                    Err(e) => println!("RESULT {:?}", e),
                }
            } else {
                match s.remove(&key, Some(RemovalRequest::whole_collection(s.id()))) {
                    Ok(_) => println!("RESULT REMOVED"),
                    Err(e) => println!("RESULT {:?}", e),
                }
            }
        }
        _ => panic!("unknown fixed fault case"),
    }
}

#[test]
fn recovery_io_failure_is_not_a_wrong_key_and_preserves_confirmed_history() {
    for op in ["write", "sync"] {
        let h = Harness::new();
        let s = create(&h.fixture);
        s.admit(&key(), entry("a", None, 0, None, 0)).unwrap();
        let original_db = fs::read(h.fixture.store().join(DATABASE)).unwrap();
        h.run(
            "prepare",
            &h.fixture.store().join(DATABASE),
            "write",
            0,
            None,
        );
        assert_ne!(
            fs::read(h.fixture.store().join(DATABASE)).unwrap(),
            original_db
        );
        let journal = fs::read(h.fixture.store().join(JOURNAL)).unwrap();
        assert_eq!(
            &journal[..8],
            &[0xd9, 0xd5, 0x05, 0xf9, 0x20, 0xa1, 0x63, 0xd7]
        );
        no_plaintext(&h.fixture);
        let before = snapshot(&h.fixture);
        assert_eq!(
            s.load(&key(), Access::ReadOnly).unwrap_err().kind(),
            ErrorKind::RecoveryRequired
        );
        assert_eq!(snapshot(&h.fixture), before);
        let result = h.run("recover", &h.fixture.store().join(DATABASE), op, 1, None);
        eprintln!("RECOVERY {op}: {result}");
        assert!(
            result.contains(&format!("cortex-fault {op} 1 1")),
            "{result}"
        );
        assert!(
            result.contains("kind: Io"),
            "native recovery I/O was misclassified: {result}"
        );
        if s.location.journal_exists().unwrap() {
            let before = snapshot(&h.fixture);
            assert_eq!(
                s.load(&key(), Access::ReadOnly).unwrap_err().kind(),
                ErrorKind::RecoveryRequired
            );
            assert_eq!(snapshot(&h.fixture), before);
        }
        no_plaintext(&h.fixture);
        validate_view(&s, false);
        no_plaintext(&h.fixture);
    }
}

fn injected(text: &str, op: &str, nth: u32) {
    assert!(
        text.contains(&format!("cortex-fault {op} {nth} 1 ")),
        "fault was not reached: {text}"
    );
}
fn native_short_write(text: &str) {
    let events: Vec<_> = text
        .lines()
        .filter(|l| l.starts_with("cortex-fault write "))
        .map(|l| l.split_whitespace().collect::<Vec<_>>())
        .collect();
    assert!(
        events
            .iter()
            .any(|v| v[3] == "0" && v[5] == "-1" && v[6] == "27"),
        "no kernel EFBIG: {text}"
    );
    assert!(
        events.iter().any(|v| {
            let requested = v[4].parse::<isize>().unwrap();
            let returned = v[5].parse::<isize>().unwrap();
            v[3] == "0" && returned > 0 && returned < requested
        }),
        "no actual short write: {text}"
    );
}
fn validate_view(store: &HistoryStore, present: bool) {
    let h = store.load(&key(), Access::Recover).unwrap();
    assert_eq!(h.context(), &context());
    let mut expected = vec![entry("a", None, 0, None, 0)];
    if present {
        expected.push(entry("b", Some("a"), 2, Some(4), 1));
    }
    assert_eq!(h.records(), expected);
    for (domain, cutoff, want) in [
        (3, 0, "a"),
        (3, 1, if present { "b" } else { "a" }),
        (4, 1, "a"),
    ] {
        let q = TemporalQuery::new(context().id().clone(), at(domain), at(cutoff)).unwrap();
        assert_eq!(
            h.query(&RecordId::new("requirement").unwrap(), &q)
                .unwrap()
                .record()
                .unwrap()
                .id()
                .as_str(),
            want
        );
    }
}

#[test]
fn native_sync_call_order_is_calibrated_for_the_qualified_stack() {
    for (mode, count) in [("create", 4), ("admit", 2), ("remove", 3)] {
        let h = Harness::new();
        if mode != "create" {
            let s = create(&h.fixture);
            s.admit(&key(), entry("a", None, 0, None, 0)).unwrap();
        }
        let result = h.run(mode, &h.fixture.store(), "sync", 0, None);
        assert_eq!(
            result
                .lines()
                .filter(|l| l.starts_with("cortex-fault sync "))
                .count(),
            count,
            "{result}"
        );
        assert!(
            !result.contains("RESULT StoreError") && !result.contains("RESULT AdmissionFailure"),
            "{result}"
        );
        eprintln!("CALIBRATE {mode}: {result}");
    }
}

#[test]
fn creation_io_failures_never_replace_incomplete_or_uncertain_stores() {
    // Usable = complete empty collection can be validated after the lost reply.
    for (file, nth, limit, kind, usable) in [
        (".lock", 1, None, Some(ErrorKind::Io), false),
        (DATABASE, 1, None, Some(ErrorKind::Io), false),
        (JOURNAL, 1, None, Some(ErrorKind::Indeterminate), false),
        (JOURNAL, 0, Some(64), Some(ErrorKind::Io), false),
        ("", 1, None, Some(ErrorKind::Io), false),
        // SQLite intentionally ignores journal-creation directory-sync errors.
        // Later commit/final directory syncs succeed. This is not proof that an
        // arbitrary sync failure is surfaced or that power loss is safe.
        ("", 2, None, None, true),
        ("", 3, None, Some(ErrorKind::Indeterminate), true),
        ("", 4, None, Some(ErrorKind::Indeterminate), true),
    ] {
        let h = Harness::new();
        let target = if file.is_empty() {
            h.fixture.store()
        } else {
            h.fixture.store().join(file)
        };
        let op = if limit.is_some() { "write" } else { "sync" };
        let result = h.run("create", &target, op, nth, limit);
        eprintln!("CREATE {file} {nth}: {result}");
        if limit.is_some() {
            native_short_write(&result);
        } else {
            injected(&result, op, nth);
        }
        if let Some(kind) = kind {
            assert!(result.contains(&format!("kind: {kind:?},")), "{result}");
        } else {
            assert!(result.contains("RESULT CREATED"), "{result}");
            assert!(result.contains("cortex-fault sync 4 0 0 0 0"), "{result}");
        }
        let opened = HistoryStore::open(&h.fixture.store(), &key(), Access::Recover);
        if usable {
            let history = opened.unwrap().load(&key(), Access::Recover).unwrap();
            assert_eq!(history.context(), &context());
            assert!(history.records().is_empty());
        } else {
            assert!(opened.is_err(), "incomplete create became usable");
        }
        let before = snapshot(&h.fixture);
        assert_eq!(
            HistoryStore::create(&h.fixture.store(), &key(), context())
                .unwrap_err()
                .kind(),
            ErrorKind::AlreadyExists
        );
        assert_eq!(snapshot(&h.fixture), before);
        no_plaintext(&h.fixture);
    }
}

#[test]
fn admission_native_write_and_injected_sync_errors_reconcile_without_duplicates() {
    for (file, op, nth, limit, completion, present) in [
        (
            JOURNAL,
            "write",
            0,
            Some(64),
            Some(Completion::RolledBack),
            false,
        ),
        (
            JOURNAL,
            "sync",
            1,
            None,
            Some(Completion::Indeterminate),
            false,
        ),
        (
            DATABASE,
            "write",
            1,
            None,
            Some(Completion::Indeterminate),
            false,
        ),
        (
            DATABASE,
            "sync",
            1,
            None,
            Some(Completion::Indeterminate),
            false,
        ),
        ("", "sync", 1, None, None, true),
        ("", "sync", 2, None, Some(Completion::Indeterminate), true),
    ] {
        let h = Harness::new();
        let s = create(&h.fixture);
        s.admit(&key(), entry("a", None, 0, None, 0)).unwrap();
        let target = if file.is_empty() {
            h.fixture.store()
        } else {
            h.fixture.store().join(file)
        };
        let result = h.run("admit", &target, op, nth, limit);
        eprintln!("ADMIT {file} {op} {nth}: {result}");
        if limit.is_some() {
            native_short_write(&result);
        } else {
            injected(&result, op, nth);
        }
        if let Some(completion) = completion {
            assert!(
                result.contains(&format!("completion: {completion:?},")),
                "{result}"
            );
            assert!(result.contains("kind: Io,"), "{result}");
        } else {
            assert!(result.contains("RESULT COMMITTED 2"), "{result}");
            assert!(result.contains("cortex-fault sync 2 0 0 0 0"), "{result}");
        }
        validate_view(&s, present);
        let b = entry("b", Some("a"), 2, Some(4), 1);
        let found = s
            .reconcile(&key(), &context(), &b, Access::Recover)
            .unwrap();
        if present {
            assert!(
                matches!(found,Reconciliation::Present(r) if r.ordinal()==2 && r.store()==s.id())
            );
        } else {
            assert_eq!(found, Reconciliation::Absent);
            assert_eq!(s.admit(&key(), b.clone()).unwrap().ordinal(), 2);
        }
        let rejected = s.admit(&key(), b).unwrap_err();
        assert_eq!(rejected.completion(), Completion::Rejected);
        validate_view(&s, true);
        no_plaintext(&h.fixture);
    }
}

#[test]
fn removal_io_failures_with_complete_intent_resume_only_under_matching_scope() {
    for (file, op, nth) in [
        (REMOVING, "sync", 1),
        ("", "sync", 1),
        (DATABASE, "unlink", 1),
        ("", "sync", 2),
        (REMOVING, "unlink", 1),
    ] {
        let h = Harness::new();
        let s = create(&h.fixture);
        s.admit(&key(), entry("a", None, 0, None, 0)).unwrap();
        let target = if file.is_empty() {
            h.fixture.store()
        } else {
            h.fixture.store().join(file)
        };
        let result = h.run("remove", &target, op, nth, None);
        eprintln!("REMOVE {file} {op} {nth}: {result}");
        injected(&result, op, nth);
        assert!(result.contains("kind: RemovalIncomplete,"), "{result}");
        let before = snapshot(&h.fixture);
        assert_eq!(
            HistoryStore::open(&h.fixture.store(), &key(), Access::ReadOnly)
                .unwrap_err()
                .kind(),
            ErrorKind::RemovalPending
        );
        assert_eq!(
            HistoryStore::resume_removal(
                &h.fixture.store(),
                &key(),
                RemovalRequest::whole_collection(StoreId([9; 16]))
            )
            .unwrap_err()
            .kind(),
            ErrorKind::AuthorizationRequired
        );
        assert_eq!(snapshot(&h.fixture), before);
        no_plaintext(&h.fixture);
        let receipt = HistoryStore::resume_removal(
            &h.fixture.store(),
            &key(),
            RemovalRequest::whole_collection(s.id()),
        )
        .unwrap();
        assert_eq!(receipt.store(), s.id());
        assert_eq!(snapshot(&h.fixture).len(), 1);
        assert_eq!(
            s.load(&key(), Access::Recover).unwrap_err().kind(),
            ErrorKind::Missing
        );
    }
}

#[test]
fn damaged_removal_intent_and_failed_final_sync_do_not_fabricate_completion() {
    for (file, op, nth, limit, partial) in [
        (REMOVING, "write", 1, None, true),
        (REMOVING, "write", 0, Some(8), true),
        ("", "sync", 3, None, false),
    ] {
        let h = Harness::new();
        let s = create(&h.fixture);
        s.admit(&key(), entry("a", None, 0, None, 0)).unwrap();
        let target = if file.is_empty() {
            h.fixture.store()
        } else {
            h.fixture.store().join(file)
        };
        let result = h.run("remove", &target, op, nth, limit);
        eprintln!("UNRESOLVED REMOVE {file} {op} {nth}: {result}");
        if limit.is_some() {
            native_short_write(&result);
        } else {
            injected(&result, op, nth);
        }
        assert!(result.contains("kind: RemovalIncomplete,"), "{result}");
        let before = snapshot(&h.fixture);
        let resumed = HistoryStore::resume_removal(
            &h.fixture.store(),
            &key(),
            RemovalRequest::whole_collection(s.id()),
        )
        .unwrap_err();
        if partial {
            assert_eq!(resumed.kind(), ErrorKind::RemovalIncomplete);
            assert!(
                fs::metadata(h.fixture.store().join(REMOVING))
                    .unwrap()
                    .len()
                    < 16
            );
            // Inspect through the private test connection only: public views stay
            // blocked. A damaged marker does not authorize silent repair or erasure.
            let conn = connect(&s.location, &key(), Access::ReadOnly).unwrap();
            assert_eq!(
                load(&conn).unwrap().1.records(),
                &[entry("a", None, 0, None, 0)]
            );
        } else {
            assert_eq!(resumed.kind(), ErrorKind::AuthorizationRequired);
            assert_eq!(snapshot(&h.fixture).len(), 1);
            assert_eq!(
                s.load(&key(), Access::Recover).unwrap_err().kind(),
                ErrorKind::Missing
            );
        }
        assert_eq!(snapshot(&h.fixture), before);
        no_plaintext(&h.fixture);
    }
}
