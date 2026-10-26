//! CAP-EVD-002 real Git -> 002-A adapter -> 002-B binding -> evidence integration.

#[path = "../../cortex-cli/tests/support/mod.rs"]
mod git_fixture;

use cortex_cli::inspect_candidate;
use cortex_core::{CandidateId, TaskContract, TaskId, binding::*};
use cortex_evidence::{ExecutionObservation, Provenance, unified::*};
use git_fixture::Repo;
use std::{collections::BTreeMap, fs, path::PathBuf, time::SystemTime};

fn contract(repo: &Repo) -> CandidateBindingContract {
    let root = repo.path.canonicalize().unwrap();
    let head = String::from_utf8(repo.git(&["rev-parse", "--verify", "HEAD"]).stdout).unwrap();
    CandidateBindingContract::new(
        TaskContract::new(
            TaskId::new("M002C.real-git").unwrap(),
            "Evaluate a bounded revision query",
            "The actual Git HEAD query must equal the required committed revision",
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
        ObservationId::new("repository-captures", sequence).unwrap(),
    )
    .unwrap()
}
fn profile(repo: &Repo) -> VerificationProfile {
    let version = String::from_utf8(repo.git(&["--version"]).stdout).unwrap();
    VerificationProfile::new(
        Provenance::new(
            "Git rev-parse oracle fixture",
            "--verify HEAD; compare stdout to the exact retained required commit",
            "local temporary repository; system/global Git configuration disabled",
        )
        .unwrap(),
        version.trim(),
        &format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
        &[],
    )
    .unwrap()
}
fn report(
    repo: &Repo,
    c: &CandidateBindingContract,
    subject: CandidateObservation,
    sequence: u64,
    p: &VerificationProfile,
) -> EvidenceRecord {
    // This is a local test mechanism, not a product runner. The actual subprocess
    // observation and explicit comparison are independent from the final assessment.
    let output = repo
        .command()
        .args(["--no-optional-locks", "rev-parse", "--verify", "HEAD"])
        .output()
        .unwrap();
    let observed = SystemTime::now();
    let conclusion = if output.status.success() {
        if String::from_utf8(output.stdout).unwrap().trim() == c.candidate().commit().as_str() {
            OracleConclusion::Satisfied
        } else {
            OracleConclusion::Violated
        }
    } else {
        OracleConclusion::Insufficient
    };
    EvidenceRecord::new(
        ReportedObservation::new(
            ObservationId::new("verification-reports", sequence).unwrap(),
            subject,
            p.clone(),
            ObservationTimes::new(observed, SystemTime::now()).unwrap(),
            ObservationKind::Execution(
                VerificationOutcome::new(
                    ExecutionObservation::Exited {
                        code: u32::try_from(output.status.code().unwrap()).unwrap(),
                    },
                    1,
                    Some(OracleEvaluation::new(conclusion, "Compare actual query output to the exact required revision; failed queries provide insufficient support").unwrap()),
                )
                .unwrap(),
            ),
            "Actual local query outcome; reports are still caller-supplied, not authenticated",
        )
        .unwrap(),
    )
}
fn assess(
    r: &mut EvidenceRecord,
    c: &CandidateBindingContract,
    current: &CandidateObservation,
    p: &VerificationProfile,
    result: AssessmentResult,
) -> Result<(), SemanticError> {
    r.assess(
        AssessmentId::new("fixture-assessment").unwrap(),
        SystemTime::now(),
        result,
        Applicability::Applicable,
        "Explicit comparison supports only this bounded revision-query criterion",
        &ValidationContext::new(c, Some(current), Some(p)),
    )
}
// Content, modification time, length and read-only permissions for every fixture
// file/directory, including .git objects, refs, logs, index and configuration.
// Access times and platform-specific inode/change/birth metadata are not compared here.
fn snapshot(repo: &Repo) -> BTreeMap<PathBuf, (Vec<u8>, SystemTime, u64, bool)> {
    let mut result = BTreeMap::new();
    let mut pending = vec![repo.path.clone()];
    while let Some(path) = pending.pop() {
        let meta = fs::symlink_metadata(&path).unwrap();
        assert!(!meta.is_symlink());
        let bytes = if meta.is_dir() {
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
                bytes,
                meta.modified().unwrap(),
                meta.len(),
                meta.permissions().readonly(),
            ),
        );
    }
    result
}

#[test]
fn real_clean_evidence_retains_actual_profile_times_context_and_read_only_state() {
    let repo = Repo::new();
    repo.commit_file("file", "initial");
    let c = contract(&repo);
    let p = profile(&repo);
    let before = snapshot(&repo);
    let first = capture(&repo, &c, 1);
    let mut r = report(&repo, &c, first, 1, &p);
    assert!(r.assessment().is_none());
    let current = capture(&repo, &c, 2);
    assess(&mut r, &c, &current, &p, AssessmentResult::Pass).unwrap();
    assert_eq!(
        r.assessment().unwrap().candidate_at_assessment(),
        Some(&current)
    );
    assert_eq!(r.assessment().unwrap().profile_at_assessment(), Some(&p));
    assert_eq!(r.observation().profile().version(), p.version());
    assert!(r.observation().times().observed() <= r.observation().times().recorded());
    for sequence in 3..=4 {
        let latest = capture(&repo, &c, sequence);
        assert!(
            r.inspect(&ValidationContext::new(&c, Some(&latest), Some(&p)))
                .current_pass()
        );
    }
    assert_eq!(snapshot(&repo), before);
}

