use super::*;
use cortex_core::{CandidateId, TaskContract, TaskId, binding::*};
use std::{
    ffi::OsString,
    io::{Read, Write},
    os::unix::{
        ffi::OsStringExt,
        fs::{MetadataExt, PermissionsExt},
    },
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

const SECRET: &str = "CONFIDENTIAL_HISTORY_SENTINEL_0123456789";
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "cortex-durable-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
    fn store(&self) -> PathBuf {
        self.0.join("store")
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn key() -> StoreKey {
    StoreKey::new(&[0x41; 32]).unwrap()
}
fn at(n: u64) -> SystemTime {
    UNIX_EPOCH + Duration::new(n, 123)
}
fn context() -> CandidateObservation {
    let root = std::env::temp_dir().join(OsString::from_vec(b"cortex-source-\xff".to_vec()));
    let r = RepositoryIdentity::new(root.clone(), root.join(".git"), root.join(".git")).unwrap();
    let commit = CandidateId::from_git_commit(&"a".repeat(40)).unwrap();
    let c = CandidateBindingContract::new(
        TaskContract::new(
            TaskId::new("storage.fixture").unwrap(),
            SECRET,
            "Retain exact temporal history",
        )
        .unwrap(),
        ContractRevision::new(u64::MAX).unwrap(),
        CandidateRequirement::new(r.clone(), commit.clone(), WorktreeRequirement::AllowChanges),
    );
    CandidateObservation::new(
        ObservationId::new("captures", u64::MAX).unwrap(),
        &c,
        RepositoryState::new(
            Some(r),
            Head::Attached {
                branch: b"branch-\xff".to_vec(),
                commit,
            },
            vec![
                Entry::Tracked {
                    path: b"file-\xff".to_vec(),
                    staged: Some(Change::Added),
                    unstaged: Some(Change::Deleted),
                },
                Entry::Untracked {
                    path: b"new".to_vec(),
                },
            ],
            ObservationCoverage::GitStatus,
            ObservationConsistency::TwoMatchingSamples,
        )
        .unwrap(),
        Some(ComparisonBasis::new("fixture mechanism", SECRET.as_bytes().to_vec()).unwrap()),
    )
}
fn entry(
    id: &str,
    predecessor: Option<&str>,
    from: u64,
    until: Option<u64>,
    recorded: u64,
) -> CanonicalRecord {
    let content = RecordContent::new(
        SECRET,
        vec![
            SourceReference::operator_reported("fixture operator", "explicit submission", "1")
                .unwrap(),
            SourceReference::observation_reference(
                ObservationId::new("observed", u64::MAX).unwrap(),
                "source observation",
                "2",
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let time = RecordTime::new(
        DomainInterval::new(at(from), until.map(at)).unwrap(),
        at(recorded),
    )
    .unwrap();
    let e = RecordId::new("requirement").unwrap();
    let v = RecordVersionId::new(id).unwrap();
    let c = context().id().clone();
    match predecessor {
        Some(p) => CanonicalRecord::correction(
            e,
            v,
            RecordKind::Requirement,
            c,
            RecordVersionId::new(p).unwrap(),
            content,
            time,
        ),
        None => CanonicalRecord::initial(e, v, RecordKind::Requirement, c, content, time),
    }
}
fn create(f: &Fixture) -> HistoryStore {
    HistoryStore::create(&f.store(), &key(), context()).unwrap()
}
fn snapshot(f: &Fixture) -> Vec<(String, Vec<u8>, u64)> {
    let mut s = fs::read_dir(f.store())
        .unwrap()
        .map(|e| {
            let p = e.unwrap().path();
            let m = fs::metadata(&p).unwrap();
            (
                p.file_name().unwrap().to_string_lossy().into_owned(),
                fs::read(&p).unwrap(),
                m.mtime() as u64,
            )
        })
        .collect::<Vec<_>>();
    s.sort_by(|a, b| a.0.cmp(&b.0));
    s
}
fn no_plaintext(f: &Fixture) {
    let hex = "41".repeat(32);
    for (_, bytes, _) in snapshot(f) {
        for sentinel in [SECRET.as_bytes(), &[0x41; 32], hex.as_bytes()] {
            assert!(!bytes.windows(sentinel.len()).any(|b| b == sentinel));
        }
    }
}

#[test]
fn encrypted_roundtrip_preserves_every_field_order_and_both_time_axes() {
    let f = Fixture::new();
    let store = create(&f);
    let a = entry("z", None, 0, None, 10);
    let b = entry("a", Some("z"), 2, Some(4), 20);
    assert_eq!(store.admit(&key(), a.clone()).unwrap().ordinal(), 1);
    store.admit(&key(), b.clone()).unwrap();
    let read = HistoryStore::open(&f.store(), &key(), Access::ReadOnly).unwrap();
    let h = read.load(&key(), Access::ReadOnly).unwrap();
    assert_eq!(h.context(), &context());
    assert_eq!(h.records(), &[a, b]);
    for (domain, cutoff, want) in [(1, 30, "z"), (2, 19, "z"), (2, 20, "a"), (4, 30, "z")] {
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
    no_plaintext(&f);
    assert!(
        !fs::read(f.store().join(DATABASE))
            .unwrap()
            .starts_with(b"SQLite format 3")
    );
}
#[test]
fn key_rejections_never_change_or_recreate_store() {
    assert_eq!(
        StoreKey::new(&[]).unwrap_err().kind(),
        ErrorKind::KeyRequired
    );
    assert_eq!(
        StoreKey::new(&[1; 31]).unwrap_err().kind(),
        ErrorKind::InvalidKey
    );
    let f = Fixture::new();
    let _store = create(&f);
    let before = snapshot(&f);
    let wrong = StoreKey::new(&[0x42; 32]).unwrap();
    let error = HistoryStore::open(&f.store(), &wrong, Access::Recover).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::CannotUnlock);
    assert!(!format!("{error:?} {wrong:?}").contains(SECRET));
    assert_eq!(snapshot(&f), before);
    assert!(HistoryStore::open(&f.store(), &key(), Access::ReadOnly).is_ok());
}
#[test]
fn duplicate_invalid_reference_and_exact_reconciliation() {
    let f = Fixture::new();
    let s = create(&f);
    let a = entry("a", None, 0, None, 0);
    s.admit(&key(), a.clone()).unwrap();
    let before = snapshot(&f);
    let err = s.admit(&key(), a.clone()).unwrap_err();
    assert_eq!(err.completion(), Completion::Rejected);
    assert_eq!(err.error().kind(), ErrorKind::Domain);
    assert_eq!(snapshot(&f), before);
    assert!(matches!(
        s.reconcile(&key(), &context(), &a, Access::ReadOnly)
            .unwrap(),
        Reconciliation::Present(_)
    ));
    let bad = entry("b", Some("missing"), 0, None, 1);
    assert_eq!(
        s.admit(&key(), bad.clone()).unwrap_err().error().kind(),
        ErrorKind::Domain
    );
    assert_eq!(snapshot(&f), before);
    assert_eq!(
        s.reconcile(&key(), &context(), &bad, Access::ReadOnly)
            .unwrap(),
        Reconciliation::Absent
    );
    let alias = entry("a", None, 1, None, 0);
    assert_eq!(
        s.reconcile(&key(), &context(), &alias, Access::ReadOnly)
            .unwrap(),
        Reconciliation::Conflict
    );
    let conn = connect(&s.location, &key(), Access::Recover).unwrap();
    conn.execute("UPDATE meta SET instance=?1", [&[9u8; 16][..]])
        .unwrap();
    drop(conn);
    assert_eq!(
        s.load(&key(), Access::Recover).unwrap_err().kind(),
        ErrorKind::TargetChanged
    );
}
#[test]
fn unsupported_version_corrupt_payload_and_ordinal_are_not_empty_history() {
    for mutation in [
        "UPDATE meta SET format=2",
        "UPDATE records SET payload=x'7b7d'",
        "UPDATE records SET ordinal=4",
        "DROP TABLE records",
    ] {
        let f = Fixture::new();
        let s = create(&f);
        s.admit(&key(), entry("a", None, 0, None, 0)).unwrap();
        let c = connect(&s.location, &key(), Access::Recover).unwrap();
        c.execute_batch(mutation).unwrap();
        drop(c);
        let before = snapshot(&f);
        let error = s.load(&key(), Access::ReadOnly).unwrap_err();
        assert_eq!(
            error.kind(),
            if mutation.contains("format=2") {
                ErrorKind::Unsupported
            } else {
                ErrorKind::Corrupt
            }
        );
        assert_eq!(snapshot(&f), before);
    }
}
#[test]
fn codec_rejects_unknown_fields_tags_limits_and_invalid_times() {
    let a = entry("a", None, 0, None, 0);
    let encoded = codec::encode_record(&a).unwrap();
    let original: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
    for (field, value) in [
        ("unknown", serde_json::json!(1)),
        ("kind", serde_json::json!(50)),
        ("from", serde_json::json!({"seconds":0,"nanos":1000000000})),
        ("body", serde_json::json!("x".repeat(4097))),
        ("sources", serde_json::json!([])),
    ] {
        let mut data = original.clone();
        data[field] = value;
        assert!(codec::decode_record(&serde_json::to_vec(&data).unwrap()).is_err());
    }
    // The format retains u64 seconds rather than narrowing through an SQL integer.
    // This Linux SystemTime cannot represent that instant: reject, never wrap it.
    let mut unrepresentable = original.clone();
    unrepresentable["from"] = serde_json::json!({"seconds":u64::MAX,"nanos":0});
    assert_eq!(
        codec::decode_record(&serde_json::to_vec(&unrepresentable).unwrap())
            .unwrap_err()
            .kind(),
        ErrorKind::Unsupported
    );
    assert_eq!(
        codec::decode_context(&codec::encode_context(&context()).unwrap()).unwrap(),
        context()
    );
    let base = context();
    for head in [
        Head::Unborn {
            branch: b"orphan".to_vec(),
        },
        Head::Detached {
            commit: base.contract().candidate().commit().clone(),
        },
        Head::Unknown,
    ] {
        let c = CandidateObservation::new(
            base.id().clone(),
            base.contract(),
            RepositoryState::new(
                None,
                head,
                vec![],
                ObservationCoverage::Incomplete,
                ObservationConsistency::Indeterminate,
            )
            .unwrap(),
            None,
        );
        assert_eq!(
            codec::decode_context(&codec::encode_context(&c).unwrap()).unwrap(),
            c
        );
    }
}
#[test]
fn local_path_permissions_links_and_unknown_files_are_rejected() {
    let f = Fixture::new();
    let s = create(&f);
    let new = HistoryStore::create(&f.store(), &key(), context()).unwrap_err();
    assert_eq!(new.kind(), ErrorKind::AlreadyExists);
    fs::write(f.store().join("unrelated"), "must survive").unwrap();
    assert_eq!(
        s.remove(&key(), Some(RemovalRequest::whole_collection(s.id())))
            .unwrap_err()
            .kind(),
        ErrorKind::UnexpectedFiles
    );
    assert!(f.store().join("unrelated").exists());
    fs::remove_file(f.store().join("unrelated")).unwrap();
    fs::set_permissions(f.store().join(DATABASE), fs::Permissions::from_mode(0o000)).unwrap();
    assert_eq!(
        s.load(&key(), Access::ReadOnly).unwrap_err().kind(),
        ErrorKind::Permission
    );
    fs::set_permissions(f.store().join(DATABASE), fs::Permissions::from_mode(0o600)).unwrap();
    fs::rename(f.store().join(DATABASE), f.0.join("saved")).unwrap();
    std::os::unix::fs::symlink(f.0.join("saved"), f.store().join(DATABASE)).unwrap();
    assert_eq!(
        s.load(&key(), Access::ReadOnly).unwrap_err().kind(),
        ErrorKind::InvalidLocation
    );
}
#[test]
fn whole_removal_is_scoped_and_cannot_recreate_old_store() {
    let f = Fixture::new();
    let s = create(&f);
    s.admit(&key(), entry("a", None, 0, None, 0)).unwrap();
    assert_eq!(
        s.remove(&key(), None).unwrap_err().kind(),
        ErrorKind::AuthorizationRequired
    );
    assert_eq!(
        s.remove(
            &key(),
            Some(RemovalRequest::whole_collection(StoreId([9; 16])))
        )
        .unwrap_err()
        .kind(),
        ErrorKind::AuthorizationRequired
    );
    let before = snapshot(&f);
    assert_eq!(
        s.remove(
            &StoreKey::new(&[8; 32]).unwrap(),
            Some(RemovalRequest::whole_collection(s.id()))
        )
        .unwrap_err()
        .kind(),
        ErrorKind::CannotUnlock
    );
    assert_eq!(snapshot(&f), before);
    let receipt = s
        .remove(&key(), Some(RemovalRequest::whole_collection(s.id())))
        .unwrap();
    assert_eq!(receipt.store(), s.id());
    assert_eq!(
        snapshot(&f)
            .iter()
            .map(|r| r.0.as_str())
            .collect::<Vec<_>>(),
        [".lock"]
    );
    assert_eq!(
        HistoryStore::open(&f.store(), &key(), Access::Recover)
            .unwrap_err()
            .kind(),
        ErrorKind::Missing
    );
    assert!(HistoryStore::create(&f.store(), &key(), context()).is_err());
}

// A test-only child protocol. Key bytes travel over stdin, not argv/environment.
fn child(f: &Fixture, mode: &str) -> Child {
    let mut c = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "history_store::tests::process_child",
            "--ignored",
            "--nocapture",
        ])
        .env("CORTEX_TEST_STORE", f.store())
        .env("CORTEX_TEST_CONTROL", &f.0)
        .env("CORTEX_TEST_MODE", mode)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    c.stdin.take().unwrap().write_all(&[0x41; 32]).unwrap();
    c
}
fn wait_ready(f: &Fixture) {
    let start = Instant::now();
    while !f.0.join("ready").exists() {
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "child did not reach controlled boundary"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn stop(mut c: Child) {
    c.kill().unwrap();
    let output = c.wait_with_output().unwrap();
    assert!(!output.status.success());
    assert!(
        !output
            .stderr
            .windows(SECRET.len())
            .any(|b| b == SECRET.as_bytes())
    );
}
#[test]
#[ignore = "subprocess entry; parent tests execute this with --ignored and an stdin key"]
fn process_child() {
    let Some(path) = std::env::var_os("CORTEX_TEST_STORE") else {
        return;
    };
    let mode = std::env::var("CORTEX_TEST_MODE").unwrap();
    let control = PathBuf::from(std::env::var_os("CORTEX_TEST_CONTROL").unwrap());
    let mut bytes = [0; 32];
    std::io::stdin().read_exact(&mut bytes).unwrap();
    let key = StoreKey::new(&bytes).unwrap();
    let s = HistoryStore::open(Path::new(&path), &key, Access::Recover).unwrap();
    if mode == "load" {
        let h = s.load(&key, Access::ReadOnly).unwrap();
        assert_eq!(h.records().len(), 2);
        assert_eq!(h.context(), &context());
        for (domain, cutoff, version) in [(3, 0, "a"), (3, 1, "b"), (4, 1, "a")] {
            let q = TemporalQuery::new(context().id().clone(), at(domain), at(cutoff)).unwrap();
            assert_eq!(
                h.query(&RecordId::new("requirement").unwrap(), &q)
                    .unwrap()
                    .record()
                    .unwrap()
                    .id()
                    .as_str(),
                version
            );
        }
        return;
    }
    if mode == "race" {
        let r = s.admit(&key, entry("child", Some("a"), 0, None, 1));
        fs::write(
            control.join("result"),
            if r.is_ok() { "committed" } else { "rejected" },
        )
        .unwrap();
        return;
    }
    let signal = |stage: Stage| {
        let matched = matches!(
            (mode.as_str(), stage),
            ("before", Stage::BeforeWrite)
                | ("during", Stage::BeforeCommit)
                | ("after", Stage::AfterCommit)
                | ("remove-intent", Stage::RemovalIntent)
                | ("remove-file", Stage::RemovalFile)
        );
        if matched {
            fs::write(control.join("ready"), b"boundary").unwrap();
            // A failed parent assertion must not leave an orphaned test child
            // holding the store lock indefinitely.
            for _ in 0..300 {
                std::thread::sleep(Duration::from_millis(100));
            }
            panic!("controlled fault boundary expired");
        }
    };
    if mode.starts_with("remove") {
        s.remove_observed(&key, Some(RemovalRequest::whole_collection(s.id())), signal)
            .unwrap();
    } else {
        s.admit_observed(&key, entry("b", Some("a"), 2, Some(4), 1), |stage, c| {
            if matches!(stage, Stage::BeforeWrite) {
                c.execute_batch("PRAGMA cache_size=1").unwrap();
            }
            signal(stage)
        })
        .unwrap();
    }
}
#[test]
fn actual_separate_process_reopens_with_explicit_stdin_key() {
    let f = Fixture::new();
    let s = create(&f);
    s.admit(&key(), entry("a", None, 0, None, 0)).unwrap();
    s.admit(&key(), entry("b", Some("a"), 2, Some(4), 1))
        .unwrap();
    let output = child(&f, "load").wait_with_output().unwrap();
    assert!(output.status.success(), "{:?}", output.stderr);
}
#[test]
fn process_crash_cutpoints_recover_old_or_committed_history_and_reconcile() {
    for mode in ["before", "during", "after"] {
        let f = Fixture::new();
        let s = create(&f);
        s.admit(&key(), entry("a", None, 0, None, 0)).unwrap();
        let c = child(&f, mode);
        wait_ready(&f);
        if mode == "during" {
            assert!(fs::metadata(f.store().join(JOURNAL)).unwrap().len() > 4096);
            // Inspect regular store descriptors, including any unlinked temporary
            // files. Never read child stdin/stderr pipes or arbitrary process files.
            for fd in fs::read_dir(format!("/proc/{}/fd", c.id())).unwrap() {
                let fd = fd.unwrap().path();
                let target = fs::read_link(&fd).unwrap();
                if target.starts_with(f.store()) && fs::metadata(&fd).unwrap().is_file() {
                    let mut bytes = Vec::new();
                    fs::File::open(&fd)
                        .unwrap()
                        .take(MAX_BYTES as u64)
                        .read_to_end(&mut bytes)
                        .unwrap();
                    assert!(!bytes.windows(SECRET.len()).any(|b| b == SECRET.as_bytes()));
                }
            }
        }
        no_plaintext(&f);
        stop(c);
        if s.location.journal_exists().unwrap() {
            let before = snapshot(&f);
            assert_eq!(
                s.load(&key(), Access::ReadOnly).unwrap_err().kind(),
                ErrorKind::RecoveryRequired
            );
            assert_eq!(snapshot(&f), before);
        }
        let h = s.load(&key(), Access::Recover).unwrap();
        assert_eq!(h.records().len(), if mode == "after" { 2 } else { 1 });
        let r = s
            .reconcile(
                &key(),
                &context(),
                &entry("b", Some("a"), 2, Some(4), 1),
                Access::Recover,
            )
            .unwrap();
        assert!(if mode == "after" {
            matches!(r, Reconciliation::Present(_))
        } else {
            r == Reconciliation::Absent
        });
        no_plaintext(&f);
    }
}
#[test]
fn interrupted_removal_blocks_reads_and_explicit_resume_handles_residual_files() {
    for mode in ["remove-intent", "remove-file"] {
        let f = Fixture::new();
        let s = create(&f);
        s.admit(&key(), entry("a", None, 0, None, 0)).unwrap();
        let c = child(&f, mode);
        wait_ready(&f);
        stop(c);
        assert_eq!(
            HistoryStore::open(&f.store(), &key(), Access::Recover)
                .unwrap_err()
                .kind(),
            ErrorKind::RemovalPending
        );
        HistoryStore::resume_removal(&f.store(), &key(), RemovalRequest::whole_collection(s.id()))
            .unwrap();
        assert_eq!(snapshot(&f).len(), 1);
    }
}
#[test]
fn lifecycle_contention_is_bounded_and_serializes_process_admission() {
    for mode in ["during", "after"] {
        let f = Fixture::new();
        let s = create(&f);
        s.admit(&key(), entry("a", None, 0, None, 0)).unwrap();
        let first = child(&f, mode);
        wait_ready(&f);
        if mode == "during" {
            let start = Instant::now();
            assert_eq!(
                s.load(&key(), Access::ReadOnly).unwrap_err().kind(),
                ErrorKind::Busy
            );
            assert!(start.elapsed() < Duration::from_secs(7));
        }
        let second = child(&f, "race");
        stop(first);
        let output = second.wait_with_output().unwrap();
        assert!(output.status.success());
        assert_eq!(
            fs::read_to_string(f.0.join("result")).unwrap(),
            if mode == "during" {
                "committed"
            } else {
                "rejected"
            }
        );
        let h = s.load(&key(), Access::Recover).unwrap();
        assert_eq!(h.records().len(), 2);
        assert_eq!(
            h.records()[1].id().as_str(),
            if mode == "during" { "child" } else { "b" }
        );
    }
}
#[test]
fn commit_busy_is_indeterminate_and_reconciliation_does_not_guess() {
    let f = Fixture::new();
    let s = create(&f);
    s.admit(&key(), entry("a", None, 0, None, 0)).unwrap();
    let reader = connect(&s.location, &key(), Access::ReadOnly).unwrap();
    reader.execute_batch("BEGIN; SELECT * FROM meta;").unwrap();
    let b = entry("b", Some("a"), 0, None, 1);
    let e = s.admit(&key(), b.clone()).unwrap_err();
    assert_eq!(e.completion(), Completion::Indeterminate);
    assert_eq!(e.error().kind(), ErrorKind::Busy);
    reader.execute_batch("ROLLBACK").unwrap();
    drop(reader);
    assert_eq!(
        s.reconcile(&key(), &context(), &b, Access::Recover)
            .unwrap(),
        Reconciliation::Absent
    );
}
#[test]
fn engine_capacity_failure_confirms_rollback() {
    let f = Fixture::new();
    let s = create(&f);
    let text = "x".repeat(4096);
    let record = CanonicalRecord::initial(
        RecordId::new("requirement").unwrap(),
        RecordVersionId::new("a").unwrap(),
        RecordKind::Requirement,
        context().id().clone(),
        RecordContent::new(
            &text,
            vec![SourceReference::operator_reported(&text, &text, &text).unwrap(); 32],
        )
        .unwrap(),
        RecordTime::new(DomainInterval::new(at(0), None).unwrap(), at(0)).unwrap(),
    );
    let before = s.load(&key(), Access::ReadOnly).unwrap();
    let err = s
        .admit_observed(&key(), record, |stage, c| {
            if matches!(stage, Stage::BeforeWrite) {
                let pages: i64 = c.query_row("PRAGMA page_count", [], |r| r.get(0)).unwrap();
                c.pragma_update(None, "max_page_count", pages).unwrap();
            }
        })
        .unwrap_err();
    assert_eq!(err.error().kind(), ErrorKind::Capacity);
    assert_eq!(err.completion(), Completion::RolledBack);
    assert_eq!(s.load(&key(), Access::Recover).unwrap(), before);
    assert!(
        s.load(&key(), Access::Recover)
            .unwrap()
            .records()
            .is_empty()
    );
}

#[test]
fn oversized_file_plaintext_foreign_database_and_ciphertext_corruption_fail_closed() {
    let f = Fixture::new();
    let s = create(&f);
    let file = fs::OpenOptions::new()
        .write(true)
        .open(f.store().join(DATABASE))
        .unwrap();
    file.set_len(MAX_BYTES as u64 + 1).unwrap();
    drop(file);
    assert_eq!(
        s.load(&key(), Access::ReadOnly).unwrap_err().kind(),
        ErrorKind::Capacity
    );
    fs::remove_file(f.store().join(DATABASE)).unwrap();
    let plain = Connection::open(f.store().join(DATABASE)).unwrap();
    plain.execute_batch("CREATE TABLE fixture(v); INSERT INTO fixture VALUES('non-sensitive plaintext fixture');").unwrap();
    drop(plain);
    fs::set_permissions(f.store().join(DATABASE), fs::Permissions::from_mode(0o600)).unwrap();
    let before = snapshot(&f);
    assert_eq!(
        s.load(&key(), Access::Recover).unwrap_err().kind(),
        ErrorKind::CannotUnlock
    );
    assert_eq!(snapshot(&f), before);
    let g = Fixture::new();
    let t = create(&g);
    t.admit(&key(), entry("a", None, 0, None, 0)).unwrap();
    use std::io::{Seek, SeekFrom};
    let mut bytes = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(g.store().join(DATABASE))
        .unwrap();
    bytes.seek(SeekFrom::Start(8192 + 200)).unwrap();
    let mut b = [0];
    bytes.read_exact(&mut b).unwrap();
    bytes.seek(SeekFrom::Start(8192 + 200)).unwrap();
    b[0] ^= 1;
    bytes.write_all(&b).unwrap();
    drop(bytes);
    assert_eq!(
        t.load(&key(), Access::Recover).unwrap_err().kind(),
        ErrorKind::Corrupt
    );
}

#[test]
fn private_format_policy_schema_and_mode_are_not_automatically_reinterpreted() {
    for mutation in [
        "UPDATE meta SET policy='unknown'",
        "CREATE TRIGGER extra AFTER INSERT ON records BEGIN DELETE FROM records; END",
        "PRAGMA journal_mode=WAL",
    ] {
        let f = Fixture::new();
        let s = create(&f);
        let c = connect(&s.location, &key(), Access::Recover).unwrap();
        c.execute_batch(mutation).unwrap();
        drop(c);
        let before = snapshot(&f);
        let e = s.load(&key(), Access::Recover).unwrap_err();
        assert!(matches!(
            e.kind(),
            ErrorKind::Unsupported | ErrorKind::Corrupt | ErrorKind::UnexpectedFiles
        ));
        assert_eq!(snapshot(&f), before);
    }
}

#[test]
fn quoted_unicode_payload_is_data_not_sql_or_authority() {
    let f = Fixture::new();
    let s = create(&f);
    let body = "'); DROP TABLE meta; --\nDecision with Unicode: ação 🦀";
    let r = CanonicalRecord::initial(
        RecordId::new("quoted").unwrap(),
        RecordVersionId::new("v1").unwrap(),
        RecordKind::Decision,
        context().id().clone(),
        RecordContent::new(
            body,
            vec![SourceReference::operator_reported("operator", "source's decision", "1").unwrap()],
        )
        .unwrap(),
        RecordTime::new(DomainInterval::new(at(0), None).unwrap(), at(0)).unwrap(),
    );
    s.admit(&key(), r.clone()).unwrap();
    let h = s.load(&key(), Access::ReadOnly).unwrap();
    assert_eq!(h.records(), &[r]);
    assert_eq!(h.context(), &context());
}
