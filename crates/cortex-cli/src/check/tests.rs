use super::*;
use serde_json::Value;
use std::fs;
#[path = "../../tests/support/mod.rs"]
mod support;
use support::Repo;

fn oid(repo: &Repo) -> String {
    String::from_utf8(repo.git(&["rev-parse", "HEAD"]).stdout)
        .unwrap()
        .trim()
        .to_owned()
}
fn input(repo: &Repo) -> Input {
    Input::parse(
        [
            "--task",
            "M002D.test",
            "--revision",
            "1",
            "--candidate",
            &oid(repo),
            "--expect-head",
            &oid(repo),
            "--repo",
            repo.path.to_str().unwrap(),
        ]
        .into_iter()
        .map(OsString::from),
        Format::Json,
    )
    .unwrap()
}
fn projection(report: &mut Report) -> Value {
    serde_json::from_slice(&output::render(report, Format::Json).unwrap()).unwrap()
}

#[test]
fn changed_real_commit_cannot_be_accepted_under_the_old_candidate_pin() {
    let repo = Repo::new();
    repo.commit_file("file", "initial");
    let mut calls = 0;
    let mut report = evaluate(input(&repo), |path| {
        calls += 1;
        if calls == 2 {
            repo.commit_file("file", "new commit");
        }
        inspect(path)
    })
    .unwrap();
    let json = projection(&mut report);
    assert_eq!(json["binding"]["status"], "INCOMPATIBLE");
    assert_eq!(json["binding"]["reason"], "COMMIT");
    assert_eq!(json["assessment"]["result"], "UNKNOWN");
    assert_eq!(json["assessment"]["validity"], "STALE");
    assert_ne!(
        json["source"]["head"]["commit"],
        json["reobserved"]["head"]["commit"]
    );
    assert_eq!(json["exit_code"], 9);
    assert!(!json["assessment"]["current_pass"].as_bool().unwrap());
}
#[test]
fn same_head_with_observed_worktree_change_is_stale_not_pass() {
    let repo = Repo::new();
    repo.commit_file("file", "initial");
    let mut calls = 0;
    let mut report = evaluate(input(&repo), |path| {
        calls += 1;
        if calls == 2 {
            fs::write(repo.path.join("new"), "new").unwrap();
        }
        inspect(path)
    })
    .unwrap();
    let json = projection(&mut report);
    assert_eq!(json["binding"]["status"], "STALE");
    assert_eq!(json["binding"]["reason"], "OBSERVED_STATE_CHANGED");
    assert_eq!(
        json["source"]["head"]["commit"],
        json["reobserved"]["head"]["commit"]
    );
    assert_eq!(json["assessment"]["result"], "UNKNOWN");
    assert_eq!(json["assessment"]["validity"], "STALE");
}
#[test]
fn unavailable_refresh_is_blocked_without_an_invented_oracle_or_fresh_capture() {
    for (error, category, exit) in [
        (InspectionError::MissingGit, "GIT_UNAVAILABLE", 3),
        (InspectionError::Limit("test budget"), "RESOURCE_LIMIT", 6),
        (
            InspectionError::GitFailed("local capture"),
            "INSPECTION_FAILURE",
            4,
        ),
    ] {
        let repo = Repo::new();
        repo.commit_file("file", "initial");
        let mut error = Some(error);
        let mut calls = 0;
        let mut report = evaluate(input(&repo), |path| {
            calls += 1;
            if calls == 2 {
                Err(error.take().unwrap())
            } else {
                inspect(path)
            }
        })
        .unwrap();
        let json = projection(&mut report);
        assert_eq!(json["assessment"]["result"], "BLOCKED");
        assert_eq!(json["assessment"]["validity"], "UNESTABLISHED");
        assert_eq!(
            json["observation"]["outcome"]["execution"]["status"],
            "UNAVAILABLE"
        );
        assert_eq!(json["observation"]["outcome"]["relevant_checks"], 0);
        assert!(json["observation"]["outcome"]["oracle"].is_null());
        assert!(json["reobserved"].is_null());
        assert_eq!(json["error"]["category"], category);
        assert_eq!(json["exit_code"], exit);
    }
}
#[test]
fn inconsistent_refresh_is_unknown_and_has_no_successful_execution_claim() {
    let repo = Repo::new();
    repo.commit_file("file", "initial");
    let mut calls = 0;
    let mut report = evaluate(input(&repo), |path| {
        calls += 1;
        if calls == 2 {
            Err(InspectionError::Inconsistent)
        } else {
            inspect(path)
        }
    })
    .unwrap();
    let json = projection(&mut report);
    assert_eq!(json["assessment"]["result"], "UNKNOWN");
    assert_eq!(json["assessment"]["validity"], "UNESTABLISHED");
    assert_eq!(json["observation"]["outcome"]["kind"], "INFERENCE");
    assert!(json["observation"]["outcome"]["execution"].is_null());
    assert_eq!(json["exit_code"], 7);
}
#[test]
fn source_absence_is_operational_error_without_fabricated_evidence() {
    let repo = Repo::new();
    repo.commit_file("file", "initial");
    let error = evaluate(input(&repo), |_| Err(InspectionError::MissingGit))
        .err()
        .unwrap();
    let error = CheckError::new(Format::Json, error);
    let mut bytes = vec![];
    error.write_machine_error(&mut bytes).unwrap();
    let json: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["type"], "error");
    assert!(json["assessment"].is_null());
    assert_eq!(json["exit_code"], 3);
    assert_eq!(json["authority"], "NONE");
    assert!(error.source().is_some());
}
fn prepared(
    repo: &Repo,
) -> (
    Input,
    CandidateBindingContract,
    CandidateObservation,
    VerificationProfile,
) {
    let input = input(repo);
    let first = inspect(&input.repo).unwrap();
    let profile = profile(first.git_version(), &input.expected).unwrap();
    let contract = CandidateBindingContract::new(
        input.task.clone(),
        input.revision,
        CandidateRequirement::new(
            first.repository_identity().unwrap(),
            input.candidate.clone(),
            WorktreeRequirement::AllowChanges,
        ),
    );
    let source = first
        .into_candidate(&contract, ObservationId::new("fixture", 1).unwrap())
        .unwrap();
    (input, contract, source, profile)
}
#[test]
fn different_profile_revision_scope_and_incomplete_coverage_stay_non_successful() {
    for case in 0..5 {
        let repo = Repo::new();
        repo.commit_file("file", "initial");
        let (input, contract, source, p) = prepared(&repo);
        let latest = inspect(&repo.path)
            .unwrap()
            .into_candidate(
                &contract,
                ObservationId::new(if case == 2 { "foreign" } else { "fixture" }, 2).unwrap(),
            )
            .unwrap();
        let latest = if case == 1 {
            let other = CandidateBindingContract::new(
                contract.task().clone(),
                ContractRevision::new(2).unwrap(),
                contract.candidate().clone(),
            );
            CandidateObservation::new(
                latest.id().clone(),
                &other,
                latest.state().clone(),
                latest.basis().cloned(),
            )
        } else if case >= 3 {
            CandidateObservation::new(
                latest.id().clone(),
                &contract,
                RepositoryState::new(
                    latest.state().repository().cloned(),
                    latest.state().head().clone(),
                    vec![],
                    if case == 3 {
                        ObservationCoverage::Incomplete
                    } else {
                        ObservationCoverage::GitStatus
                    },
                    if case == 4 {
                        ObservationConsistency::Indeterminate
                    } else {
                        ObservationConsistency::TwoMatchingSamples
                    },
                )
                .unwrap(),
                latest.basis().cloned(),
            )
        } else {
            latest
        };
        let latest_p = if case == 0 {
            profile("changed tool version", &input.expected).unwrap()
        } else {
            p.clone()
        };
        let mut report = finish(
            input.expected,
            contract,
            source,
            p,
            "fixture",
            Ok((latest, latest_p)),
        )
        .unwrap();
        let json = projection(&mut report);
        assert_eq!(json["assessment"]["result"], "UNKNOWN");
        assert_ne!(json["assessment"]["validity"], "CURRENT");
        assert_eq!(json["exit_code"], 9);
    }
}
#[test]
fn error_and_output_boundaries_preserve_causes_and_do_not_panic() {
    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::from(io::ErrorKind::BrokenPipe))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let error = run(
        ["--help"].into_iter().map(OsString::from),
        &mut Broken,
        &mut io::sink(),
    )
    .unwrap_err();
    assert_eq!(error.exit_code(), 1);
    assert!(error.source().is_some());
    let mut bytes = vec![];
    error.write_machine_error(&mut bytes).unwrap();
    assert!(bytes.is_empty());
    let repo = Repo::new();
    repo.commit_file("file", "initial");
    let oid = oid(&repo);
    let args = [
        "--json",
        "--task",
        "output",
        "--revision",
        "1",
        "--candidate",
        &oid,
        "--expect-head",
        &oid,
        "--repo",
        repo.path.to_str().unwrap(),
    ]
    .map(OsString::from);
    let error = run(args.into_iter(), &mut Broken, &mut io::sink()).unwrap_err();
    assert_eq!(error.exit_code(), 1);
    error.write_machine_error(&mut bytes).unwrap();
    assert!(bytes.is_empty());
    for token in ["", "0", "-1", "+1", "18446744073709551616", "x"] {
        let args = [
            "--task",
            "valid",
            "--revision",
            token,
            "--candidate",
            &"a".repeat(40),
            "--expect-head",
            &"a".repeat(40),
        ]
        .map(OsString::from);
        assert!(Input::parse(args.into_iter(), Format::Human).is_err());
    }
}

