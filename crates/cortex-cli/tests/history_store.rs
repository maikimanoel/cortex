//! Real Git -> Core -> encrypted store -> validated Core reconstruction.
#![cfg(target_os = "linux")]

mod support;
use cortex_cli::{history_store::*, inspect_candidate};
use cortex_core::{CandidateId, TaskContract, TaskId, binding::*, temporal::*};
use cortex_evidence::{Provenance, unified::*};
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};
use support::Repo;
static NEXT: AtomicU64 = AtomicU64::new(0);
struct StorePath(PathBuf);
impl StorePath {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!(
            "cortex-encrypted-consumer-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )))
    }
}
impl Drop for StorePath {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn contract(repo: &Repo) -> CandidateBindingContract {
    let root = repo.path.canonicalize().unwrap();
    let head = String::from_utf8(repo.git(&["rev-parse", "HEAD"]).stdout).unwrap();
    CandidateBindingContract::new(
        TaskContract::new(
            TaskId::new("store.integration").unwrap(),
            "Retain explicit requirements",
            "Preserve captured candidate and record history",
        )
        .unwrap(),
        ContractRevision::new(1).unwrap(),
        CandidateRequirement::new(
            RepositoryIdentity::new(root.clone(), root.join(".git"), root.join(".git")).unwrap(),
            CandidateId::from_git_commit(head.trim()).unwrap(),
            WorktreeRequirement::AllowChanges,
        ),
    )
}
fn capture(repo: &Repo, c: &CandidateBindingContract, n: u64) -> CandidateObservation {
    inspect_candidate(&repo.path, c, ObservationId::new("store-git", n).unwrap()).unwrap()
}
fn record(c: &CandidateObservation) -> CanonicalRecord {
    CanonicalRecord::initial(
        RecordId::new("decision").unwrap(),
        RecordVersionId::new("v1").unwrap(),
        RecordKind::Decision,
        c.id().clone(),
        RecordContent::new(
            "A recorded decision does not grant authority",
            vec![
                SourceReference::operator_reported("fixture operator", "explicit decision", "1")
                    .unwrap(),
            ],
        )
        .unwrap(),
        RecordTime::new(DomainInterval::new(UNIX_EPOCH, None).unwrap(), UNIX_EPOCH).unwrap(),
    )
}
fn snapshot(repo: &Repo) -> BTreeMap<PathBuf, (Vec<u8>, SystemTime, u64, bool)> {
    let mut map = BTreeMap::new();
    let mut pending = vec![repo.path.clone()];
    while let Some(p) = pending.pop() {
        let m = fs::symlink_metadata(&p).unwrap();
        assert!(!m.is_symlink());
        let b = if m.is_dir() {
            pending.extend(fs::read_dir(&p).unwrap().map(|e| e.unwrap().path()));
            vec![]
        } else {
            fs::read(&p).unwrap()
        };
        map.insert(
            p.strip_prefix(&repo.path).unwrap().to_owned(),
            (
                b,
                m.modified().unwrap(),
                m.len(),
                m.permissions().readonly(),
            ),
        );
    }
    map
}

#[test]
fn clean_dirty_detached_unborn_captures_roundtrip_without_git_mutation() {
    let repo = Repo::new();
    repo.commit_file("tracked", "original");
    let c = contract(&repo);
    let key = StoreKey::new(&[17; 32]).unwrap();
    for phase in 0..4 {
        match phase {
            1 => {
                fs::write(repo.path.join("tracked"), "staged").unwrap();
                repo.git(&["add", "tracked"]);
                fs::write(repo.path.join("tracked"), "unstaged").unwrap();
                fs::write(repo.path.join("new"), "untracked").unwrap();
            }
            2 => {
                repo.git(&["checkout", "--detach"]);
            }
            3 => {
                repo.git(&["checkout", "--orphan", "orphan"]);
            }
            _ => {}
        }
        let before = snapshot(&repo);
        let observed = capture(&repo, &c, phase + 1);
        let path = StorePath::new();
        let s = HistoryStore::create(&path.0, &key, observed.clone()).unwrap();
        let e = record(&observed);
        s.admit(&key, e.clone()).unwrap();
        let h = HistoryStore::open(&path.0, &key, Access::ReadOnly)
            .unwrap()
            .load(&key, Access::ReadOnly)
            .unwrap();
        assert_eq!(h.context(), &observed);
        assert_eq!(h.records(), &[e]);
        assert_eq!(snapshot(&repo), before);
        assert!(observed.basis().is_some());
        assert_eq!(
            h.context().basis().unwrap().material(),
            observed.basis().unwrap().material()
        );
        if phase == 0 {
            assert!(h.context().state().entries().is_empty());
        }
        if phase == 1 {
            assert!(!h.context().state().entries().is_empty());
        }
        if phase == 2 {
            assert!(matches!(h.context().state().head(), Head::Detached { .. }));
        }
        if phase == 3 {
            assert!(matches!(h.context().state().head(), Head::Unborn { .. }));
        }
    }
}

#[test]
fn storage_location_inside_inspected_source_is_rejected_without_mutation() {
    let repo = Repo::new();
    repo.commit_file("tracked", "original");
    let c = contract(&repo);
    let before = snapshot(&repo);
    let observed = capture(&repo, &c, 1);
    assert_eq!(
        HistoryStore::create(
            &repo.path.join("store"),
            &StoreKey::new(&[8; 32]).unwrap(),
            observed
        )
        .unwrap_err()
        .kind(),
        ErrorKind::InvalidLocation
    );
    assert_eq!(snapshot(&repo), before);
}

#[test]
fn replay_retains_old_capture_without_promoting_inference_or_changed_candidate() {
    let repo = Repo::new();
    repo.commit_file("tracked", "original");
    let c = contract(&repo);
    let captured = capture(&repo, &c, 1);
    let latest = capture(&repo, &c, 2);
    assert!(matches!(
        bind_candidate(&c, Some(&captured), Some(&latest)).unwrap(),
        BindingDecision::Compatible(_)
    ));
    let profile = VerificationProfile::new(
        Provenance::new(
            "fixture reasoning",
            "recorded decision",
            "no verification execution",
        )
        .unwrap(),
        "1",
        "fixture",
        &[],
    )
    .unwrap();
    let mut evidence = EvidenceRecord::new(
        ReportedObservation::new(
            ObservationId::new("reasoning", 1).unwrap(),
            captured.clone(),
            profile.clone(),
            ObservationTimes::new(UNIX_EPOCH, UNIX_EPOCH).unwrap(),
            ObservationKind::Inference,
            "Historical data is not evidence of successful execution",
        )
        .unwrap(),
    );
    let original = evidence.clone();
    let key = StoreKey::new(&[7; 32]).unwrap();
    let path = StorePath::new();
    let s = HistoryStore::create(&path.0, &key, captured.clone()).unwrap();
    s.admit(&key, record(&captured)).unwrap();
    fs::write(repo.path.join("tracked"), "changed after capture").unwrap();
    let changed = capture(&repo, &c, 3);
    let before = snapshot(&repo);
    assert_eq!(s.load(&key, Access::Recover).unwrap().context(), &captured);
    assert_eq!(
        s.admit(&key, record(&changed)).unwrap_err().error().kind(),
        ErrorKind::Domain
    );
    assert_eq!(
        s.reconcile(&key, &changed, &record(&changed), Access::Recover)
            .unwrap(),
        Reconciliation::Conflict
    );
    assert_eq!(evidence, original);
    assert_eq!(
        evidence.assess(
            AssessmentId::new("unsupported").unwrap(),
            UNIX_EPOCH,
            AssessmentResult::Pass,
            Applicability::Applicable,
            "Persistence and compatibility cannot justify passing",
            &ValidationContext::new(&c, Some(&latest), Some(&profile))
        ),
        Err(SemanticError::UnsupportedResult)
    );
    assert_eq!(evidence, original);
    assert_eq!(snapshot(&repo), before);
}
