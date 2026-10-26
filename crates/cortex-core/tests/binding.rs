//! CAP-CORE-002 positive/negative domain behavior; no filesystem or Git needed.

use cortex_core::{CandidateId, TaskContract, TaskId, binding::*};
use std::path::PathBuf;

fn repository(name: &str) -> RepositoryIdentity {
    let root = std::env::temp_dir().join(name);
    RepositoryIdentity::new(root.clone(), root.join(".git"), root.join(".git")).unwrap()
}
fn commit(byte: &str) -> CandidateId {
    CandidateId::from_git_commit(&byte.repeat(40)).unwrap()
}
fn contract() -> CandidateBindingContract {
    CandidateBindingContract::new(
        TaskContract::new(
            TaskId::new("M002.binding").unwrap(),
            "Bind the actual candidate",
            "Reject mismatched observations",
        )
        .unwrap(),
        ContractRevision::new(1).unwrap(),
        CandidateRequirement::new(
            repository("cortex-domain"),
            commit("a"),
            WorktreeRequirement::Clean,
        ),
    )
}
fn with_revision(c: &CandidateBindingContract, revision: u64) -> CandidateBindingContract {
    CandidateBindingContract::new(
        c.task().clone(),
        ContractRevision::new(revision).unwrap(),
        c.candidate().clone(),
    )
}
fn dirty_policy(c: &CandidateBindingContract) -> CandidateBindingContract {
    CandidateBindingContract::new(
        c.task().clone(),
        c.revision(),
        CandidateRequirement::new(
            c.candidate().repository().clone(),
            c.candidate().commit().clone(),
            WorktreeRequirement::AllowChanges,
        ),
    )
}
fn state(c: &CandidateBindingContract, entries: Vec<Entry>) -> RepositoryState {
    RepositoryState::new(
        Some(c.candidate().repository().clone()),
        Head::Attached {
            branch: b"main".to_vec(),
            commit: c.candidate().commit().clone(),
        },
        entries,
        ObservationCoverage::GitStatus,
        ObservationConsistency::TwoMatchingSamples,
    )
    .unwrap()
}
fn observed(
    c: &CandidateBindingContract,
    sequence: u64,
    state: RepositoryState,
) -> CandidateObservation {
    CandidateObservation::new(
        ObservationId::new("test-producer", sequence).unwrap(),
        c,
        state,
        Some(ComparisonBasis::new("test-profile", b"fixed comparison material".to_vec()).unwrap()),
    )
}
fn decide(c: &CandidateBindingContract, state: RepositoryState) -> BindingDecision {
    bind_candidate(
        c,
        Some(&observed(c, 1, state.clone())),
        Some(&observed(c, 2, state)),
    )
    .unwrap()
}
fn untracked(path: &str) -> Entry {
    Entry::Untracked {
        path: path.as_bytes().to_vec(),
    }
}

#[test]
fn matching_committed_candidate_retains_full_context_and_is_deterministic() {
    let c = contract();
    let first = observed(&c, 1, state(&c, vec![]));
    let latest = observed(&c, 2, state(&c, vec![]));
    let decision = bind_candidate(&c, Some(&first), Some(&latest)).unwrap();
    assert_eq!(
        bind_candidate(&c, Some(&first), Some(&latest)).unwrap(),
        decision
    );
    let BindingDecision::Compatible(bound) = decision else {
        panic!("matching clean candidate must bind");
    };
    assert_eq!(bound.baseline(), first.id());
    assert_eq!(bound.observation(), &latest);
    assert_eq!(bound.observation().contract(), &c);
    assert_eq!(
        bound.observation().state().coverage(),
        ObservationCoverage::GitStatus
    );
    assert_eq!(
        bound.observation().state().consistency(),
        ObservationConsistency::TwoMatchingSamples
    );
}

#[test]
fn different_commit_and_repository_are_demonstrated_mismatches() {
    let c = contract();
    for (repository, head, reason) in [
        (
            Some(c.candidate().repository().clone()),
            Head::Detached {
                commit: commit("b"),
            },
            BindingMismatch::Commit,
        ),
        (
            Some(repository("another-repository")),
            state(&c, vec![]).head().clone(),
            BindingMismatch::Repository,
        ),
    ] {
        let report = RepositoryState::new(
            repository,
            head,
            vec![],
            ObservationCoverage::GitStatus,
            ObservationConsistency::TwoMatchingSamples,
        )
        .unwrap();
        assert_eq!(decide(&c, report), BindingDecision::Incompatible(reason));
    }
}

