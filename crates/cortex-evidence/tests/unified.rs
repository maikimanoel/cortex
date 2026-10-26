//! CAP-EVD-002: independent semantics, forbidden transitions and contextual validity.

mod support;
use cortex_core::{CandidateId, TaskContract, TaskId, binding::*};
use cortex_evidence::{Evidence, ExecutionObservation, Provenance, Verdict, unified::*};
use std::{
    error::Error,
    io::{self, Write},
    time::{Duration, UNIX_EPOCH},
};
use support::*;

fn contract() -> CandidateBindingContract {
    let root = std::env::temp_dir().join("cortex-evidence-domain");
    CandidateBindingContract::new(
        TaskContract::new(
            TaskId::new("M002C.semantics").unwrap(),
            "Assess the actual candidate",
            "One relevant assertion must hold",
        )
        .unwrap(),
        ContractRevision::new(1).unwrap(),
        CandidateRequirement::new(
            RepositoryIdentity::new(root.clone(), root.join(".git"), root.join(".git")).unwrap(),
            CandidateId::from_git_commit(&"a".repeat(40)).unwrap(),
            WorktreeRequirement::AllowChanges,
        ),
    )
}
fn candidate(c: &CandidateBindingContract, sequence: u64) -> CandidateObservation {
    CandidateObservation::new(
        ObservationId::new("repository-series", sequence).unwrap(),
        c,
        RepositoryState::new(
            Some(c.candidate().repository().clone()),
            Head::Attached {
                branch: b"main".to_vec(),
                commit: c.candidate().commit().clone(),
            },
            vec![],
            ObservationCoverage::GitStatus,
            ObservationConsistency::TwoMatchingSamples,
        )
        .unwrap(),
        Some(ComparisonBasis::new("inspection-v1", b"same metadata".to_vec()).unwrap()),
    )
}
fn kind(
    status: ExecutionObservation,
    checks: u64,
    oracle: Option<OracleConclusion>,
) -> ObservationKind {
    ObservationKind::Execution(
        VerificationOutcome::new(
            status,
            checks,
            oracle.map(|conclusion| {
                OracleEvaluation::new(conclusion, "Explicit criterion comparison").unwrap()
            }),
        )
        .unwrap(),
    )
}
fn passed() -> ObservationKind {
    kind(
        ExecutionObservation::Exited { code: 0 },
        1,
        Some(OracleConclusion::Satisfied),
    )
}
fn record(c: &CandidateBindingContract, observation_kind: ObservationKind) -> EvidenceRecord {
    EvidenceRecord::new(
        ReportedObservation::new(
            ObservationId::new("verification-series", 1).unwrap(),
            candidate(c, 1),
            profile("v1"),
            times(),
            observation_kind,
            "Observed outcome or explicit blockage/absence/derivation basis",
        )
        .unwrap(),
    )
}
fn assess(
    record: &mut EvidenceRecord,
    c: &CandidateBindingContract,
    result: AssessmentResult,
    applicability: Applicability,
) -> Result<(), SemanticError> {
    record.assess(
        AssessmentId::new("assessment-1").unwrap(),
        time(102),
        result,
        applicability,
        "Explicit scoped assessment reasoning",
        &ValidationContext::new(c, Some(&candidate(c, 2)), Some(&profile("v1"))),
    )
}

#[test]
fn compatibility_and_exit_zero_create_no_automatic_assessment_or_authority() {
    let c = contract();
    assert!(matches!(
        bind_candidate(&c, Some(&candidate(&c, 1)), Some(&candidate(&c, 2))).unwrap(),
        BindingDecision::Compatible(_)
    ));
    let mut r = record(&c, passed());
    assert!(r.assessment().is_none());
    let latest = candidate(&c, 2);
    let p = profile("v1");
    let view = r.inspect(&ValidationContext::new(&c, Some(&latest), Some(&p)));
    assert_eq!(view.result(), None);
    assert_eq!(view.applicability(), None);
    assert_eq!(view.validity(), Some(Validity::Current));
    assert!(!view.current_pass());
    let mut output = Vec::new();
    view.write_redacted(&mut output).unwrap();
    assert!(
        String::from_utf8(output)
            .unwrap()
            .contains("Authority: NONE")
    );
}