#[test]
fn changed_real_candidate_invalidates_pass_and_revalidation_needs_a_new_report() {
    let repo = Repo::new();
    repo.commit_file("file", "initial");
    let c = contract(&repo);
    let p = profile(&repo);
    let mut r = report(&repo, &c, capture(&repo, &c, 1), 1, &p);
    assess(
        &mut r,
        &c,
        &capture(&repo, &c, 2),
        &p,
        AssessmentResult::Pass,
    )
    .unwrap();
    fs::write(repo.path.join("file"), "changed bytes").unwrap();
    let changed = capture(&repo, &c, 3);
    let view = r.inspect(&ValidationContext::new(&c, Some(&changed), Some(&p)));
    assert_eq!(view.result(), Some(AssessmentResult::Pass));
    assert_eq!(view.validity(), Some(Validity::Stale));
    assert!(!view.current_pass());
    fs::write(repo.path.join("file"), "initial").unwrap();
    let restored = capture(&repo, &c, 4);
    assert!(
        !r.inspect(&ValidationContext::new(&c, Some(&restored), Some(&p)))
            .current_pass()
    );
    let mut new_record = report(&repo, &c, restored, 2, &p);
    let latest = capture(&repo, &c, 5);
    assess(&mut new_record, &c, &latest, &p, AssessmentResult::Pass).unwrap();
    assert!(
        new_record
            .inspect(&ValidationContext::new(&c, Some(&latest), Some(&p)))
            .current_pass()
    );
}

#[test]
fn dirty_candidates_keep_categories_and_cannot_support_current_acceptance() {
    let repo = Repo::new();
    repo.commit_file("file", "initial");
    repo.commit_file("staged", "initial");
    let c = contract(&repo);
    let p = profile(&repo);
    fs::write(repo.path.join("file"), "unstaged").unwrap();
    fs::write(repo.path.join("staged"), "staged").unwrap();
    repo.git(&["add", "staged"]);
    fs::write(repo.path.join("untracked"), "new").unwrap();
    let before = snapshot(&repo);
    let mut r = report(&repo, &c, capture(&repo, &c, 1), 1, &p);
    let current = capture(&repo, &c, 2);
    assert_eq!(
        assess(&mut r, &c, &current, &p, AssessmentResult::Pass),
        Err(SemanticError::CurrentContextRequired)
    );
    assess(&mut r, &c, &current, &p, AssessmentResult::Unknown).unwrap();
    let view = r.inspect(&ValidationContext::new(&c, Some(&current), Some(&p)));
    assert_eq!(view.validity(), None);
    assert_eq!(
        view.validity_issue(),
        Some(&ValidityIssue::CandidateInsufficient(
            BindingInsufficiency::UncommittedIdentity
        ))
    );
    let mut output = Vec::new();
    view.write_redacted(&mut output).unwrap();
    assert!(
        String::from_utf8(output)
            .unwrap()
            .contains("DIRTY; staged: 1; unstaged: 1; untracked: 1")
    );
    assert_eq!(r.observation().subject().state().entries().len(), 3);
    assert_eq!(snapshot(&repo), before);
}

#[test]
fn detached_head_can_support_a_commit_bound_query_without_a_named_branch() {
    let repo = Repo::new();
    repo.commit_file("file", "initial");
    repo.git(&["checkout", "--detach"]);
    let c = contract(&repo);
    let p = profile(&repo);
    let mut r = report(&repo, &c, capture(&repo, &c, 1), 1, &p);
    assert!(
        matches!(r.observation().subject().state().head(), Head::Detached { commit } if commit == c.candidate().commit())
    );
    let latest = capture(&repo, &c, 2);
    assess(&mut r, &c, &latest, &p, AssessmentResult::Pass).unwrap();
}

#[test]
fn unborn_head_has_no_fabricated_commit_and_cannot_pass_a_required_revision() {
    let repo = Repo::new();
    repo.commit_file("file", "initial");
    let c = contract(&repo); // An actual existing revision is required, not a placeholder.
    repo.git(&["symbolic-ref", "HEAD", "refs/heads/unborn"]);
    let p = profile(&repo);
    let mut r = report(&repo, &c, capture(&repo, &c, 1), 1, &p);
    assert!(matches!(
        r.observation().subject().state().head(),
        Head::Unborn { .. }
    ));
    let latest = capture(&repo, &c, 2);
    assess(&mut r, &c, &latest, &p, AssessmentResult::Unknown).unwrap();
    let view = r.inspect(&ValidationContext::new(&c, Some(&latest), Some(&p)));
    assert_eq!(view.result(), Some(AssessmentResult::Unknown));
    assert_eq!(view.validity(), Some(Validity::Stale));
    assert!(!view.current_pass());
    let mut output = Vec::new();
    view.write_redacted(&mut output).unwrap();
    assert!(
        String::from_utf8(output)
            .unwrap()
            .contains("UNBORN (NO COMMIT)")
    );
}

#[test]
fn another_real_repository_cannot_supply_a_current_source_even_at_the_same_commit() {
    let first = Repo::new();
    let second = Repo::new();
    first.commit_file("file", "same");
    second.commit_file("file", "same");
    let c = contract(&first);
    let p = profile(&first);
    assert_eq!(
        c.candidate().commit(),
        contract(&second).candidate().commit()
    );
    let mut r = report(&first, &c, capture(&first, &c, 1), 1, &p);
    let wrong = capture(&second, &c, 2);
    assert_eq!(
        assess(&mut r, &c, &wrong, &p, AssessmentResult::Pass),
        Err(SemanticError::CurrentContextRequired)
    );
    let view = r.inspect(&ValidationContext::new(&c, Some(&wrong), Some(&p)));
    assert_eq!(view.validity(), Some(Validity::Stale));
    assert_eq!(
        view.validity_issue(),
        Some(&ValidityIssue::CandidateMismatch(
            BindingMismatch::Repository
        ))
    );
}