#[test]
fn unknown_identity_is_insufficient_and_unborn_explicitly_lacks_commit() {
    let c = contract();
    for (repository, head, expected) in [
        (
            None,
            Head::Unknown,
            BindingDecision::Insufficient(BindingInsufficiency::MissingRepository),
        ),
        (
            Some(c.candidate().repository().clone()),
            Head::Unknown,
            BindingDecision::Insufficient(BindingInsufficiency::MissingCommit),
        ),
        (
            Some(c.candidate().repository().clone()),
            Head::Unborn {
                branch: b"new".to_vec(),
            },
            BindingDecision::Incompatible(BindingMismatch::CommitAbsent),
        ),
    ] {
        let report = RepositoryState::new(
            repository,
            head,
            vec![],
            ObservationCoverage::GitStatus,
            ObservationConsistency::TwoMatchingSamples,
        )
        .unwrap();
        assert_eq!(decide(&c, report), expected);
    }
}

#[test]
fn detached_head_can_bind_without_a_branch() {
    let c = contract();
    let report = RepositoryState::new(
        Some(c.candidate().repository().clone()),
        Head::Detached {
            commit: c.candidate().commit().clone(),
        },
        vec![],
        ObservationCoverage::GitStatus,
        ObservationConsistency::TwoMatchingSamples,
    )
    .unwrap();
    let BindingDecision::Compatible(bound) = decide(&c, report) else {
        panic!("detached commit must bind");
    };
    assert!(matches!(
        bound.observation().state().head(),
        Head::Detached { .. }
    ));
}

#[test]
fn dirty_categories_survive_and_matching_dirty_status_cannot_prove_identity() {
    let clean = contract();
    let allow = dirty_policy(&clean);
    let entries = vec![
        Entry::Tracked {
            path: b"both".to_vec(),
            staged: Some(Change::Added),
            unstaged: Some(Change::Modified),
        },
        untracked("new"),
    ];
    let report = state(&clean, entries.clone());
    assert_eq!(report.entries(), entries);
    assert_eq!(
        decide(&clean, report.clone()),
        BindingDecision::Incompatible(BindingMismatch::WorkingTree)
    );
    assert_eq!(
        decide(&allow, report),
        BindingDecision::Insufficient(BindingInsufficiency::UncommittedIdentity)
    );
}

#[test]
fn same_head_changed_working_state_is_stale_in_both_directions() {
    let c = contract();
    let clean = state(&c, vec![]);
    let dirty = state(&c, vec![untracked("new")]);
    for (a, b) in [(clean.clone(), dirty.clone()), (dirty, clean)] {
        assert_eq!(
            bind_candidate(&c, Some(&observed(&c, 1, a)), Some(&observed(&c, 2, b))).unwrap(),
            BindingDecision::Stale(BindingStaleness::ObservedStateChanged)
        );
    }
}

#[test]
fn changed_comparison_material_or_profile_invalidates_even_clean_status() {
    let c = contract();
    let first = observed(&c, 1, state(&c, vec![]));
    for (profile, material) in [
        ("test-profile", b"changed index/config".to_vec()),
        ("another-profile", b"fixed comparison material".to_vec()),
    ] {
        let latest = CandidateObservation::new(
            ObservationId::new("test-producer", 2).unwrap(),
            &c,
            state(&c, vec![]),
            Some(ComparisonBasis::new(profile, material).unwrap()),
        );
        assert_eq!(
            bind_candidate(&c, Some(&first), Some(&latest)).unwrap(),
            BindingDecision::Stale(BindingStaleness::ObservedStateChanged)
        );
    }
}

#[test]
fn baseline_and_reobservation_are_required_and_cannot_reuse_order() {
    let c = contract();
    let first = observed(&c, 2, state(&c, vec![]));
    assert_eq!(
        bind_candidate(&c, None, None).unwrap(),
        BindingDecision::Insufficient(BindingInsufficiency::MissingObservation)
    );
    assert_eq!(
        bind_candidate(&c, Some(&first), None).unwrap(),
        BindingDecision::Insufficient(BindingInsufficiency::ReobservationRequired)
    );
    for sequence in [1, 2] {
        assert_eq!(
            bind_candidate(
                &c,
                Some(&first),
                Some(&observed(&c, sequence, state(&c, vec![])))
            )
            .unwrap(),
            BindingDecision::Stale(BindingStaleness::ObservationOrder)
        );
    }
    let other = CandidateObservation::new(
        ObservationId::new("other-series", 3).unwrap(),
        &c,
        state(&c, vec![]),
        first.basis().cloned(),
    );
    assert_eq!(
        bind_candidate(&c, Some(&first), Some(&other)).unwrap(),
        BindingDecision::Insufficient(BindingInsufficiency::DifferentObservationScope)
    );
}