#[test]
fn supported_pass_and_fail_are_explicit_and_retain_identity_time_and_reasoning() {
    let c = contract();
    for (code, oracle, result) in [
        (0, OracleConclusion::Satisfied, AssessmentResult::Pass),
        (0, OracleConclusion::Violated, AssessmentResult::Fail),
        (1, OracleConclusion::Violated, AssessmentResult::Fail),
    ] {
        let mut r = record(
            &c,
            kind(ExecutionObservation::Exited { code }, 1, Some(oracle)),
        );
        assess(&mut r, &c, result, Applicability::Applicable).unwrap();
        let a = r.assessment().unwrap().clone();
        assert_eq!(a.result(), result);
        assert_eq!(a.applicability(), Applicability::Applicable);
        assert_eq!(a.id().as_str(), "assessment-1");
        assert_eq!(a.assessed_at(), time(102));
        assert_eq!(a.compared_to(), Some(candidate(&c, 2).id()));
        assert_eq!(a.validity_at_assessment(), Some(Validity::Current));
        let mut redacted = Vec::new();
        r.inspect(&ValidationContext::new(
            &c,
            Some(&candidate(&c, 2)),
            Some(&profile("v1")),
        ))
        .write_redacted(&mut redacted)
        .unwrap();
        assert!(
            String::from_utf8(redacted)
                .unwrap()
                .contains("Authority: NONE (evidence grants no permission)")
        );
        assert_eq!(r.observation().subject().contract(), &c);
        assert_eq!(r.observation().times(), times());
        assert!(!a.rationale().is_empty());
        assert_eq!(
            r.inspect(&ValidationContext::new(
                &c,
                Some(&candidate(&c, 2)),
                Some(&profile("v1"))
            ))
            .current_pass(),
            result == AssessmentResult::Pass
        );
    }
}

#[test]
fn zero_tests_missing_or_wrong_oracle_and_nonzero_exit_cannot_pass() {
    let c = contract();
    for (observation, expected) in [
        (
            kind(ExecutionObservation::Exited { code: 0 }, 0, None),
            SemanticError::MissingOracle,
        ),
        (
            kind(ExecutionObservation::Exited { code: 0 }, 1, None),
            SemanticError::MissingOracle,
        ),
        (
            kind(
                ExecutionObservation::Exited { code: 0 },
                1,
                Some(OracleConclusion::Insufficient),
            ),
            SemanticError::MissingOracle,
        ),
        (
            kind(
                ExecutionObservation::Exited { code: 0 },
                1,
                Some(OracleConclusion::Violated),
            ),
            SemanticError::MissingOracle,
        ),
        (
            kind(
                ExecutionObservation::Exited { code: 2 },
                1,
                Some(OracleConclusion::Satisfied),
            ),
            SemanticError::NonzeroExitCannotPass,
        ),
    ] {
        let mut r = record(&c, observation);
        let original = r.clone();
        assert_eq!(
            assess(
                &mut r,
                &c,
                AssessmentResult::Pass,
                Applicability::Applicable
            ),
            Err(expected)
        );
        assert_eq!(r, original);
        assess(
            &mut r,
            &c,
            AssessmentResult::Unknown,
            Applicability::Applicable,
        )
        .unwrap();
        assert_eq!(r.assessment().unwrap().result(), AssessmentResult::Unknown);
    }
}

#[test]
fn not_run_and_blocked_cannot_be_recast_as_unknown_na_or_success() {
    let c = contract();
    for (status, result) in [
        (ExecutionObservation::NotRun, AssessmentResult::NotRun),
        (ExecutionObservation::Unavailable, AssessmentResult::Blocked),
    ] {
        for applicability in [Applicability::Applicable, Applicability::Uncertain] {
            let mut r = record(&c, kind(status, 0, None));
            for forbidden in [
                AssessmentResult::Pass,
                AssessmentResult::Fail,
                AssessmentResult::Unknown,
                AssessmentResult::NotApplicable,
            ] {
                assert!(assess(&mut r, &c, forbidden, applicability).is_err());
                assert!(r.assessment().is_none());
            }
            assess(&mut r, &c, result, applicability).unwrap();
            assert_eq!(r.assessment().unwrap().result(), result);
            assert!(!r.observation().detail().is_empty());
        }
    }
}

