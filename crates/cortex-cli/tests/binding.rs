//! CAP-CORE-002 cross-crate acceptance through the real 002-A producer.

mod support;
use cortex_cli::inspect_candidate;
use cortex_core::{CandidateId, TaskContract, TaskId, binding::*};
use std::{fs, path::PathBuf};
use support::Repo;

fn id(sequence: u64) -> ObservationId {
    ObservationId::new("real-git-fixture", sequence).unwrap()
}
fn fixture_contract(repo: &Repo, policy: WorktreeRequirement) -> CandidateBindingContract {
    let root = repo.path.canonicalize().unwrap();
    let git_dir = PathBuf::from(
        String::from_utf8(repo.git(&["rev-parse", "--absolute-git-dir"]).stdout)
            .unwrap()
            .trim_end_matches('\n'),
    );
    let common_dir = PathBuf::from(
        String::from_utf8(
            repo.git(&["rev-parse", "--path-format=absolute", "--git-common-dir"])
                .stdout,
        )
        .unwrap()
        .trim_end_matches('\n'),
    );
    let head = repo
        .command()
        .args(["rev-parse", "--verify", "HEAD"])
        .output()
        .unwrap();
    let commit = if head.status.success() {
        CandidateId::from_git_commit(String::from_utf8(head.stdout).unwrap().trim()).unwrap()
    } else {
        CandidateId::from_git_commit(&"a".repeat(40)).unwrap()
    };
    CandidateBindingContract::new(
        TaskContract::new(
            TaskId::new("M002B.integration").unwrap(),
            "Bind the observed repository",
            "Reject stale or insufficient observations",
        )
        .unwrap(),
        ContractRevision::new(1).unwrap(),
        CandidateRequirement::new(
            RepositoryIdentity::new(root, git_dir, common_dir).unwrap(),
            commit,
            policy,
        ),
    )
}
fn capture(
    repo: &Repo,
    contract: &CandidateBindingContract,
    sequence: u64,
) -> CandidateObservation {
    inspect_candidate(&repo.path, contract, id(sequence)).unwrap()
}