#[test]
fn incomplete_uncertain_or_missing_comparison_material_never_binds() {
    let c = contract();
    let complete = observed(&c, 1, state(&c, vec![]));
    for (coverage, consistency, basis, expected) in [
        (
            ObservationCoverage::Incomplete,
            ObservationConsistency::TwoMatchingSamples,
            complete.basis().cloned(),
            BindingInsufficiency::IncompleteCoverage,
        ),
        (
            ObservationCoverage::GitStatus,
            ObservationConsistency::Indeterminate,
            complete.basis().cloned(),
            BindingInsufficiency::IndeterminateConsistency,
        ),
        (
            ObservationCoverage::GitStatus,
            ObservationConsistency::TwoMatchingSamples,
            None,
            BindingInsufficiency::MissingComparisonBasis,
        ),
    ] {
        let report = RepositoryState::new(
            Some(c.candidate().repository().clone()),
            state(&c, vec![]).head().clone(),
            vec![],
            coverage,
            consistency,
        )
        .unwrap();
        let limited = CandidateObservation::new(
            ObservationId::new("test-producer", 2).unwrap(),
            &c,
            report,
            basis,
        );
        assert_eq!(
            bind_candidate(&c, Some(&complete), Some(&limited)).unwrap(),
            BindingDecision::Insufficient(expected)
        );
        let early = CandidateObservation::new(
            ObservationId::new("test-producer", 1).unwrap(),
            &c,
            limited.state().clone(),
            limited.basis().cloned(),
        );
        let late = observed(&c, 2, state(&c, vec![]));
        assert_eq!(
            bind_candidate(&c, Some(&early), Some(&late)).unwrap(),
            BindingDecision::Insufficient(expected)
        );
    }
}

#[test]
fn different_contract_revisions_are_stale_regardless_of_numeric_direction() {
    let c = contract();
    for (expected, captured) in [
        (with_revision(&c, 2), c.clone()),
        (c.clone(), with_revision(&c, 2)),
    ] {
        assert_eq!(
            bind_candidate(
                &expected,
                Some(&observed(&captured, 1, state(&captured, vec![]))),
                Some(&observed(&captured, 2, state(&captured, vec![])))
            )
            .unwrap(),
            BindingDecision::Stale(BindingStaleness::ContractRevision)
        );
    }
}

#[test]
fn task_identity_and_same_revision_aliasing_are_distinct() {
    let c = contract();
    let other_task = CandidateBindingContract::new(
        TaskContract::new(
            TaskId::new("another-task").unwrap(),
            c.task().objective(),
            c.task().acceptance(),
        )
        .unwrap(),
        c.revision(),
        c.candidate().clone(),
    );
    assert_eq!(
        bind_candidate(&c, Some(&observed(&other_task, 1, state(&c, vec![]))), None).unwrap(),
        BindingDecision::Incompatible(BindingMismatch::TaskIdentity)
    );
    for changed in [
        CandidateBindingContract::new(
            TaskContract::new(
                c.task().id().clone(),
                "Changed objective",
                c.task().acceptance(),
            )
            .unwrap(),
            c.revision(),
            c.candidate().clone(),
        ),
        CandidateBindingContract::new(
            TaskContract::new(
                c.task().id().clone(),
                c.task().objective(),
                "Changed criterion",
            )
            .unwrap(),
            c.revision(),
            c.candidate().clone(),
        ),
        dirty_policy(&c),
        CandidateBindingContract::new(
            c.task().clone(),
            c.revision(),
            CandidateRequirement::new(
                repository("different"),
                commit("a"),
                WorktreeRequirement::Clean,
            ),
        ),
        CandidateBindingContract::new(
            c.task().clone(),
            c.revision(),
            CandidateRequirement::new(
                c.candidate().repository().clone(),
                commit("b"),
                WorktreeRequirement::Clean,
            ),
        ),
    ] {
        let stale = observed(&changed, 1, state(&c, vec![]));
        let current = observed(&c, 2, state(&c, vec![]));
        assert_eq!(
            bind_candidate(&c, Some(&stale), Some(&current)),
            Err(BindingError::ContractRevisionReused)
        );
        let current = observed(&c, 1, state(&c, vec![]));
        let stale = observed(&changed, 2, state(&c, vec![]));
        assert_eq!(
            bind_candidate(&c, Some(&current), Some(&stale)),
            Err(BindingError::ContractRevisionReused)
        );
    }
}

#[test]
fn malformed_ids_and_revisions_are_rejected_without_echoing_them() {
    assert_eq!(
        ContractRevision::new(0),
        Err(BindingError::InvalidContractRevision)
    );
    assert_eq!(ContractRevision::new(u64::MAX).unwrap().get(), u64::MAX);
    for scope in [
        "".to_owned(),
        "-bad".into(),
        "private-token\n".into(),
        "x".repeat(129),
        "ü".into(),
    ] {
        let error = ObservationId::new(&scope, 1).unwrap_err();
        assert_eq!(error, BindingError::InvalidObservationId);
        assert!(!error.to_string().contains("private-token"));
    }
    assert!(ObservationId::new("x", 0).is_err());
    let id = ObservationId::new(&"x".repeat(128), u64::MAX).unwrap();
    assert_eq!(id.sequence(), u64::MAX);
    assert_eq!(id.scope().len(), 128);
    assert!(TaskId::new("bad/task").is_err());
    assert!(CandidateId::from_git_commit("missing").is_err());
}