#[test]
fn rendering_a_large_typed_report_fails_boundedly_in_both_formats() {
    let repo = Repo::new();
    repo.commit_file("file", "initial");
    let (input, contract, source, p) = prepared(&repo);
    // Synthetic bounded metadata exercises presentation limits; this is not
    // claimed as an actual Git branch or authenticated capture.
    let state = RepositoryState::new(
        source.state().repository().cloned(),
        Head::Attached {
            branch: vec![b'x'; 70_000],
            commit: contract.candidate().commit().clone(),
        },
        vec![],
        ObservationCoverage::GitStatus,
        ObservationConsistency::TwoMatchingSamples,
    )
    .unwrap();
    let latest = CandidateObservation::new(
        ObservationId::new("fixture", 2).unwrap(),
        &contract,
        state.clone(),
        source.basis().cloned(),
    );
    let source = CandidateObservation::new(
        source.id().clone(),
        &contract,
        state,
        source.basis().cloned(),
    );
    let mut report = finish(
        input.expected,
        contract,
        source,
        p.clone(),
        "fixture",
        Ok((latest, p)),
    )
    .unwrap();
    for format in [Format::Json, Format::Human] {
        assert!(matches!(
            output::render(&mut report, format),
            Err(ErrorKind::Limit)
        ));
        let error = CheckError::new(format, ErrorKind::Limit);
        assert_eq!(error.exit_code(), 6);
        let mut bytes = vec![];
        error.write_machine_error(&mut bytes).unwrap();
        if format == Format::Json {
            let json: Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(json["error"]["category"], "RESOURCE_LIMIT");
            assert!(json["assessment"].is_null());
        }
    }
}

#[test]
fn verification_time_follows_the_actual_refresh_instead_of_backdating_the_outcome() {
    let repo = Repo::new();
    repo.commit_file("file", "initial");
    let mut calls = 0;
    let mut completed = None;
    let report = evaluate(input(&repo), |path| {
        calls += 1;
        let observation = inspect(path);
        if calls == 2 {
            completed = Some(SystemTime::now());
        }
        observation
    })
    .unwrap();
    assert!(report.evidence.observation().times().observed() >= completed.unwrap());
    assert!(
        report.evidence.observation().times().recorded()
            >= report.evidence.observation().times().observed()
    );
    assert!(
        report.evidence.assessment().unwrap().assessed_at()
            >= report.evidence.observation().times().recorded()
    );
}
