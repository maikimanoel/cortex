//! CAP-MEM-001 consumes genuine 002-A captures without changing 002-B/C semantics.

mod support;

use cortex_cli::inspect_candidate;
use cortex_core::{CandidateId, TaskContract, TaskId, binding::*, temporal::*};
use cortex_evidence::{Provenance, unified::*};
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use support::Repo;

fn contract(repo: &Repo) -> CandidateBindingContract {
    let root = repo.path.canonicalize().unwrap();
    let head = String::from_utf8(repo.git(&["rev-parse", "--verify", "HEAD"]).stdout).unwrap();
    CandidateBindingContract::new(
        TaskContract::new(
            TaskId::new("M003A.integration").unwrap(),
            "Retain historical requirements",
            "Corrections preserve original context and evidence",
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

fn capture(repo: &Repo, c: &CandidateBindingContract, sequence: u64) -> CandidateObservation {
    inspect_candidate(
        &repo.path,
        c,
        ObservationId::new("real-repository-captures", sequence).unwrap(),
    )
    .unwrap()
}

fn entry(capture: &ObservationId, correction: bool) -> CanonicalRecord {
    let entity = RecordId::new("release.requirement").unwrap();
    let id = RecordVersionId::new(if correction { "v2" } else { "v1" }).unwrap();
    let content = RecordContent::new(
        if correction {
            "Revised requirement"
        } else {
            "Original requirement"
        },
        vec![
            SourceReference::operator_reported(
                "fixture operator",
                "explicit task requirement",
                "1",
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let time = RecordTime::new(
        DomainInterval::new(UNIX_EPOCH, None).unwrap(),
        UNIX_EPOCH + Duration::from_secs(u64::from(correction)),
    )
    .unwrap();
    if correction {
        CanonicalRecord::correction(
            entity,
            id,
            RecordKind::Requirement,
            capture.clone(),
            RecordVersionId::new("v1").unwrap(),
            content,
            time,
        )
    } else {
        CanonicalRecord::initial(
            entity,
            id,
            RecordKind::Requirement,
            capture.clone(),
            content,
            time,
        )
    }
}

// File/directory contents, mtime, length and read-only permissions are compared.
// Reading may affect access times; inode/change/birth metadata are not covered here.
fn snapshot(repo: &Repo) -> BTreeMap<PathBuf, (Vec<u8>, SystemTime, u64, bool)> {
    let mut result = BTreeMap::new();
    let mut pending = vec![repo.path.clone()];
    while let Some(path) = pending.pop() {
        let meta = fs::symlink_metadata(&path).unwrap();
        assert!(!meta.is_symlink());
        let contents = if meta.is_dir() {
            pending.extend(
                fs::read_dir(&path)
                    .unwrap()
                    .map(|entry| entry.unwrap().path()),
            );
            vec![]
        } else {
            fs::read(&path).unwrap()
        };
        result.insert(
            path.strip_prefix(&repo.path).unwrap().to_owned(),
            (
                contents,
                meta.modified().unwrap(),
                meta.len(),
                meta.permissions().readonly(),
            ),
        );
    }
    result
}

#[test]
fn real_clean_candidate_history_is_read_only_and_cannot_promote_inference_to_pass() {
    let repo = Repo::new();
    repo.commit_file("tracked", "original");
    let c = contract(&repo);
    let before = snapshot(&repo);
    let first = capture(&repo, &c, 1);
    let current = capture(&repo, &c, 2);
    assert!(matches!(
        bind_candidate(&c, Some(&first), Some(&current)).unwrap(),
        BindingDecision::Compatible(_)
    ));
    let p = VerificationProfile::new(
        Provenance::new(
            "caller-reported history reasoning",
            "canonical record references",
            "synthetic reasoning; no check executed",
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
            first.clone(),
            p.clone(),
            ObservationTimes::new(UNIX_EPOCH, UNIX_EPOCH).unwrap(),
            ObservationKind::Inference,
            "A recorded requirement is historical information, not a verified outcome",
        )
        .unwrap(),
    );
    let original_evidence = evidence.clone();
    let mut h = CanonicalHistory::new(first.clone());
    h.append(entry(first.id(), false)).unwrap();
    h.append(entry(first.id(), true)).unwrap();
    let old = h
        .query(
            &RecordId::new("release.requirement").unwrap(),
            &TemporalQuery::new(first.id().clone(), UNIX_EPOCH, UNIX_EPOCH).unwrap(),
        )
        .unwrap();
    assert_eq!(old.record().unwrap().id().as_str(), "v1");
    assert_eq!(old.context(), &first);
    assert_eq!(evidence, original_evidence);
    assert!(evidence.assessment().is_none());
    assert_eq!(
        evidence.assess(
            AssessmentId::new("unsupported-pass").unwrap(),
            UNIX_EPOCH,
            AssessmentResult::Pass,
            Applicability::Applicable,
            "History and compatibility alone cannot establish success",
            &ValidationContext::new(&c, Some(&current), Some(&p))
        ),
        Err(SemanticError::UnsupportedResult)
    );
    assert_eq!(evidence, original_evidence);
    assert_eq!(capture(&repo, &c, 3).state(), first.state());
    assert_eq!(snapshot(&repo), before);
}

#[test]
fn same_head_dirty_captures_preserve_staged_unstaged_untracked_and_reject_scope_aliases() {
    let repo = Repo::new();
    repo.commit_file("tracked", "original");
    let c = contract(&repo);
    let clean = capture(&repo, &c, 1);
    fs::write(repo.path.join("tracked"), "staged").unwrap();
    repo.git(&["add", "tracked"]);
    fs::write(repo.path.join("tracked"), "unstaged").unwrap();
    fs::write(repo.path.join("untracked"), "untracked").unwrap();
    let before = snapshot(&repo);
    let dirty = capture(&repo, &c, 2);
    assert_eq!(dirty.state().head(), clean.state().head());
    assert!(dirty.state().entries().iter().any(|e| matches!(
        e,
        Entry::Tracked {
            staged: Some(_),
            unstaged: Some(_),
            ..
        }
    )));
    assert!(
        dirty
            .state()
            .entries()
            .iter()
            .any(|e| matches!(e, Entry::Untracked { .. }))
    );
    let mut h = CanonicalHistory::new(dirty.clone());
    h.append(entry(dirty.id(), false)).unwrap();
    let retained = h.clone();
    assert_eq!(
        h.append(entry(clean.id(), true)),
        Err(TemporalError::ScopeMismatch)
    );
    assert_eq!(h, retained);
    assert_eq!(h.context(), &dirty);
    assert_eq!(snapshot(&repo), before);
    // Same status and HEAD still do not identify changed uncommitted bytes.
    fs::write(repo.path.join("tracked"), "different dirty bytes").unwrap();
    let changed_before = snapshot(&repo);
    let changed = capture(&repo, &c, 3);
    assert_eq!(
        h.append(entry(changed.id(), true)),
        Err(TemporalError::ScopeMismatch)
    );
    assert_eq!(h, retained);
    assert_eq!(snapshot(&repo), changed_before);
    assert_eq!(h.context().state().entries(), dirty.state().entries());
}

#[test]
fn real_detached_and_unborn_observations_keep_commit_presence_distinct() {
    let repo = Repo::new();
    repo.commit_file("tracked", "original");
    let c = contract(&repo);
    repo.git(&["checkout", "--detach"]);
    let detached = capture(&repo, &c, 1);
    assert!(
        matches!(detached.state().head(), Head::Detached { commit } if commit == c.candidate().commit())
    );
    let mut h = CanonicalHistory::new(detached.clone());
    h.append(entry(detached.id(), false)).unwrap();
    assert_eq!(h.context(), &detached);
    // Pin an actual retained Git object, then observe an orphan with no HEAD commit.
    repo.git(&["checkout", "--orphan", "new-history"]);
    let before = snapshot(&repo);
    let unborn = capture(&repo, &c, 2);
    assert!(matches!(unborn.state().head(), Head::Unborn { .. }));
    let mut orphan_history = CanonicalHistory::new(unborn.clone());
    orphan_history.append(entry(unborn.id(), false)).unwrap();
    assert!(matches!(
        orphan_history.context().state().head(),
        Head::Unborn { .. }
    ));
    assert_eq!(orphan_history.context(), &unborn);
    assert_eq!(snapshot(&repo), before);
    assert_eq!(h.context(), &detached);
}