#[test]
fn timeout_cancellation_and_inference_are_unknown_not_execution_success() {
    let c = contract();
    for observation in [
        kind(ExecutionObservation::TimedOut, 0, None),
        kind(ExecutionObservation::Cancelled, 0, None),
        ObservationKind::Inference,
        ObservationKind::ScopeInspection(ScopeFinding::Uncertain),
    ] {
        let mut r = record(&c, observation);
        assert!(
            assess(
                &mut r,
                &c,
                AssessmentResult::Pass,
                Applicability::Applicable
            )
            .is_err()
        );
        assess(
            &mut r,
            &c,
            AssessmentResult::Unknown,
            Applicability::Uncertain,
        )
        .unwrap();
        assert_eq!(
            r.assessment().unwrap().applicability(),
            Applicability::Uncertain
        );
    }
}

#[test]
fn applicability_matrix_rejects_contradictory_combinations() {
    let c = contract();
    let cases = [
        (passed(), AssessmentResult::Pass),
        (
            kind(
                ExecutionObservation::Exited { code: 0 },
                1,
                Some(OracleConclusion::Violated),
            ),
            AssessmentResult::Fail,
        ),
        (
            kind(ExecutionObservation::Exited { code: 0 }, 0, None),
            AssessmentResult::Unknown,
        ),
        (
            kind(ExecutionObservation::NotRun, 0, None),
            AssessmentResult::NotRun,
        ),
        (
            kind(ExecutionObservation::Unavailable, 0, None),
            AssessmentResult::Blocked,
        ),
        (
            ObservationKind::ScopeInspection(ScopeFinding::Absent),
            AssessmentResult::NotApplicable,
        ),
    ];
    for (observation, result) in cases {
        for applicability in [
            Applicability::Applicable,
            Applicability::NotApplicable,
            Applicability::Uncertain,
        ] {
            let expected = match applicability {
                Applicability::Applicable => result != AssessmentResult::NotApplicable,
                Applicability::NotApplicable => result == AssessmentResult::NotApplicable,
                Applicability::Uncertain => matches!(
                    result,
                    AssessmentResult::Unknown
                        | AssessmentResult::NotRun
                        | AssessmentResult::Blocked
                ),
            };
            let mut r = record(&c, observation.clone());
            let original = r.clone();
            let actual = assess(&mut r, &c, result, applicability);
            assert_eq!(actual.is_ok(), expected, "{result:?}/{applicability:?}");
            if !expected {
                assert_eq!(r, original);
            }
        }
    }
}

#[test]
fn na_requires_examined_absence_with_a_basis_not_a_missing_tool() {
    let c = contract();
    for observation in [
        passed(),
        kind(ExecutionObservation::NotRun, 0, None),
        kind(ExecutionObservation::Unavailable, 0, None),
        ObservationKind::Inference,
        ObservationKind::ScopeInspection(ScopeFinding::Present),
        ObservationKind::ScopeInspection(ScopeFinding::Uncertain),
    ] {
        let mut r = record(&c, observation);
        assert_eq!(
            assess(
                &mut r,
                &c,
                AssessmentResult::NotApplicable,
                Applicability::NotApplicable
            ),
            Err(SemanticError::UnsupportedResult)
        );
    }
    let mut r = record(&c, ObservationKind::ScopeInspection(ScopeFinding::Absent));
    assess(
        &mut r,
        &c,
        AssessmentResult::NotApplicable,
        Applicability::NotApplicable,
    )
    .unwrap();
    assert_eq!(
        r.assessment().unwrap().result(),
        AssessmentResult::NotApplicable
    );
    assert!(!r.observation().detail().trim().is_empty());
}

