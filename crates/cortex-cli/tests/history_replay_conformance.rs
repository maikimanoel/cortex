//! PP05 / CAP-MEM-001 / CAP-SEC-002: application-level encrypted replay conformance.
//! Core is the domain oracle; literal identities and selections check shared assumptions.
//! Discarded receipts model lost application acknowledgement, not failed native commits.
#![cfg(target_os = "linux")]

mod support;

use cortex_cli::{history_store::*, inspect_candidate};
use cortex_core::{CandidateId, TaskContract, TaskId, binding::*, temporal::*};
use std::{
    collections::BTreeMap,
    error::Error,
    fs,
    os::unix::fs::{DirBuilderExt, PermissionsExt},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use support::Repo;

const BODY: &str = "W05_PRIVATE_BODY_synthetic_replay_7391";
const SOURCE: &str = "W05_PRIVATE_SOURCE_synthetic_replay_2857";
const AUTHOR: &str = "W05_PRIVATE_AUTHOR_synthetic_replay_9603";
const ENTITIES: [&str; 4] = [
    "requirement.alpha",
    "decision.beta",
    "requirement.gamma",
    "decision.delta",
];
const IDENTITIES: [(&str, &str); 8] = [
    ("requirement.alpha", "z-first"),
    ("decision.beta", "a-first"),
    ("requirement.gamma", "m-first"),
    ("decision.delta", "b-first"),
    ("requirement.alpha", "y-next"),
    ("decision.beta", "c-next"),
    ("requirement.gamma", "l-next"),
    ("decision.delta", "d-next"),
];
static NEXT: AtomicU64 = AtomicU64::new(0);

// Atomically reserve a private parent; cleanup only ever targets that owned parent.
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        loop {
            let path = std::env::temp_dir().join(format!(
                "cortex-w05-replay-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::DirBuilder::new().mode(0o700).create(&path) {
                Ok(()) => return Self(path.canonicalize().unwrap()),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => panic!("cannot reserve synthetic replay directory: {:?}", e.kind()),
            }
        }
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

// Contents, mtime, length and Unix permissions, including .git; access times and
// inode/change/birth metadata are deliberately outside this read-invariance oracle.
type Snapshot = BTreeMap<PathBuf, (Vec<u8>, SystemTime, u64, u32)>;
fn snapshot(root: &Path) -> Snapshot {
    let mut result = BTreeMap::new();
    let mut pending = vec![root.to_owned()];
    while let Some(path) = pending.pop() {
        let meta = fs::symlink_metadata(&path).unwrap();
        assert!(!meta.is_symlink());
        let bytes = if meta.is_dir() {
            pending.extend(fs::read_dir(&path).unwrap().map(|e| e.unwrap().path()));
            vec![]
        } else {
            fs::read(&path).unwrap()
        };
        result.insert(
            path.strip_prefix(root).unwrap().to_owned(),
            (
                bytes,
                meta.modified().unwrap(),
                meta.len(),
                meta.permissions().mode(),
            ),
        );
    }
    result
}

struct Fixture {
    repo: Repo,
    scratch: Scratch,
    path: PathBuf,
    context: CandidateObservation,
    source_before: Snapshot,
}
impl Fixture {
    fn new() -> Self {
        let repo = Repo::new();
        repo.commit_file("requirements.txt", "Synthetic replay fixture\n");
        let root = repo.path.canonicalize().unwrap();
        let commit = String::from_utf8(repo.git(&["rev-parse", "HEAD"]).stdout).unwrap();
        let contract = CandidateBindingContract::new(
            TaskContract::new(
                TaskId::new("history.replay.conformance").unwrap(),
                "Retain synthetic interleaved canonical history",
                "Reload exact records and reconcile original admissions",
            )
            .unwrap(),
            ContractRevision::new(1).unwrap(),
            CandidateRequirement::new(
                RepositoryIdentity::new(root.clone(), root.join(".git"), root.join(".git"))
                    .unwrap(),
                CandidateId::from_git_commit(commit.trim()).unwrap(),
                WorktreeRequirement::Clean,
            ),
        );
        let source_before = snapshot(&repo.path);
        let context = inspect_candidate(
            &repo.path,
            &contract,
            ObservationId::new("w05-capture", 1).unwrap(),
        )
        .unwrap();
        let scratch = Scratch::new();
        let path = scratch.0.join("store");
        assert!(!path.starts_with(&root));
        Self {
            repo,
            scratch,
            path,
            context,
            source_before,
        }
    }

    fn assert_source_unchanged(&self) {
        assert_eq!(snapshot(&self.repo.path), self.source_before);
    }

    fn assert_diagnostic_private(&self, text: &str) {
        for private in [BODY, SOURCE, AUTHOR] {
            assert!(
                !text.contains(private),
                "diagnostic exposed a synthetic sentinel"
            );
        }
        for path in [&self.repo.path, &self.path] {
            assert!(
                !text.contains(path.to_str().unwrap()),
                "diagnostic exposed a fixture path"
            );
        }
    }

    fn assert_managed_files_private(&self) {
        for (bytes, _, _, _) in snapshot(&self.path).values() {
            for private in [BODY, SOURCE, AUTHOR] {
                assert!(
                    !bytes
                        .windows(private.len())
                        .any(|b| b == private.as_bytes()),
                    "managed live file exposed a synthetic sentinel"
                );
            }
        }
    }
}

fn key() -> StoreKey {
    // Explicit synthetic test material only, supplied through library memory.
    StoreKey::new(&[91; 32]).unwrap()
}
fn at(seconds: u64) -> SystemTime {
    UNIX_EPOCH + Duration::new(seconds, 987_654_321)
}
fn content(version: &str) -> RecordContent {
    RecordContent::new(
        &format!("{BODY}: {version}\nUnicode: é; quoted 'record'"),
        vec![
            SourceReference::observation_reference(
                ObservationId::new("w05-unresolved-source", 7).unwrap(),
                SOURCE,
                version,
            )
            .unwrap(),
            SourceReference::operator_reported(AUTHOR, "synthetic operator assertion", "source-v1")
                .unwrap(),
        ],
    )
    .unwrap()
}
fn draft(
    context: &ObservationId,
    entity: &str,
    version: &str,
    kind: RecordKind,
    predecessor: Option<&str>,
    domain: (u64, Option<u64>),
    recorded: u64,
) -> CanonicalRecord {
    let entity = RecordId::new(entity).unwrap();
    let version = RecordVersionId::new(version).unwrap();
    let time = RecordTime::new(
        DomainInterval::new(at(domain.0), domain.1.map(at)).unwrap(),
        at(recorded),
    )
    .unwrap();
    match predecessor {
        Some(previous) => CanonicalRecord::correction(
            entity,
            version.clone(),
            kind,
            context.clone(),
            RecordVersionId::new(previous).unwrap(),
            content(version.as_str()),
            time,
        ),
        None => CanonicalRecord::initial(
            entity,
            version.clone(),
            kind,
            context.clone(),
            content(version.as_str()),
            time,
        ),
    }
}
fn trace(context: &ObservationId) -> Vec<CanonicalRecord> {
    use RecordKind::{Decision, Requirement};
    vec![
        draft(
            context,
            "requirement.alpha",
            "z-first",
            Requirement,
            None,
            (0, None),
            50,
        ),
        draft(
            context,
            "decision.beta",
            "a-first",
            Decision,
            None,
            (10, Some(40)),
            10,
        ),
        draft(
            context,
            "requirement.gamma",
            "m-first",
            Requirement,
            None,
            (20, None),
            30,
        ),
        draft(
            context,
            "decision.delta",
            "b-first",
            Decision,
            None,
            (0, None),
            5,
        ),
        draft(
            context,
            "requirement.alpha",
            "y-next",
            Requirement,
            Some("z-first"),
            (10, Some(20)),
            50,
        ),
        draft(
            context,
            "decision.beta",
            "c-next",
            Decision,
            Some("a-first"),
            (20, Some(30)),
            10,
        ),
        draft(
            context,
            "requirement.gamma",
            "l-next",
            Requirement,
            Some("m-first"),
            (0, Some(10)),
            31,
        ),
        draft(
            context,
            "decision.delta",
            "d-next",
            Decision,
            Some("b-first"),
            (5, None),
            6,
        ),
    ]
}

fn check_history(loaded: &CanonicalHistory, oracle: &CanonicalHistory) {
    assert_eq!(loaded, oracle); // Includes all content, times, provenance and capture material.
    assert_eq!(loaded.context(), oracle.context());
    assert_eq!(loaded.retained_bytes(), oracle.retained_bytes());
    let count = oracle.records().len();
    assert_eq!(
        loaded
            .records()
            .iter()
            .map(|r| (r.entity().as_str(), r.id().as_str()))
            .collect::<Vec<_>>(),
        IDENTITIES[..count]
    );
    for expected in oracle.records() {
        assert_eq!(loaded.version(expected.id()).unwrap(), expected);
    }
    for entity in ENTITIES {
        let id = RecordId::new(entity).unwrap();
        let expected_ids = IDENTITIES[..count]
            .iter()
            .filter(|(e, _)| *e == entity)
            .map(|(_, v)| *v)
            .collect::<Vec<_>>();
        if expected_ids.is_empty() {
            assert_eq!(
                loaded.history(&id).unwrap_err(),
                TemporalError::MissingEntity
            );
            let q = TemporalQuery::new(loaded.context().id().clone(), at(20), at(50)).unwrap();
            assert_eq!(
                loaded.query(&id, &q).unwrap_err(),
                TemporalError::MissingEntity
            );
            continue;
        }
        let chain = loaded.history(&id).unwrap();
        assert_eq!(chain.entity(), &id);
        assert_eq!(chain.context(), oracle.context());
        assert_eq!(
            chain.records().map(|r| r.id().as_str()).collect::<Vec<_>>(),
            expected_ids
        );
        assert_eq!(
            chain.records().collect::<Vec<_>>(),
            oracle.history(&id).unwrap().records().collect::<Vec<_>>()
        );
        // One loaded history per phase; the point-query grid performs no storage I/O.
        for domain in [0, 5, 9, 10, 19, 20, 29, 30, 40, 100] {
            for cutoff in [4, 5, 6, 9, 10, 29, 30, 31, 49, 50] {
                let q = TemporalQuery::new(loaded.context().id().clone(), at(domain), at(cutoff))
                    .unwrap();
                assert_eq!(
                    loaded.query(&id, &q).unwrap(),
                    oracle.query(&id, &q).unwrap()
                );
            }
        }
    }
    assert_eq!(
        loaded
            .version(&RecordVersionId::new("never-admitted").unwrap())
            .unwrap_err(),
        TemporalError::MissingVersion
    );
    let wrong = TemporalQuery::new(
        ObservationId::new("w05-other-capture", 1).unwrap(),
        at(10),
        at(50),
    )
    .unwrap();
    assert_eq!(
        loaded
            .query(&RecordId::new(ENTITIES[0]).unwrap(), &wrong)
            .unwrap_err(),
        TemporalError::ScopeMismatch
    );
}

fn reopen_and_check(
    f: &Fixture,
    store: HistoryStore,
    key: &StoreKey,
    oracle: &CanonicalHistory,
    access: Access,
) -> HistoryStore {
    let id = store.id();
    drop(store);
    let reopened = HistoryStore::open(&f.path, key, access).unwrap();
    assert_eq!(reopened.id(), id);
    let loaded = reopened.load(key, access).unwrap();
    check_history(&loaded, oracle);
    f.assert_diagnostic_private(&format!("{loaded:?} {reopened:?} {key:?}"));
    f.assert_source_unchanged();
    reopened
}

fn reject(
    f: &Fixture,
    store: &HistoryStore,
    key: &StoreKey,
    entry: CanonicalRecord,
    cause: TemporalError,
) {
    let failure = store.admit(key, entry).unwrap_err();
    assert_eq!(failure.completion(), Completion::Rejected);
    assert_eq!(failure.error().kind(), ErrorKind::Domain);
    assert_eq!(
        failure
            .error()
            .source()
            .unwrap()
            .downcast_ref::<TemporalError>(),
        Some(&cause)
    );
    f.assert_diagnostic_private(&format!(
        "{failure:?} {failure} {}",
        failure.error().source().unwrap()
    ));
}

fn check_literal_queries(history: &CanonicalHistory) {
    let cases = [
        (ENTITIES[0], 0, 49, None),
        (ENTITIES[0], 0, 50, Some("z-first")),
        (ENTITIES[0], 10, 50, Some("y-next")),
        (ENTITIES[0], 19, 50, Some("y-next")),
        (ENTITIES[0], 20, 50, Some("z-first")),
        (ENTITIES[0], 100, 50, Some("z-first")),
        (ENTITIES[1], 9, 10, None),
        (ENTITIES[1], 10, 9, None),
        (ENTITIES[1], 10, 10, Some("a-first")),
        (ENTITIES[1], 20, 10, Some("c-next")),
        (ENTITIES[1], 29, 10, Some("c-next")),
        (ENTITIES[1], 30, 10, Some("a-first")),
        (ENTITIES[1], 40, 10, None),
        (ENTITIES[2], 0, 30, None),
        (ENTITIES[2], 0, 31, Some("l-next")),
        (ENTITIES[2], 9, 31, Some("l-next")),
        (ENTITIES[2], 10, 31, None),
        (ENTITIES[2], 20, 29, None),
        (ENTITIES[2], 20, 30, Some("m-first")),
        (ENTITIES[2], 20, 31, Some("m-first")),
        (ENTITIES[3], 0, 4, None),
        (ENTITIES[3], 0, 6, Some("b-first")),
        (ENTITIES[3], 5, 5, Some("b-first")),
        (ENTITIES[3], 5, 6, Some("d-next")),
    ];
    for (entity, domain, cutoff, expected) in cases {
        let q = TemporalQuery::new(history.context().id().clone(), at(domain), at(cutoff)).unwrap();
        let view = history.query(&RecordId::new(entity).unwrap(), &q).unwrap();
        assert_eq!(view.context(), history.context());
        assert_eq!(
            view.record().map(|r| r.id().as_str()),
            expected,
            "literal selection: {entity}, {domain}, {cutoff}"
        );
    }
    // Inclusive recording cutoffs at nanosecond precision; domain end is exclusive.
    let q = TemporalQuery::new(
        history.context().id().clone(),
        at(10),
        at(50) - Duration::from_nanos(1),
    )
    .unwrap();
    assert!(
        history
            .query(&RecordId::new(ENTITIES[0]).unwrap(), &q)
            .unwrap()
            .record()
            .is_none()
    );
    let q = TemporalQuery::new(
        history.context().id().clone(),
        at(20) - Duration::from_nanos(1),
        at(50),
    )
    .unwrap();
    assert_eq!(
        history
            .query(&RecordId::new(ENTITIES[0]).unwrap(), &q)
            .unwrap()
            .record()
            .unwrap()
            .id()
            .as_str(),
        "y-next"
    );
}

#[test]
fn interleaved_admissions_and_rejections_reopen_as_exact_core_history() {
    use RecordKind::{Decision, Requirement};
    let f = Fixture::new();
    let key = key();
    let mut oracle = CanonicalHistory::new(f.context.clone());
    let mut store = HistoryStore::create(&f.path, &key, f.context.clone()).unwrap();
    store = reopen_and_check(&f, store, &key, &oracle, Access::ReadOnly);
    let records = trace(f.context.id());
    let other = ObservationId::new("w05-capture", 2).unwrap();
    // Sixteen storage attempts, eight retained versions, one store. Rejected inputs
    // are interspersed with later valid work to catch damage and ordinal gaps.
    let operations = vec![
        (records[0].clone(), None),
        (records[1].clone(), None),
        (records[0].clone(), Some(TemporalError::DuplicateVersion)),
        (records[2].clone(), None),
        (
            draft(
                f.context.id(),
                ENTITIES[2],
                "wrong-entity",
                Requirement,
                Some("a-first"),
                (0, None),
                50,
            ),
            Some(TemporalError::EntityMismatch),
        ),
        (records[3].clone(), None),
        (
            draft(
                f.context.id(),
                ENTITIES[0],
                "wrong-category",
                Decision,
                Some("z-first"),
                (0, None),
                50,
            ),
            Some(TemporalError::CategoryMismatch),
        ),
        (records[4].clone(), None),
        (
            draft(
                f.context.id(),
                ENTITIES[0],
                "stale",
                Requirement,
                Some("z-first"),
                (0, None),
                50,
            ),
            Some(TemporalError::StalePredecessor),
        ),
        (records[5].clone(), None),
        (
            draft(
                &other,
                ENTITIES[2],
                "wrong-context",
                Requirement,
                Some("m-first"),
                (0, None),
                31,
            ),
            Some(TemporalError::ScopeMismatch),
        ),
        (records[6].clone(), None),
        (
            draft(
                f.context.id(),
                ENTITIES[3],
                "backwards",
                Decision,
                Some("b-first"),
                (0, None),
                4,
            ),
            Some(TemporalError::RecordingOrder),
        ),
        (records[7].clone(), None),
        (
            draft(
                f.context.id(),
                ENTITIES[0],
                "y-next",
                Requirement,
                Some("y-next"),
                (0, None),
                51,
            ),
            Some(TemporalError::DuplicateVersion),
        ),
        (
            draft(
                f.context.id(),
                ENTITIES[1],
                "missing",
                Decision,
                Some("unavailable"),
                (0, None),
                11,
            ),
            Some(TemporalError::MissingPredecessor),
        ),
    ];
    for (phase, (entry, expected_error)) in operations.into_iter().enumerate() {
        if let Some(cause) = expected_error {
            let before = oracle.clone();
            assert_eq!(oracle.append(entry.clone()), Err(cause.clone()));
            assert_eq!(oracle, before);
            reject(&f, &store, &key, entry, cause);
        } else {
            oracle.append(entry.clone()).unwrap();
            let receipt = store.admit(&key, entry).unwrap();
            assert_eq!(receipt.store(), store.id());
            assert_eq!(receipt.ordinal(), oracle.records().len() as u64);
            f.assert_diagnostic_private(&format!("{receipt:?}"));
        }
        let access = if phase % 2 == 0 {
            Access::ReadOnly
        } else {
            Access::Recover
        };
        store = reopen_and_check(&f, store, &key, &oracle, access);
    }
    let before_reads = snapshot(&f.path);
    let loaded = store.load(&key, Access::ReadOnly).unwrap();
    check_literal_queries(&loaded);
    assert_eq!(loaded.records(), records);
    for (index, entry) in records.iter().enumerate() {
        let Reconciliation::Present(receipt) = store
            .reconcile(&key, &f.context, entry, Access::ReadOnly)
            .unwrap()
        else {
            panic!("committed version was not reconciled as present");
        };
        assert_eq!(receipt.ordinal(), (index + 1) as u64);
        assert_eq!(receipt.store(), store.id());
    }
    assert_eq!(snapshot(&f.path), before_reads);
    f.assert_managed_files_private();
    f.assert_source_unchanged();
}

#[test]
fn lost_application_acknowledgement_reconciles_exactly_without_duplicate_admission() {
    let f = Fixture::new();
    let key = key();
    let mut store = HistoryStore::create(&f.path, &key, f.context.clone()).unwrap();
    let records = trace(f.context.id());
    let mut oracle = CanonicalHistory::new(f.context.clone());
    for entry in &records[..4] {
        store.admit(&key, entry.clone()).unwrap();
        oracle.append(entry.clone()).unwrap();
    }
    let attempted = &records[4];
    // The adapter confirmed this commit. The application deliberately retains no
    // receipt; this does not produce AdmissionFailure or a native COMMIT failure.
    let _ = store.admit(&key, attempted.clone()).unwrap();
    oracle.append(attempted.clone()).unwrap();
    let original_id = store.id();
    drop(store);
    store = HistoryStore::open(&f.path, &key, Access::Recover).unwrap();
    assert_eq!(store.id(), original_id);
    let before_reads = snapshot(&f.path);
    let Reconciliation::Present(original) = store
        .reconcile(&key, &f.context, attempted, Access::Recover)
        .unwrap()
    else {
        panic!("lost application acknowledgement did not reconcile");
    };
    assert_eq!(original.ordinal(), 5);
    assert_eq!(original.store(), original_id);
    assert_eq!(
        store
            .reconcile(&key, &f.context, attempted, Access::ReadOnly)
            .unwrap(),
        Reconciliation::Present(original)
    );

    let other_capture = ObservationId::new("w05-capture", 2).unwrap();
    let mut conflicts = vec![
        draft(
            f.context.id(),
            ENTITIES[1],
            "y-next",
            RecordKind::Requirement,
            Some("z-first"),
            (10, Some(20)),
            50,
        ),
        draft(
            f.context.id(),
            ENTITIES[0],
            "y-next",
            RecordKind::Decision,
            Some("z-first"),
            (10, Some(20)),
            50,
        ),
        draft(
            f.context.id(),
            ENTITIES[0],
            "y-next",
            RecordKind::Requirement,
            Some("m-first"),
            (10, Some(20)),
            50,
        ),
        draft(
            f.context.id(),
            ENTITIES[0],
            "y-next",
            RecordKind::Requirement,
            Some("z-first"),
            (10, Some(21)),
            50,
        ),
        draft(
            f.context.id(),
            ENTITIES[0],
            "y-next",
            RecordKind::Requirement,
            Some("z-first"),
            (10, Some(20)),
            51,
        ),
        draft(
            &other_capture,
            ENTITIES[0],
            "y-next",
            RecordKind::Requirement,
            Some("z-first"),
            (10, Some(20)),
            50,
        ),
    ];
    for changed_content in [
        RecordContent::new(
            "different synthetic body",
            attempted.content().sources().to_vec(),
        )
        .unwrap(),
        RecordContent::new(
            attempted.content().body(),
            vec![
                SourceReference::operator_reported(AUTHOR, SOURCE, "changed-source-version")
                    .unwrap(),
            ],
        )
        .unwrap(),
    ] {
        conflicts.push(CanonicalRecord::correction(
            attempted.entity().clone(),
            attempted.id().clone(),
            attempted.kind(),
            attempted.context().clone(),
            attempted.predecessor().unwrap().clone(),
            changed_content,
            attempted.time(),
        ));
    }
    for changed in conflicts {
        assert_ne!(&changed, attempted);
        assert_eq!(
            store
                .reconcile(&key, &f.context, &changed, Access::ReadOnly)
                .unwrap(),
            Reconciliation::Conflict
        );
    }
    // Equal capture identity still requires the entire original context, including
    // opaque comparison material. These are caller assertions, not authentication.
    let changed_context = CandidateObservation::new(
        f.context.id().clone(),
        f.context.contract(),
        f.context.state().clone(),
        Some(
            ComparisonBasis::new(
                "w05-changed-profile",
                b"changed synthetic comparison material".to_vec(),
            )
            .unwrap(),
        ),
    );
    assert_eq!(changed_context.id(), f.context.id());
    assert_ne!(changed_context, f.context);
    assert_eq!(
        store
            .reconcile(&key, &changed_context, attempted, Access::ReadOnly)
            .unwrap(),
        Reconciliation::Conflict
    );
    assert_eq!(snapshot(&f.path), before_reads);
    reject(
        &f,
        &store,
        &key,
        attempted.clone(),
        TemporalError::DuplicateVersion,
    );
    store = reopen_and_check(&f, store, &key, &oracle, Access::ReadOnly);

    // Only a valid complete same-instance store can confirm absence. Even then,
    // an attempted correction to a stale predecessor must be validated again.
    let stale = draft(
        f.context.id(),
        ENTITIES[0],
        "absent-stale",
        RecordKind::Requirement,
        Some("z-first"),
        (0, None),
        51,
    );
    assert_eq!(
        store
            .reconcile(&key, &f.context, &stale, Access::ReadOnly)
            .unwrap(),
        Reconciliation::Absent
    );
    reject(&f, &store, &key, stale, TemporalError::StalePredecessor);
    let fresh = draft(
        f.context.id(),
        ENTITIES[0],
        "fresh",
        RecordKind::Requirement,
        Some("y-next"),
        (0, None),
        51,
    );
    assert_eq!(
        store
            .reconcile(&key, &f.context, &fresh, Access::ReadOnly)
            .unwrap(),
        Reconciliation::Absent
    );
    let receipt = store.admit(&key, fresh.clone()).unwrap();
    assert_eq!(receipt.ordinal(), 6);
    assert_eq!(receipt.store(), original_id);
    oracle.append(fresh).unwrap();
    drop(store);
    let reopened = HistoryStore::open(&f.path, &key, Access::ReadOnly).unwrap();
    let loaded = reopened.load(&key, Access::ReadOnly).unwrap();
    assert_eq!(loaded, oracle);
    assert_eq!(
        loaded
            .records()
            .iter()
            .map(|r| r.id().as_str())
            .collect::<Vec<_>>(),
        [
            "z-first", "a-first", "m-first", "b-first", "y-next", "fresh"
        ]
    );
    assert_eq!(
        reopened
            .reconcile(&key, &f.context, attempted, Access::ReadOnly)
            .unwrap(),
        Reconciliation::Present(original)
    );
    f.assert_source_unchanged();
}

#[test]
fn missing_store_is_unresolved_until_the_original_instance_returns() {
    let f = Fixture::new();
    let key = key();
    let store = HistoryStore::create(&f.path, &key, f.context.clone()).unwrap();
    let entry = trace(f.context.id()).remove(0);
    let _ = store.admit(&key, entry.clone()).unwrap();
    let held = f.scratch.0.join("temporarily-unavailable");
    fs::rename(&f.path, &held).unwrap();
    for access in [Access::ReadOnly, Access::Recover] {
        let error = store
            .reconcile(&key, &f.context, &entry, access)
            .unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Missing);
        f.assert_diagnostic_private(&format!("{error:?} {error}"));
        assert_eq!(
            HistoryStore::open(&f.path, &key, access)
                .unwrap_err()
                .kind(),
            ErrorKind::Missing
        );
        assert!(!f.path.exists()); // No empty replacement is created by a failed read.
    }
    fs::rename(held, &f.path).unwrap();
    let Reconciliation::Present(receipt) = store
        .reconcile(&key, &f.context, &entry, Access::Recover)
        .unwrap()
    else {
        panic!("restored original store did not confirm its admission");
    };
    assert_eq!(receipt.store(), store.id());
    assert_eq!(receipt.ordinal(), 1);
    assert_eq!(
        store.load(&key, Access::ReadOnly).unwrap().records(),
        &[entry]
    );
    f.assert_source_unchanged();
}

#[test]
fn replacement_with_matching_context_cannot_prove_original_admission_absent() {
    let f = Fixture::new();
    let key = key();
    let store = HistoryStore::create(&f.path, &key, f.context.clone()).unwrap();
    let entry = trace(f.context.id()).remove(0);
    let _ = store.admit(&key, entry.clone()).unwrap();
    let original_id = store.id();
    let replacement_path = f.scratch.0.join("replacement");
    let replacement = HistoryStore::create(&replacement_path, &key, f.context.clone()).unwrap();
    assert_ne!(replacement.id(), original_id);
    drop(replacement);
    let held = f.scratch.0.join("original");
    fs::rename(&f.path, &held).unwrap();
    fs::rename(&replacement_path, &f.path).unwrap();
    let before_reads = snapshot(&f.path);
    for access in [Access::ReadOnly, Access::Recover] {
        let error = store
            .reconcile(&key, &f.context, &entry, access)
            .unwrap_err();
        assert_eq!(error.kind(), ErrorKind::TargetChanged);
        f.assert_diagnostic_private(&format!("{error:?} {error}"));
    }
    // An explicit fresh open can load the new empty instance; that is not absence
    // in the original instance, whose identity must still be checked by the caller.
    let observed_replacement = HistoryStore::open(&f.path, &key, Access::ReadOnly).unwrap();
    assert_ne!(observed_replacement.id(), original_id);
    let loaded = observed_replacement.load(&key, Access::ReadOnly).unwrap();
    assert_eq!(loaded.context(), &f.context);
    assert!(loaded.records().is_empty());
    assert_eq!(snapshot(&f.path), before_reads);
    drop(observed_replacement);
    fs::rename(&f.path, replacement_path).unwrap();
    fs::rename(held, &f.path).unwrap();
    let Reconciliation::Present(receipt) = store
        .reconcile(&key, &f.context, &entry, Access::ReadOnly)
        .unwrap()
    else {
        panic!("original instance no longer reconciled");
    };
    assert_eq!(receipt.store(), original_id);
    assert_eq!(receipt.ordinal(), 1);
    f.assert_source_unchanged();
}

#[test]
fn unreadable_store_is_not_confirmed_absence_and_retains_committed_history() {
    let f = Fixture::new();
    let key = key();
    let store = HistoryStore::create(&f.path, &key, f.context.clone()).unwrap();
    let entry = trace(f.context.id()).remove(0);
    let _ = store.admit(&key, entry.clone()).unwrap();
    let database = f.path.join("history.db");
    fs::set_permissions(&database, fs::Permissions::from_mode(0o000)).unwrap();
    for access in [Access::ReadOnly, Access::Recover] {
        let error = store
            .reconcile(&key, &f.context, &entry, access)
            .unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Permission);
        f.assert_diagnostic_private(&format!("{error:?} {error}"));
        assert_eq!(
            HistoryStore::open(&f.path, &key, access)
                .unwrap_err()
                .kind(),
            ErrorKind::Permission
        );
        assert_eq!(
            fs::metadata(&database).unwrap().permissions().mode() & 0o777,
            0
        );
    }
    fs::set_permissions(database, fs::Permissions::from_mode(0o600)).unwrap();
    let Reconciliation::Present(receipt) = store
        .reconcile(&key, &f.context, &entry, Access::Recover)
        .unwrap()
    else {
        panic!("permission restoration did not reveal original admission");
    };
    assert_eq!(receipt.ordinal(), 1);
    assert_eq!(receipt.store(), store.id());
    assert_eq!(
        store.load(&key, Access::ReadOnly).unwrap().records(),
        &[entry]
    );
    f.assert_source_unchanged();
}

#[test]
fn corrupt_encrypted_header_never_becomes_an_empty_or_repaired_history() {
    let f = Fixture::new();
    let key = key();
    let store = HistoryStore::create(&f.path, &key, f.context.clone()).unwrap();
    let entry = trace(f.context.id()).remove(0);
    let _ = store.admit(&key, entry.clone()).unwrap();
    // Damage only this owned synthetic store. No schema, cipher settings, native
    // fault controls or repair mechanism are introduced by the conformance suite.
    fs::write(
        f.path.join("history.db"),
        b"synthetic damaged encrypted header",
    )
    .unwrap();
    let damaged = snapshot(&f.path);
    for access in [Access::ReadOnly, Access::Recover] {
        let error = store
            .reconcile(&key, &f.context, &entry, access)
            .unwrap_err();
        assert_eq!(error.kind(), ErrorKind::CannotUnlock);
        f.assert_diagnostic_private(&format!("{error:?} {error}"));
        assert_eq!(
            HistoryStore::open(&f.path, &key, access)
                .unwrap_err()
                .kind(),
            ErrorKind::CannotUnlock
        );
        assert_eq!(snapshot(&f.path), damaged);
    }
    f.assert_source_unchanged();
}