#[test]
fn real_clean_checkout_binds_with_actual_commit_paths_and_coverage() {
    let repo = Repo::new();
    repo.commit_file("file", "initial");
    let contract = fixture_contract(&repo, WorktreeRequirement::Clean);
    let first = capture(&repo, &contract, 1);
    let latest = capture(&repo, &contract, 2);
    let decision = bind_candidate(&contract, Some(&first), Some(&latest)).unwrap();
    let BindingDecision::Compatible(bound) = &decision else {
        panic!("{decision:?}");
    };
    assert_eq!(bound.observation().contract(), &contract);
    assert_eq!(
        bound.observation().state().repository().unwrap().root(),
        repo.path.canonicalize().unwrap()
    );
    assert_eq!(
        bound.observation().state().coverage(),
        ObservationCoverage::GitStatus
    );
    assert_eq!(
        bound.observation().state().consistency(),
        ObservationConsistency::TwoMatchingSamples
    );
    assert_eq!(
        bind_candidate(&contract, Some(&first), Some(&latest)).unwrap(),
        decision
    );
    let version = String::from_utf8(repo.git(&["--version"]).stdout).unwrap();
    let profile = format!(
        "002-A-v1:{}:{}-{}",
        version.trim(),
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    assert_eq!(bound.observation().basis().unwrap().profile(), profile);
}

#[test]
fn staged_unstaged_untracked_and_mixed_remain_distinct_at_core_boundary() {
    for (staged, unstaged, untracked) in [
        (true, false, false),
        (false, true, false),
        (false, false, true),
        (true, true, true),
    ] {
        let repo = Repo::new();
        repo.commit_file("file", "initial");
        if staged {
            fs::write(repo.path.join("file"), "staged").unwrap();
            repo.git(&["add", "file"]);
        }
        if unstaged {
            fs::write(repo.path.join("file"), "unstaged").unwrap();
        }
        if untracked {
            fs::write(repo.path.join("new"), "untracked").unwrap();
        }
        let contract = fixture_contract(&repo, WorktreeRequirement::AllowChanges);
        let first = capture(&repo, &contract, 1);
        let latest = capture(&repo, &contract, 2);
        let entries = latest.state().entries();
        assert_eq!(
            entries
                .iter()
                .filter(|e| matches!(
                    e,
                    Entry::Tracked {
                        staged: Some(_),
                        ..
                    }
                ))
                .count(),
            usize::from(staged)
        );
        assert_eq!(
            entries
                .iter()
                .filter(|e| matches!(
                    e,
                    Entry::Tracked {
                        unstaged: Some(_),
                        ..
                    }
                ))
                .count(),
            usize::from(unstaged)
        );
        assert_eq!(
            entries
                .iter()
                .filter(|e| matches!(e, Entry::Untracked { .. }))
                .count(),
            usize::from(untracked)
        );
        assert_eq!(
            bind_candidate(&contract, Some(&first), Some(&latest)).unwrap(),
            BindingDecision::Insufficient(BindingInsufficiency::UncommittedIdentity)
        );
    }
}

#[test]
fn same_head_new_untracked_file_invalidates_clean_observation() {
    let repo = Repo::new();
    repo.commit_file("file", "initial");
    let contract = fixture_contract(&repo, WorktreeRequirement::Clean);
    let first = capture(&repo, &contract, 1);
    fs::write(repo.path.join("new"), "new").unwrap();
    let latest = capture(&repo, &contract, 2);
    assert_eq!(first.state().head(), latest.state().head());
    assert_eq!(
        bind_candidate(&contract, Some(&first), Some(&latest)).unwrap(),
        BindingDecision::Stale(BindingStaleness::ObservedStateChanged)
    );
}

#[test]
fn actual_dirty_byte_edits_with_identical_status_stay_insufficient() {
    for tracked in [false, true] {
        let repo = Repo::new();
        repo.commit_file("file", "initial");
        let name = if tracked { "file" } else { "new" };
        fs::write(repo.path.join(name), "dirty one").unwrap();
        let contract = fixture_contract(&repo, WorktreeRequirement::AllowChanges);
        let first = capture(&repo, &contract, 1);
        fs::write(repo.path.join(name), "dirty two with different bytes").unwrap();
        let latest = capture(&repo, &contract, 2);
        assert_eq!(first.state(), latest.state());
        assert_eq!(first.basis(), latest.basis());
        assert_eq!(
            bind_candidate(&contract, Some(&first), Some(&latest)).unwrap(),
            BindingDecision::Insufficient(BindingInsufficiency::UncommittedIdentity)
        );
    }
}

#[test]
fn changed_staged_blob_is_visible_even_when_status_categories_match() {
    let repo = Repo::new();
    repo.commit_file("file", "initial");
    fs::write(repo.path.join("file"), "staged one").unwrap();
    repo.git(&["add", "file"]);
    let contract = fixture_contract(&repo, WorktreeRequirement::AllowChanges);
    let first = capture(&repo, &contract, 1);
    fs::write(repo.path.join("file"), "staged two").unwrap();
    repo.git(&["add", "file"]);
    let latest = capture(&repo, &contract, 2);
    assert_eq!(first.state(), latest.state());
    assert_ne!(first.basis(), latest.basis());
    assert_eq!(
        bind_candidate(&contract, Some(&first), Some(&latest)).unwrap(),
        BindingDecision::Stale(BindingStaleness::ObservedStateChanged)
    );
}

#[test]
fn changed_repository_configuration_invalidates_matching_clean_status_and_is_redacted() {
    let repo = Repo::new();
    repo.commit_file("file", "initial");
    let contract = fixture_contract(&repo, WorktreeRequirement::Clean);
    let first = capture(&repo, &contract, 1);
    repo.git(&["config", "cortex.private", "private-token-example"]);
    let latest = capture(&repo, &contract, 2);
    assert_eq!(first.state(), latest.state());
    assert!(!format!("{latest:?}").contains("private-token-example"));
    assert_eq!(
        bind_candidate(&contract, Some(&first), Some(&latest)).unwrap(),
        BindingDecision::Stale(BindingStaleness::ObservedStateChanged)
    );
}

#[test]
fn actual_repository_and_commit_mismatches_are_rejected() {
    let repo = Repo::new();
    repo.commit_file("file", "initial");
    let contract = fixture_contract(&repo, WorktreeRequirement::Clean);
    let first = capture(&repo, &contract, 1);
    let other = Repo::new();
    other.commit_file("file", "initial");
    let wrong_repo = capture(&other, &contract, 2);
    assert_eq!(
        bind_candidate(&contract, Some(&first), Some(&wrong_repo)).unwrap(),
        BindingDecision::Incompatible(BindingMismatch::Repository)
    );
    repo.commit_file("file", "changed commit");
    let wrong_commit = capture(&repo, &contract, 2);
    assert_eq!(
        bind_candidate(&contract, Some(&first), Some(&wrong_commit)).unwrap(),
        BindingDecision::Incompatible(BindingMismatch::Commit)
    );
}

#[test]
fn detached_and_unborn_states_cross_boundary_without_fabrication() {
    let repo = Repo::new();
    let contract = fixture_contract(&repo, WorktreeRequirement::Clean);
    let first = capture(&repo, &contract, 1);
    let latest = capture(&repo, &contract, 2);
    assert!(matches!(latest.state().head(),Head::Unborn { branch } if branch==b"main"));
    assert_eq!(
        bind_candidate(&contract, Some(&first), Some(&latest)).unwrap(),
        BindingDecision::Incompatible(BindingMismatch::CommitAbsent)
    );
    repo.commit_file("file", "initial");
    repo.git(&["checkout", "--detach"]);
    let contract = fixture_contract(&repo, WorktreeRequirement::Clean);
    let first = capture(&repo, &contract, 1);
    let latest = capture(&repo, &contract, 2);
    assert!(
        matches!(latest.state().head(),Head::Detached { commit } if commit==contract.candidate().commit())
    );
    assert!(matches!(
        bind_candidate(&contract, Some(&first), Some(&latest)).unwrap(),
        BindingDecision::Compatible(_)
    ));
}

#[test]
fn linked_worktree_identity_is_distinct_even_at_the_same_commit() {
    let repo = Repo::new();
    repo.commit_file("file", "initial");
    let contract = fixture_contract(&repo, WorktreeRequirement::Clean);
    let first = capture(&repo, &contract, 1);
    let linked = repo.path.join("linked");
    repo.git(&["worktree", "add", "-b", "linked", linked.to_str().unwrap()]);
    let latest = inspect_candidate(&linked, &contract, id(2)).unwrap();
    let identity = latest.state().repository().unwrap();
    assert_eq!(
        identity.common_dir(),
        contract.candidate().repository().common_dir()
    );
    assert_ne!(
        identity.git_dir(),
        contract.candidate().repository().git_dir()
    );
    assert_eq!(
        bind_candidate(&contract, Some(&first), Some(&latest)).unwrap(),
        BindingDecision::Incompatible(BindingMismatch::Repository)
    );
}

#[test]
fn inspection_failure_cannot_be_translated_into_a_binding_observation() {
    let repo = Repo::new();
    repo.commit_file("file", "initial");
    let contract = fixture_contract(&repo, WorktreeRequirement::Clean);
    repo.git(&["update-index", "--assume-unchanged", "file"]);
    assert!(matches!(
        inspect_candidate(&repo.path, &contract, id(1)),
        Err(cortex_cli::InspectionError::Unsupported(_))
    ));
    repo.git(&["update-index", "--no-assume-unchanged", "file"]);
    let head = contract.candidate().commit().as_str();
    fs::remove_file(
        repo.path
            .join(".git/objects")
            .join(&head[..2])
            .join(&head[2..]),
    )
    .unwrap();
    assert!(matches!(
        inspect_candidate(&repo.path, &contract, id(2)),
        Err(cortex_cli::InspectionError::GitFailed(_))
    ));
}

#[test]
fn changed_contract_revision_requires_new_applicable_observations() {
    let repo = Repo::new();
    repo.commit_file("file", "initial");
    let old = fixture_contract(&repo, WorktreeRequirement::Clean);
    let baseline = capture(&repo, &old, 1);
    let revised = CandidateBindingContract::new(
        old.task().clone(),
        ContractRevision::new(2).unwrap(),
        old.candidate().clone(),
    );
    let latest = capture(&repo, &revised, 2);
    assert_eq!(
        bind_candidate(&revised, Some(&baseline), Some(&latest)).unwrap(),
        BindingDecision::Stale(BindingStaleness::ContractRevision)
    );
    let applicable = capture(&repo, &revised, 3);
    assert!(matches!(
        bind_candidate(&revised, Some(&latest), Some(&applicable)).unwrap(),
        BindingDecision::Compatible(_)
    ));
}

#[test]
fn typed_adapter_repeated_captures_preserve_refresh_sensitive_index_and_files() {
    let repo = Repo::new();
    repo.commit_file("file", "initial");
    let contract = fixture_contract(&repo, WorktreeRequirement::Clean);
    let file = fs::OpenOptions::new()
        .write(true)
        .open(repo.path.join("file"))
        .unwrap();
    file.set_modified(
        std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000),
    )
    .unwrap();
    drop(file);
    let paths = [
        "file",
        ".git/index",
        ".git/config",
        ".git/HEAD",
        ".git/refs/heads/main",
    ];
    let before: Vec<_> = paths
        .iter()
        .map(|path| {
            let p = repo.path.join(path);
            (
                fs::read(&p).unwrap(),
                fs::metadata(p).unwrap().modified().unwrap(),
            )
        })
        .collect();
    let first = capture(&repo, &contract, 1);
    let latest = capture(&repo, &contract, 2);
    assert!(matches!(
        bind_candidate(&contract, Some(&first), Some(&latest)).unwrap(),
        BindingDecision::Compatible(_)
    ));
    let after: Vec<_> = paths
        .iter()
        .map(|path| {
            let p = repo.path.join(path);
            (
                fs::read(&p).unwrap(),
                fs::metadata(p).unwrap().modified().unwrap(),
            )
        })
        .collect();
    assert_eq!(before, after);
}