#[test]
fn unknown_and_incomplete_candidate_context_cannot_support_current_pass() {
    let c = contract();
    let p = profile("v1");
    let clean = candidate(&c, 2);
    let partial = CandidateObservation::new(
        clean.id().clone(),
        &c,
        RepositoryState::new(
            clean.state().repository().cloned(),
            clean.state().head().clone(),
            vec![],
            ObservationCoverage::Incomplete,
            ObservationConsistency::TwoMatchingSamples,
        )
        .unwrap(),
        clean.basis().cloned(),
    );
    let uncertain = CandidateObservation::new(
        clean.id().clone(),
        &c,
        RepositoryState::new(
            clean.state().repository().cloned(),
            Head::Unknown,
            vec![],
            ObservationCoverage::GitStatus,
            ObservationConsistency::Indeterminate,
        )
        .unwrap(),
        clean.basis().cloned(),
    );
    let foreign = CandidateObservation::new(
        ObservationId::new("foreign-producer", 2).unwrap(),
        &c,
        clean.state().clone(),
        clean.basis().cloned(),
    );
    for current in [None, Some(&partial), Some(&uncertain), Some(&foreign)] {
        let mut r = record(&c, passed());
        let context = ValidationContext::new(&c, current, Some(&p));
        assert_eq!(
            r.assess(
                AssessmentId::new("a").unwrap(),
                time(102),
                AssessmentResult::Pass,
                Applicability::Applicable,
                "Explicit oracle reasoning",
                &context
            ),
            Err(SemanticError::CurrentContextRequired)
        );
        let view = r.inspect(&context);
        assert_eq!(view.validity(), None);
        assert!(!view.current_pass());
        assert!(view.validity_issue().is_some());
    }
    let mut r = record(&c, passed());
    assert_eq!(
        r.inspect(&ValidationContext::new(&c, Some(&clean), None))
            .validity_issue(),
        Some(&ValidityIssue::MissingProfile)
    );
}

#[test]
fn stale_pass_remains_historical_pass_and_never_automatically_becomes_current() {
    let c = contract();
    let p = profile("v1");
    let original = candidate(&c, 2);
    let mut r = record(&c, passed());
    assess(
        &mut r,
        &c,
        AssessmentResult::Pass,
        Applicability::Applicable,
    )
    .unwrap();
    let assessment = r.assessment().unwrap().clone();
    let observation = r.observation().clone();
    let changed = CandidateObservation::new(
        ObservationId::new("repository-series", 3).unwrap(),
        &c,
        RepositoryState::new(
            original.state().repository().cloned(),
            original.state().head().clone(),
            vec![Entry::Untracked {
                path: b"new".to_vec(),
            }],
            ObservationCoverage::GitStatus,
            ObservationConsistency::TwoMatchingSamples,
        )
        .unwrap(),
        original.basis().cloned(),
    );
    let view = r.inspect(&ValidationContext::new(&c, Some(&changed), Some(&p)));
    assert_eq!(view.result(), Some(AssessmentResult::Pass));
    assert_eq!(view.applicability(), Some(Applicability::Applicable));
    assert_eq!(view.validity(), Some(Validity::Stale));
    assert!(!view.current_pass());
    let restored = candidate(&c, 4);
    assert_eq!(
        r.inspect(&ValidationContext::new(&c, Some(&restored), Some(&p)))
            .validity(),
        Some(Validity::Stale)
    );
    assert_eq!(r.assessment(), Some(&assessment));
    assert_eq!(r.observation(), &observation);
    assert_eq!(
        assess(
            &mut r,
            &c,
            AssessmentResult::Pass,
            Applicability::Applicable
        ),
        Err(SemanticError::AlreadyAssessed)
    );
    let mut new_record = EvidenceRecord::new(
        ReportedObservation::new(
            ObservationId::new("verification-series", 2).unwrap(),
            candidate(&c, 3),
            p.clone(),
            ObservationTimes::new(time(102), time(103)).unwrap(),
            passed(),
            "New report for revalidation",
        )
        .unwrap(),
    );
    new_record
        .assess(
            AssessmentId::new("revalidation").unwrap(),
            time(103),
            AssessmentResult::Pass,
            Applicability::Applicable,
            "Fresh assessment after new report",
            &ValidationContext::new(&c, Some(&restored), Some(&p)),
        )
        .unwrap();
    assert!(
        new_record
            .inspect(&ValidationContext::new(&c, Some(&restored), Some(&p)))
            .current_pass()
    );
}