#[test]
fn malformed_repository_branch_and_change_inputs_are_rejected() {
    let c = contract();
    for root in [
        PathBuf::new(),
        PathBuf::from("relative"),
        std::env::temp_dir().join("a/../b"),
    ] {
        assert_eq!(
            RepositoryIdentity::new(root, PathBuf::from("/git"), PathBuf::from("/common")),
            Err(BindingError::InvalidRepositoryIdentity)
        );
    }
    for branch in [vec![], b"private-token\n".to_vec()] {
        assert_eq!(
            RepositoryState::new(
                None,
                Head::Unborn { branch },
                vec![],
                ObservationCoverage::Incomplete,
                ObservationConsistency::Indeterminate
            ),
            Err(BindingError::InvalidHead)
        );
    }
    for path in [
        b"".as_slice(),
        b"/absolute",
        b"a/../b",
        b"./a",
        b"a//b",
        b"a/",
        b"secret\0",
    ] {
        assert_eq!(
            RepositoryState::new(
                None,
                Head::Unknown,
                vec![Entry::Untracked {
                    path: path.to_vec()
                }],
                ObservationCoverage::Incomplete,
                ObservationConsistency::Indeterminate
            ),
            Err(BindingError::InvalidEntry)
        );
    }
    assert_eq!(
        RepositoryState::new(
            None,
            Head::Unknown,
            vec![Entry::Tracked {
                path: b"x".to_vec(),
                staged: None,
                unstaged: None
            }],
            ObservationCoverage::Incomplete,
            ObservationConsistency::Indeterminate
        ),
        Err(BindingError::InvalidEntry)
    );
    assert_eq!(
        RepositoryState::new(
            None,
            Head::Unknown,
            vec![untracked("same"), untracked("same")],
            ObservationCoverage::Incomplete,
            ObservationConsistency::Indeterminate
        ),
        Err(BindingError::DuplicateEntry)
    );
    let report = state(&c, vec![untracked("z"), untracked("a\nü")]);
    assert_eq!(report.entries()[0].path(), "a\nü".as_bytes());
}

#[test]
fn observation_limits_reject_overflow_and_comparison_debug_redacts_material() {
    let c = contract();
    let entries: Vec<_> = (0..MAX_OBSERVATION_ENTRIES)
        .map(|n| untracked(&format!("f{n}")))
        .collect();
    assert_eq!(
        state(&c, entries.clone()).entries().len(),
        MAX_OBSERVATION_ENTRIES
    );
    let mut too_many = entries;
    too_many.push(untracked("overflow"));
    assert_eq!(
        RepositoryState::new(
            None,
            Head::Unknown,
            too_many,
            ObservationCoverage::Incomplete,
            ObservationConsistency::Indeterminate
        ),
        Err(BindingError::ObservationLimit)
    );
    let exact = Entry::Untracked {
        path: vec![b'x'; MAX_OBSERVATION_BYTES],
    };
    assert!(
        RepositoryState::new(
            None,
            Head::Unknown,
            vec![exact],
            ObservationCoverage::Incomplete,
            ObservationConsistency::Indeterminate
        )
        .is_ok()
    );
    assert_eq!(
        RepositoryState::new(
            None,
            Head::Unknown,
            vec![Entry::Untracked {
                path: vec![b'x'; MAX_OBSERVATION_BYTES + 1]
            }],
            ObservationCoverage::Incomplete,
            ObservationConsistency::Indeterminate
        ),
        Err(BindingError::ObservationLimit)
    );
    assert!(ComparisonBasis::new("", vec![1]).is_err());
    assert!(ComparisonBasis::new(&"x".repeat(4096), vec![1]).is_ok());
    assert_eq!(
        ComparisonBasis::new(&"x".repeat(4097), vec![1]),
        Err(BindingError::ObservationLimit)
    );
    assert!(ComparisonBasis::new("profile", vec![1; 3 * MAX_OBSERVATION_BYTES + 24]).is_ok());
    assert!(ComparisonBasis::new("profile", vec![]).is_err());
    assert!(ComparisonBasis::new("profile", vec![1; 3 * MAX_OBSERVATION_BYTES + 25]).is_err());
    let basis = ComparisonBasis::new("profile", b"private-token-example".to_vec()).unwrap();
    assert!(!format!("{basis:?}").contains("private-token-example"));
}