#[test]
fn sha256_candidate_binds_through_the_actual_adapter() {
    let repo = Repo::new();
    repo.git(&[
        "init",
        "--object-format=sha256",
        "--initial-branch=main",
        "sha256",
    ]);
    let nested = Repo {
        path: repo.path.join("sha256"),
    };
    nested.git(&["config", "user.name", "Cortex Test"]);
    nested.git(&["config", "user.email", "cortex-test@example.invalid"]);
    nested.commit_file("file", "initial");
    let contract = fixture_contract(&nested, WorktreeRequirement::Clean);
    assert_eq!(contract.candidate().commit().as_str().len(), 64);
    let first = capture(&nested, &contract, 1);
    let latest = capture(&nested, &contract, 2);
    assert!(matches!(
        bind_candidate(&contract, Some(&first), Some(&latest)).unwrap(),
        BindingDecision::Compatible(_)
    ));
}

#[cfg(unix)]
#[test]
fn non_utf8_repository_and_entry_paths_survive_the_typed_boundary() {
    use std::{
        ffi::{OsStr, OsString},
        os::unix::ffi::OsStringExt,
    };
    let parent = Repo::new();
    let path = parent.path.join(OsString::from_vec(b"repo-\xff".to_vec()));
    assert!(
        parent
            .command()
            .args([
                OsStr::new("init"),
                OsStr::new("--initial-branch=main"),
                path.as_os_str()
            ])
            .output()
            .unwrap()
            .status
            .success()
    );
    let repo = Repo { path };
    repo.git(&["config", "user.name", "Cortex Test"]);
    repo.git(&["config", "user.email", "cortex-test@example.invalid"]);
    repo.commit_file("file", "initial");
    let root = repo.path.canonicalize().unwrap();
    let commit = CandidateId::from_git_commit(
        String::from_utf8(repo.git(&["rev-parse", "HEAD"]).stdout)
            .unwrap()
            .trim(),
    )
    .unwrap();
    let contract = CandidateBindingContract::new(
        TaskContract::new(
            TaskId::new("M002B.bytes").unwrap(),
            "Preserve path bytes",
            "No lossy conversion at binding boundary",
        )
        .unwrap(),
        ContractRevision::new(1).unwrap(),
        CandidateRequirement::new(
            RepositoryIdentity::new(root.clone(), root.join(".git"), root.join(".git")).unwrap(),
            commit,
            WorktreeRequirement::AllowChanges,
        ),
    );
    let name = b"new-\xff\n".to_vec();
    fs::write(repo.path.join(OsString::from_vec(name.clone())), "new").unwrap();
    let first = capture(&repo, &contract, 1);
    let latest = capture(&repo, &contract, 2);
    assert_eq!(latest.state().entries()[0].path(), name);
    assert_eq!(latest.state().repository().unwrap().root(), root);
    assert_eq!(
        bind_candidate(&contract, Some(&first), Some(&latest)).unwrap(),
        BindingDecision::Insufficient(BindingInsufficiency::UncommittedIdentity)
    );
}