#[test]
fn rejecting_a_stale_conclusive_request_retains_the_observed_invalidation() {
    let c = contract();
    let changed = profile("v2");
    let p = profile("v1");
    let current = candidate(&c, 2);
    let mut r = record(&c, passed());
    let original = r.observation().clone();
    assert_eq!(
        r.assess(
            AssessmentId::new("a").unwrap(),
            time(102),
            AssessmentResult::Pass,
            Applicability::Applicable,
            "Supported oracle but changed profile",
            &ValidationContext::new(&c, Some(&current), Some(&changed))
        ),
        Err(SemanticError::CurrentContextRequired)
    );
    assert!(r.assessment().is_none());
    assert_eq!(r.observation(), &original);
    assert_eq!(
        r.inspect(&ValidationContext::new(&c, Some(&current), Some(&p)))
            .validity(),
        Some(Validity::Stale)
    );
}

#[test]
fn contract_revision_and_same_revision_aliases_are_not_ignored() {
    let c = contract();
    let p = profile("v1");
    let revised = CandidateBindingContract::new(
        c.task().clone(),
        ContractRevision::new(2).unwrap(),
        c.candidate().clone(),
    );
    let mut r = record(&c, passed());
    assert_eq!(
        r.inspect(&ValidationContext::new(
            &revised,
            Some(&candidate(&revised, 2)),
            Some(&p)
        ))
        .validity_issue(),
        Some(&ValidityIssue::CandidateStale(
            BindingStaleness::ContractRevision
        ))
    );
    let aliased = CandidateBindingContract::new(
        TaskContract::new(
            c.task().id().clone(),
            "Changed objective",
            c.task().acceptance(),
        )
        .unwrap(),
        c.revision(),
        c.candidate().clone(),
    );
    let mut r = record(&c, passed());
    let current = candidate(&aliased, 2);
    let context = ValidationContext::new(&aliased, Some(&current), Some(&p));
    assert_eq!(
        r.assess(
            AssessmentId::new("a").unwrap(),
            time(102),
            AssessmentResult::Pass,
            Applicability::Applicable,
            "Oracle",
            &context
        ),
        Err(SemanticError::Binding(BindingError::ContractRevisionReused))
    );
    assert_eq!(r.inspect(&context).validity(), None);
}

#[test]
fn mechanism_inputs_configuration_environment_and_components_invalidate_independently() {
    let c = contract();
    let current = candidate(&c, 2);
    let old = profile("v1");
    let variants = [
        profile("v2"),
        VerificationProfile::new(
            Provenance::new(
                "other mechanism",
                "single retained contract criterion",
                "offline deterministic checks",
            )
            .unwrap(),
            "v1",
            "fixture environment",
            &[],
        )
        .unwrap(),
        VerificationProfile::new(
            Provenance::new(
                "fixture verifier",
                "other inputs",
                "offline deterministic checks",
            )
            .unwrap(),
            "v1",
            "fixture environment",
            &[],
        )
        .unwrap(),
        VerificationProfile::new(
            Provenance::new(
                "fixture verifier",
                "single retained contract criterion",
                "different configuration",
            )
            .unwrap(),
            "v1",
            "fixture environment",
            &[],
        )
        .unwrap(),
        VerificationProfile::new(old.provenance().clone(), "v1", "other environment", &[]).unwrap(),
        VerificationProfile::new(
            old.provenance().clone(),
            "v1",
            "fixture environment",
            &[("participating-tool", "v2")],
        )
        .unwrap(),
    ];
    for changed in variants {
        let mut r = record(&c, passed());
        assess(
            &mut r,
            &c,
            AssessmentResult::Pass,
            Applicability::Applicable,
        )
        .unwrap();
        assert_eq!(
            r.inspect(&ValidationContext::new(&c, Some(&current), Some(&changed)))
                .validity_issue(),
            Some(&ValidityIssue::ProfileChanged)
        );
        assert_eq!(
            r.inspect(&ValidationContext::new(&c, Some(&current), Some(&old)))
                .validity(),
            Some(Validity::Stale)
        );
    }
}

#[test]
fn explicit_invalidation_is_sticky_and_keeps_its_first_reason() {
    let c = contract();
    let mut r = record(&c, passed());
    let original = r.clone();
    assert!(r.invalidate(" ").is_err());
    assert_eq!(r, original);
    r.invalidate("Declared dependency changed").unwrap();
    assert_eq!(
        r.invalidate("replacement"),
        Err(SemanticError::AlreadyInvalidated)
    );
    assert_eq!(r.invalidation_reason(), Some("Declared dependency changed"));
    assert_eq!(
        r.inspect(&ValidationContext::new(
            &c,
            Some(&candidate(&c, 2)),
            Some(&profile("v1"))
        ))
        .validity(),
        Some(Validity::Stale)
    );
}

#[test]
fn terminal_assessments_and_rejected_malformed_judgments_preserve_original_values() {
    let c = contract();
    let mut r = record(&c, passed());
    let original = r.clone();
    let p = profile("v1");
    let current = candidate(&c, 2);
    let context = ValidationContext::new(&c, Some(&current), Some(&p));
    for (at, reason) in [
        (time(100), "valid reason"),
        (time(102), ""),
        (time(102), "private-token\0"),
    ] {
        assert!(
            r.assess(
                AssessmentId::new("a").unwrap(),
                at,
                AssessmentResult::Pass,
                Applicability::Applicable,
                reason,
                &context
            )
            .is_err()
        );
        assert_eq!(r, original);
    }
    assess(
        &mut r,
        &c,
        AssessmentResult::Pass,
        Applicability::Applicable,
    )
    .unwrap();
    let original = r.clone();
    for result in [
        AssessmentResult::Pass,
        AssessmentResult::Fail,
        AssessmentResult::Unknown,
        AssessmentResult::NotRun,
        AssessmentResult::Blocked,
        AssessmentResult::NotApplicable,
    ] {
        assert_eq!(
            assess(&mut r, &c, result, Applicability::Applicable),
            Err(SemanticError::AlreadyAssessed)
        );
        assert_eq!(r, original);
    }
}

#[test]
fn invalid_profiles_timestamps_and_contradictory_observations_fail_at_construction() {
    for bad in [
        "".to_owned(),
        " \n".into(),
        "private-token\0".into(),
        "x".repeat(4097),
    ] {
        let e = VerificationProfile::new(Provenance::new("m", "i", "c").unwrap(), &bad, "env", &[])
            .unwrap_err();
        assert!(!e.to_string().contains("private-token"));
        assert!(e.source().is_some());
        assert!(OracleEvaluation::new(OracleConclusion::Satisfied, &bad).is_err());
    }
    assert!(AssessmentId::new("bad/name").is_err());
    assert!(ObservationTimes::new(time(101), time(100)).is_err());
    assert!(ObservationTimes::new(UNIX_EPOCH - Duration::from_secs(1), time(101)).is_err());
    for status in [
        ExecutionObservation::NotRun,
        ExecutionObservation::Unavailable,
        ExecutionObservation::TimedOut,
        ExecutionObservation::Cancelled,
    ] {
        assert_eq!(
            VerificationOutcome::new(status, 1, None),
            Err(SemanticError::ContradictoryObservation)
        );
        assert!(
            VerificationOutcome::new(
                status,
                0,
                Some(
                    OracleEvaluation::new(OracleConclusion::Insufficient, "Not completed").unwrap()
                )
            )
            .is_err()
        );
    }
    assert!(
        VerificationOutcome::new(
            ExecutionObservation::Exited { code: 0 },
            0,
            Some(OracleEvaluation::new(OracleConclusion::Satisfied, "No tests ran").unwrap())
        )
        .is_err()
    );
    assert!(
        VerificationProfile::new(
            Provenance::new("m", "i", "c").unwrap(),
            "1",
            "env",
            &[("git", "1"), ("git", "2")]
        )
        .is_err()
    );
}

#[test]
fn profile_text_and_component_count_boundaries_are_exercised() {
    let text = "é".repeat(2048);
    let p = VerificationProfile::new(
        Provenance::new(&text, &text, &text).unwrap(),
        &text,
        &text,
        &[("z", "1"), ("a", "2")],
    )
    .unwrap();
    assert_eq!(p.version(), text);
    assert_eq!(p.environment(), text);
    assert_eq!(p.components()[0], ("a".to_owned(), "2".to_owned()));
    let names: Vec<_> = (0..33).map(|n| format!("component-{n}")).collect();
    let components: Vec<_> = names.iter().map(|n| (n.as_str(), "v1")).collect();
    assert!(
        VerificationProfile::new(p.provenance().clone(), "1", "env", &components[..32]).is_ok()
    );
    assert_eq!(
        VerificationProfile::new(p.provenance().clone(), "1", "env", &components),
        Err(SemanticError::ComponentLimit)
    );
}

#[test]
fn observation_identity_collision_is_rejected_in_report_and_validation() {
    let c = contract();
    let subject = candidate(&c, 1);
    assert_eq!(
        ReportedObservation::new(
            subject.id().clone(),
            subject,
            profile("v1"),
            times(),
            passed(),
            "Observed"
        ),
        Err(SemanticError::IdentityCollision)
    );
    let mut r = record(&c, passed());
    let old = candidate(&c, 2);
    let current = CandidateObservation::new(
        r.observation().id().clone(),
        &c,
        old.state().clone(),
        old.basis().cloned(),
    );
    let p = profile("v1");
    let context = ValidationContext::new(&c, Some(&current), Some(&p));
    assert_eq!(
        r.assess(
            AssessmentId::new("a").unwrap(),
            time(102),
            AssessmentResult::Pass,
            Applicability::Applicable,
            "Oracle",
            &context
        ),
        Err(SemanticError::IdentityCollision)
    );
    assert_eq!(
        r.inspect(&context).validity_issue(),
        Some(&ValidityIssue::ObservationIdentityCollision)
    );
}

#[test]
fn redacted_inspection_preserves_dimensions_and_omits_all_private_text() {
    let c = contract();
    let p = VerificationProfile::new(
        Provenance::new("secret-mechanism", "secret-input", "secret-config").unwrap(),
        "secret-version",
        "secret-environment",
        &[("secret-component", "secret-component-version")],
    )
    .unwrap();
    let report = ReportedObservation::new(
        ObservationId::new("secret-report-id", 1).unwrap(),
        candidate(&c, 1),
        p.clone(),
        times(),
        kind(
            ExecutionObservation::Exited { code: 0 },
            1,
            Some(OracleConclusion::Satisfied),
        ),
        "secret-detail",
    )
    .unwrap();
    let mut r = EvidenceRecord::new(report);
    let latest = candidate(&c, 2);
    let context = ValidationContext::new(&c, Some(&latest), Some(&p));
    r.assess(
        AssessmentId::new("secret-assessment-id").unwrap(),
        time(102),
        AssessmentResult::Pass,
        Applicability::Applicable,
        "secret-rationale",
        &context,
    )
    .unwrap();
    r.invalidate("secret-invalidation").unwrap();
    assert!(!format!("{r:?}").contains("secret-"));
    let mut out = Vec::new();
    let view = r.inspect(&context);
    view.write_redacted(&mut out).unwrap();
    let out = String::from_utf8(out).unwrap();
    for expected in [
        "Result: PASS",
        "Applicability: APPLICABLE",
        "Validity: STALE",
        "Authority: NONE",
    ] {
        assert!(out.contains(expected));
    }
    assert!(!out.contains("secret-"));
    assert!(!out.contains(c.task().id().as_str()));
    assert!(!out.contains(c.candidate().repository().root().to_str().unwrap()));
    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::ErrorKind::BrokenPipe.into())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    assert_eq!(
        view.write_redacted(&mut Broken).unwrap_err().kind(),
        io::ErrorKind::BrokenPipe
    );
}

#[test]
fn legacy_inconclusive_retains_its_original_semantics_and_has_no_inferred_metadata() {
    let c = contract();
    for status in [
        ExecutionObservation::NotRun,
        ExecutionObservation::Unavailable,
        ExecutionObservation::TimedOut,
        ExecutionObservation::Exited { code: 0 },
    ] {
        let mut legacy = Evidence::new(
            c.task().clone(),
            c.candidate().commit().clone(),
            Provenance::new("legacy mechanism", "legacy inputs", "legacy config").unwrap(),
            status,
        );
        legacy
            .assess(Verdict::Inconclusive, "Original unclassified rationale")
            .unwrap();
        assert_eq!(legacy.observation(), status);
        assert_eq!(
            legacy.assessment().unwrap().verdict(),
            Verdict::Inconclusive
        );
        assert_eq!(
            legacy.assessment().unwrap().rationale(),
            "Original unclassified rationale"
        );
    }
}

#[test]
fn fail_and_na_also_require_current_context_and_missing_validity_is_not_clean() {
    let c = contract();
    let p = profile("v1");
    for (kind, result, applicability) in [
        (
            kind(
                ExecutionObservation::Exited { code: 1 },
                1,
                Some(OracleConclusion::Violated),
            ),
            AssessmentResult::Fail,
            Applicability::Applicable,
        ),
        (
            ObservationKind::ScopeInspection(ScopeFinding::Absent),
            AssessmentResult::NotApplicable,
            Applicability::NotApplicable,
        ),
    ] {
        let mut r = record(&c, kind);
        assert_eq!(
            r.assess(
                AssessmentId::new("a").unwrap(),
                time(102),
                result,
                applicability,
                "Explicit reasoning",
                &ValidationContext::new(&c, None, Some(&p))
            ),
            Err(SemanticError::CurrentContextRequired)
        );
        assert!(r.assessment().is_none());
    }
    let original = candidate(&c, 1);
    for (coverage, consistency, head) in [
        (
            ObservationCoverage::Incomplete,
            ObservationConsistency::TwoMatchingSamples,
            original.state().head().clone(),
        ),
        (
            ObservationCoverage::GitStatus,
            ObservationConsistency::Indeterminate,
            original.state().head().clone(),
        ),
        (
            ObservationCoverage::GitStatus,
            ObservationConsistency::TwoMatchingSamples,
            Head::Unknown,
        ),
    ] {
        let uncertain = CandidateObservation::new(
            original.id().clone(),
            &c,
            RepositoryState::new(
                original.state().repository().cloned(),
                head,
                vec![],
                coverage,
                consistency,
            )
            .unwrap(),
            original.basis().cloned(),
        );
        let mut r = EvidenceRecord::new(
            ReportedObservation::new(
                ObservationId::new("verification", 1).unwrap(),
                uncertain,
                p.clone(),
                times(),
                ObservationKind::Inference,
                "Insufficient candidate facts",
            )
            .unwrap(),
        );
        let current = candidate(&c, 2);
        let view = r.inspect(&ValidationContext::new(&c, Some(&current), Some(&p)));
        assert_eq!(view.validity(), None);
        let mut output = Vec::new();
        view.write_redacted(&mut output).unwrap();
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("Working tree: INDETERMINATE"));
        assert!(!output.contains("CLEAN"));
    }
}

#[test]
fn identical_inputs_produce_equal_assessments_and_views_without_hidden_clock_or_io() {
    let c = contract();
    let p = profile("v1");
    let current = candidate(&c, 2);
    let mut first = record(&c, passed());
    let mut second = first.clone();
    assess(
        &mut first,
        &c,
        AssessmentResult::Pass,
        Applicability::Applicable,
    )
    .unwrap();
    assess(
        &mut second,
        &c,
        AssessmentResult::Pass,
        Applicability::Applicable,
    )
    .unwrap();
    assert_eq!(first, second);
    assert_eq!(
        first.assessment().unwrap().candidate_at_assessment(),
        Some(&current)
    );
    assert_eq!(
        first.assessment().unwrap().profile_at_assessment(),
        Some(&p)
    );
    assert!(
        first
            .assessment()
            .unwrap()
            .validity_issue_at_assessment()
            .is_none()
    );
    let mut outputs = [vec![], vec![]];
    first
        .inspect(&ValidationContext::new(&c, Some(&current), Some(&p)))
        .write_redacted(&mut outputs[0])
        .unwrap();
    second
        .inspect(&ValidationContext::new(&c, Some(&current), Some(&p)))
        .write_redacted(&mut outputs[1])
        .unwrap();
    assert_eq!(outputs[0], outputs[1]);
}
