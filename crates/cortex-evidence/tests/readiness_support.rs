//! A-01..32: Evidence-to-edge support, using literal independent expectations.
use cortex_core::{
    CandidateId, TaskContract, TaskId, binding::*, parallel::*, temporal::SourceReference,
};
use cortex_evidence::{ExecutionObservation, Provenance, readiness::*, unified::*};
use std::{
    path::PathBuf,
    time::{Duration, UNIX_EPOCH},
};

fn rev(n: u64) -> ContractRevision {
    ContractRevision::new(n).unwrap()
}
fn task(s: &str) -> TaskId {
    TaskId::new(s).unwrap()
}
fn edge(s: &str) -> EdgeId {
    EdgeId::new(s).unwrap()
}
fn candidate(s: &str) -> CandidateId {
    CandidateId::from_git_commit(&s.repeat(40)).unwrap()
}
fn source(s: &str) -> SourceReference {
    SourceReference::operator_reported("private-canary", s, "1").unwrap()
}
fn limits() -> SupportLimits {
    SupportLimits::new(32, 32, 16, 4_000_000).unwrap()
}
fn contract(id: &str, hex: &str, revision: u64, criterion: &str) -> CandidateBindingContract {
    let root = PathBuf::from(format!("/synthetic/private-canary-{id}"));
    CandidateBindingContract::new(
        TaskContract::new(task(id), "private-canary objective", criterion).unwrap(),
        rev(revision),
        CandidateRequirement::new(
            RepositoryIdentity::new(root.clone(), root.join(".git"), root.join(".git")).unwrap(),
            candidate(hex),
            WorktreeRequirement::Clean,
        ),
    )
}
fn capture(c: &CandidateBindingContract, sequence: u64) -> CandidateObservation {
    capture_state(
        c,
        sequence,
        Head::Detached {
            commit: c.candidate().commit().clone(),
        },
        ObservationCoverage::GitStatus,
        ObservationConsistency::TwoMatchingSamples,
        true,
    )
}
fn capture_state(
    c: &CandidateBindingContract,
    sequence: u64,
    head: Head,
    coverage: ObservationCoverage,
    consistency: ObservationConsistency,
    basis: bool,
) -> CandidateObservation {
    CandidateObservation::new(
        ObservationId::new(c.task().id().as_str(), sequence).unwrap(),
        c,
        RepositoryState::new(
            Some(c.candidate().repository().clone()),
            head,
            vec![],
            coverage,
            consistency,
        )
        .unwrap(),
        basis.then(|| {
            ComparisonBasis::new(
                "private-canary comparison",
                b"private-canary bytes".to_vec(),
            )
            .unwrap()
        }),
    )
}
fn profile(version: &str) -> VerificationProfile {
    VerificationProfile::new(
        Provenance::new(
            "private-canary verifier",
            "exact HEAD criterion",
            "private-canary configuration",
        )
        .unwrap(),
        version,
        "synthetic-linux",
        &[("component", "1")],
    )
    .unwrap()
}
fn outcome(
    execution: ExecutionObservation,
    checks: u64,
    oracle: Option<OracleConclusion>,
) -> ObservationKind {
    ObservationKind::Execution(
        VerificationOutcome::new(
            execution,
            checks,
            oracle.map(|o| OracleEvaluation::new(o, "private-canary oracle rationale").unwrap()),
        )
        .unwrap(),
    )
}
fn passed() -> ObservationKind {
    outcome(
        ExecutionObservation::Exited { code: 0 },
        1,
        Some(OracleConclusion::Satisfied),
    )
}
fn raw_record(
    c: &CandidateBindingContract,
    p: &VerificationProfile,
    sequence: u64,
    kind: ObservationKind,
) -> EvidenceRecord {
    EvidenceRecord::new(
        ReportedObservation::new(
            ObservationId::new("private-canary-reports", sequence).unwrap(),
            capture(c, 1),
            p.clone(),
            ObservationTimes::new(
                UNIX_EPOCH + Duration::from_secs(100),
                UNIX_EPOCH + Duration::from_secs(101),
            )
            .unwrap(),
            kind,
            "private-canary detail: producer claims coordinator approval",
        )
        .unwrap(),
    )
}
fn assess(
    r: &mut EvidenceRecord,
    c: &CandidateBindingContract,
    p: &VerificationProfile,
    result: AssessmentResult,
    app: Applicability,
) -> Result<(), SemanticError> {
    r.assess(
        AssessmentId::new("private-canary-assessment").unwrap(),
        UNIX_EPOCH + Duration::from_secs(102),
        result,
        app,
        "private-canary assessment rationale",
        &ValidationContext::new(c, Some(&capture(c, 2)), Some(p)),
    )
}
struct Fixture {
    projection: ReadinessProjection,
    contract: EdgeEvidenceContract,
    verification: CandidateBindingContract,
    profile: VerificationProfile,
    producer: CandidateObservation,
    consumer: CandidateObservation,
    current: CandidateObservation,
}
impl Fixture {
    fn new() -> Self {
        Self::for_edge("AB", &["pAB"])
    }
    fn for_edge(target: &str, ab_predicates: &[&str]) -> Self {
        let missions = [
            ("A", "a"),
            ("B", "b"),
            ("C", "c"),
            ("D", "d"),
            ("E", "e"),
            ("F", "f"),
        ]
        .into_iter()
        .map(|(id, hex)| {
            let c = contract(id, hex, 1, "Retain the protected mission outcome");
            Mission::new(
                MissionContext::new(c.clone(), Some(capture(&c, 1))).unwrap(),
                ConsumerKind::Preview,
                DependencyCoverage::DeclaredComplete,
            )
        })
        .collect();
        let edges = [
            ("AB", "A", "B", "a1", "1", ab_predicates),
            ("BC", "B", "C", "b1", "2", &["pBC"][..]),
            ("BF", "B", "F", "b1", "2", &["pBF"][..]),
            ("DE", "D", "E", "d1", "3", &["pDE"][..]),
            ("EF", "E", "F", "e1", "4", &["pEF"][..]),
        ]
        .into_iter()
        .map(|(id, p, c, a, hex, preds)| {
            DependencyEdge::new(
                edge(id),
                task(p),
                task(c),
                ArtifactRequirement::new(
                    ArtifactReference::new(ArtifactId::new(a).unwrap(), rev(1), candidate(hex)),
                    DependencyKind::Artifact,
                    ReadinessLevel::IntegratedAccepted,
                    preds.iter().map(|s| PredicateId::new(s).unwrap()).collect(),
                )
                .unwrap(),
            )
        })
        .collect();
        let projection = ReadinessProjection::new(
            DependencyGraph::new(
                GraphReference::new(GraphId::new("private-canary-graph").unwrap(), rev(1)),
                missions,
                edges,
                GraphLimits::new(6, 5, 64, 4096, 2_000_000).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        let scope = EdgeScopeSnapshot::capture(&projection, &edge(target)).unwrap();
        let hex = if target == "AB" {
            "1"
        } else if target == "DE" {
            "3"
        } else if target == "EF" {
            "4"
        } else {
            "2"
        };
        let verification = contract(
            "V.head",
            hex,
            1,
            "The actual HEAD query equals the required committed revision",
        );
        let p = profile("1");
        let producer = capture(scope.producer().context().contract(), 2);
        let consumer = capture(scope.consumer().context().contract(), 2);
        let current = capture(&verification, 3);
        let mappings = scope
            .requirement()
            .predicates()
            .iter()
            .map(|id| {
                PredicateCorrespondence::new(
                    id.clone(),
                    verification.clone(),
                    p.clone(),
                    CriterionCoverage::ExactSingleCriterion,
                )
            })
            .collect();
        let contract = EdgeEvidenceContract::new(
            scope,
            rev(1),
            verification.candidate().clone(),
            mappings,
            source("private-canary mapping"),
            limits(),
        )
        .unwrap();
        Self {
            projection,
            contract,
            verification,
            profile: p,
            producer,
            consumer,
            current,
        }
    }
    fn record(&self) -> EvidenceRecord {
        self.record_kind(
            1,
            passed(),
            Some((AssessmentResult::Pass, Applicability::Applicable)),
        )
    }
    fn record_kind(
        &self,
        sequence: u64,
        kind: ObservationKind,
        result: Option<(AssessmentResult, Applicability)>,
    ) -> EvidenceRecord {
        let mut r = raw_record(&self.verification, &self.profile, sequence, kind);
        if let Some((r0, a)) = result {
            assess(&mut r, &self.verification, &self.profile, r0, a).unwrap();
        }
        r
    }
    fn bindings(&self) -> Vec<WitnessBinding> {
        self.contract
            .scope()
            .requirement()
            .predicates()
            .iter()
            .map(|p| {
                WitnessBinding::new(
                    p.clone(),
                    EvidenceReference::new(
                        ObservationId::new("private-canary-reports", 1).unwrap(),
                        Some(AssessmentId::new("private-canary-assessment").unwrap()),
                    ),
                    self.contract.scope().requirement().artifact().clone(),
                )
            })
            .collect()
    }
    fn exams(&self) -> Vec<RecordExamination<'_>> {
        vec![RecordExamination::new(
            ObservationId::new("private-canary-reports", 1).unwrap(),
            &self.verification,
            Some(&self.current),
            Some(&self.profile),
        )]
    }
    fn request<'a>(
        &'a self,
        exams: &'a [RecordExamination<'a>],
        bindings: &'a [WitnessBinding],
    ) -> SupportRequest<'a> {
        SupportRequest::new(Some(&self.producer), Some(&self.consumer), exams, bindings)
    }
    fn inspect<'e>(&self, records: &'e mut [EvidenceRecord]) -> SupportReport<'e> {
        inspect_support(
            &self.projection,
            &self.contract,
            records,
            &self.request(&self.exams(), &self.bindings()),
            limits(),
        )
        .unwrap()
    }
    fn remap(&self, revision: u64, mappings: Vec<PredicateCorrespondence>) -> EdgeEvidenceContract {
        EdgeEvidenceContract::new(
            self.contract.scope().clone(),
            rev(revision),
            self.verification.candidate().clone(),
            mappings,
            source("private-canary mapping"),
            limits(),
        )
        .unwrap()
    }
}
fn not_supported(r: &SupportReport<'_>) {
    assert_eq!(r.evidence_part(), EvidencePart::Incomplete);
    assert!(
        r.findings()
            .iter()
            .any(|f| f.disposition() == SupportDisposition::NotSupported)
    );
    assert_eq!(r.authority(), ExecutionAuthority::Unavailable);
}
fn has_issue(r: &SupportReport<'_>, issue: WitnessIssue) -> bool {
    r.findings().iter().any(|f| {
        f.reasons()
            .iter()
            .any(|r| matches!(r,SupportReason::Witness{issue:i,..} if *i==issue))
    })
}
fn apply(p: &mut ReadinessProjection, id: &str, event: Event) {
    p.apply(
        Notification::new(
            NotificationId::new(id).unwrap(),
            p.graph().reference().clone(),
            event,
        )
        .unwrap(),
    )
    .unwrap();
}

#[test]
fn a_01_current_pass_and_repetition_preserve_originals_and_g1() {
    let f = Fixture::new();
    let mut records = vec![f.record()];
    let original = records.clone();
    let graph = f.projection.clone();
    let first = f.inspect(&mut records);
    assert_eq!(
        first.evidence_part(),
        EvidencePart::AllRequiredPredicatesSupported
    );
    assert_eq!(
        first.findings()[0].disposition(),
        SupportDisposition::SupportedWithinDeclaredContract
    );
    assert!(first.findings()[0].reasons().is_empty());
    let view = &first.inspected_records()[0];
    assert_eq!(view.result(), Some(AssessmentResult::Pass));
    assert_eq!(view.applicability(), Some(Applicability::Applicable));
    assert_eq!(view.validity(), Some(Validity::Current));
    assert_eq!(view.compared_to(), Some(f.current.id()));
    assert_eq!(first.authority(), ExecutionAuthority::Unavailable);
    let basis = first.basis().clone();
    drop(first);
    let second = f.inspect(&mut records);
    assert_eq!(second.basis(), &basis);
    drop(second);
    assert_eq!(records, original);
    assert_eq!(f.projection, graph);
}
#[test]
fn a_02_unassessed_is_not_not_run_or_support() {
    let f = Fixture::new();
    let mut r = vec![f.record_kind(1, passed(), None)];
    let report = f.inspect(&mut r);
    not_supported(&report);
    assert_eq!(report.inspected_records()[0].result(), None);
    assert_eq!(report.inspected_records()[0].applicability(), None);
    assert_eq!(
        report.inspected_records()[0].validity(),
        Some(Validity::Current)
    );
    assert!(
        report.inspected_records()[0]
            .record()
            .assessment()
            .is_none()
    );
}
#[test]
fn a_03_not_run_remains_not_run() {
    let f = Fixture::new();
    let mut r = vec![f.record_kind(
        1,
        outcome(ExecutionObservation::NotRun, 0, None),
        Some((AssessmentResult::NotRun, Applicability::Applicable)),
    )];
    let report = f.inspect(&mut r);
    not_supported(&report);
    assert_eq!(
        report.inspected_records()[0].result(),
        Some(AssessmentResult::NotRun)
    );
}
#[test]
fn a_04_blocked_is_not_not_applicable() {
    let f = Fixture::new();
    let mut r = vec![f.record_kind(
        1,
        outcome(ExecutionObservation::Unavailable, 0, None),
        Some((AssessmentResult::Blocked, Applicability::Uncertain)),
    )];
    let report = f.inspect(&mut r);
    not_supported(&report);
    assert_eq!(
        report.inspected_records()[0].result(),
        Some(AssessmentResult::Blocked)
    );
    assert_eq!(
        report.inspected_records()[0].applicability(),
        Some(Applicability::Uncertain)
    );
}
#[test]
fn a_05_unknown_execution_inference_and_oracle_remain_unknown() {
    let f = Fixture::new();
    for kind in [
        outcome(ExecutionObservation::TimedOut, 0, None),
        outcome(ExecutionObservation::Cancelled, 0, None),
        ObservationKind::Inference,
        outcome(
            ExecutionObservation::Exited { code: 0 },
            1,
            Some(OracleConclusion::Insufficient),
        ),
    ] {
        let mut r = vec![f.record_kind(
            1,
            kind,
            Some((AssessmentResult::Unknown, Applicability::Applicable)),
        )];
        let report = f.inspect(&mut r);
        not_supported(&report);
        assert_eq!(
            report.inspected_records()[0].result(),
            Some(AssessmentResult::Unknown)
        );
    }
}
#[test]
fn a_06_current_fail_and_conflicting_witnesses_are_not_cherry_picked() {
    let f = Fixture::new();
    let failed = f.record_kind(
        2,
        outcome(
            ExecutionObservation::Exited { code: 0 },
            1,
            Some(OracleConclusion::Violated),
        ),
        Some((AssessmentResult::Fail, Applicability::Applicable)),
    );
    let mut r = vec![f.record(), failed];
    let mut bindings = f.bindings();
    let mut exams = f.exams();
    bindings.push(WitnessBinding::new(
        PredicateId::new("pAB").unwrap(),
        EvidenceReference::new(
            r[1].observation().id().clone(),
            r[1].assessment().map(|a| a.id().clone()),
        ),
        f.contract.scope().requirement().artifact().clone(),
    ));
    exams.push(RecordExamination::new(
        r[1].observation().id().clone(),
        &f.verification,
        Some(&f.current),
        Some(&f.profile),
    ));
    let report = inspect_support(
        &f.projection,
        &f.contract,
        &mut r,
        &f.request(&exams, &bindings),
        limits(),
    )
    .unwrap();
    not_supported(&report);
    assert_eq!(
        report.inspected_records()[1].result(),
        Some(AssessmentResult::Fail)
    );
    assert_eq!(
        report.inspected_records()[1].validity(),
        Some(Validity::Current)
    );
    assert_eq!(report.findings()[0].witnesses().len(), 2);
    assert!(has_issue(&report, WitnessIssue::NotCurrentPass));
}
#[test]
fn a_07_examined_not_applicable_does_not_remove_required_predicate() {
    let f = Fixture::new();
    let mut r = vec![f.record_kind(
        1,
        ObservationKind::ScopeInspection(ScopeFinding::Absent),
        Some((
            AssessmentResult::NotApplicable,
            Applicability::NotApplicable,
        )),
    )];
    let report = f.inspect(&mut r);
    not_supported(&report);
    assert_eq!(report.findings().len(), 1);
    assert_eq!(
        report.inspected_records()[0].result(),
        Some(AssessmentResult::NotApplicable)
    );
    assert_eq!(
        report.inspected_records()[0].validity(),
        Some(Validity::Current)
    );
}
#[test]
fn a_08_zero_relevant_checks_cannot_be_assessed_pass_or_support() {
    let f = Fixture::new();
    let mut record = f.record_kind(
        1,
        outcome(ExecutionObservation::Exited { code: 0 }, 0, None),
        None,
    );
    let original = record.clone();
    assert_eq!(
        assess(
            &mut record,
            &f.verification,
            &f.profile,
            AssessmentResult::Pass,
            Applicability::Applicable
        ),
        Err(SemanticError::MissingOracle)
    );
    assert_eq!(record, original);
    not_supported(&f.inspect(&mut [record]));
}
#[test]
fn a_09_missing_insufficient_or_wrong_oracle_cannot_be_laundered() {
    let f = Fixture::new();
    for oracle in [
        None,
        Some(OracleConclusion::Insufficient),
        Some(OracleConclusion::Violated),
    ] {
        let mut record = f.record_kind(
            1,
            outcome(ExecutionObservation::Exited { code: 0 }, 1, oracle),
            None,
        );
        assert_eq!(
            assess(
                &mut record,
                &f.verification,
                &f.profile,
                AssessmentResult::Pass,
                Applicability::Applicable
            ),
            Err(SemanticError::MissingOracle)
        );
        not_supported(&f.inspect(&mut [record]));
    }
    assert_eq!(
        VerificationOutcome::new(
            ExecutionObservation::Exited { code: 0 },
            0,
            Some(OracleEvaluation::new(OracleConclusion::Satisfied, "claimed").unwrap())
        ),
        Err(SemanticError::ContradictoryObservation)
    );
}
#[test]
fn a_10_narrow_pass_and_unestablished_coverage_do_not_support_broader_claim() {
    let f = Fixture::new();
    let mut r = vec![f.record()];
    let broad = CandidateBindingContract::new(
        TaskContract::new(task("V.broad"), "Broader", "All acceptance criteria hold").unwrap(),
        rev(1),
        f.verification.candidate().clone(),
    );
    for (c, coverage, expected) in [
        (
            broad,
            CriterionCoverage::ExactSingleCriterion,
            WitnessIssue::CriterionMismatch,
        ),
        (
            f.verification.clone(),
            CriterionCoverage::Unestablished,
            WitnessIssue::NotCurrentPass,
        ),
    ] {
        let rule = f.remap(
            2,
            vec![PredicateCorrespondence::new(
                PredicateId::new("pAB").unwrap(),
                c,
                f.profile.clone(),
                coverage,
            )],
        );
        let report = inspect_support(
            &f.projection,
            &rule,
            &mut r,
            &f.request(&f.exams(), &f.bindings()),
            limits(),
        )
        .unwrap();
        not_supported(&report);
        if coverage == CriterionCoverage::Unestablished {
            assert!(
                report.findings()[0]
                    .reasons()
                    .contains(&SupportReason::CoverageUnestablished)
            );
        } else {
            assert!(has_issue(&report, expected));
        }
        assert_eq!(
            report.inspected_records()[0].validity(),
            Some(Validity::Current)
        );
    }
}
#[test]
fn a_11_source_candidate_mismatch_stales_but_target_label_does_not() {
    let f = Fixture::new();
    let mut r = vec![f.record()];
    let wrong = capture_state(
        &f.verification,
        3,
        Head::Detached {
            commit: candidate("9"),
        },
        ObservationCoverage::GitStatus,
        ObservationConsistency::TwoMatchingSamples,
        true,
    );
    let exams = vec![RecordExamination::new(
        r[0].observation().id().clone(),
        &f.verification,
        Some(&wrong),
        Some(&f.profile),
    )];
    let report = inspect_support(
        &f.projection,
        &f.contract,
        &mut r,
        &f.request(&exams, &f.bindings()),
        limits(),
    )
    .unwrap();
    not_supported(&report);
    assert_eq!(
        report.inspected_records()[0].validity_issue(),
        Some(&ValidityIssue::CandidateMismatch(BindingMismatch::Commit))
    );
    drop(report);
    let mut fresh = vec![f.record()];
    let original = fresh.clone();
    let b = &f.bindings()[0];
    let bindings = vec![WitnessBinding::new(
        b.predicate().clone(),
        b.evidence().clone(),
        ArtifactReference::new(b.artifact().id().clone(), rev(1), candidate("9")),
    )];
    let report = inspect_support(
        &f.projection,
        &f.contract,
        &mut fresh,
        &f.request(&f.exams(), &bindings),
        limits(),
    )
    .unwrap();
    not_supported(&report);
    assert!(has_issue(&report, WitnessIssue::ArtifactMismatch));
    drop(report);
    assert_eq!(fresh, original);
}
#[test]
fn a_12_revision_mismatch_alias_and_stale_scope_have_distinct_atomic_results() {
    let f = Fixture::new();
    let mut r = vec![f.record()];
    let new = CandidateBindingContract::new(
        f.verification.task().clone(),
        rev(2),
        f.verification.candidate().clone(),
    );
    let latest = capture(&new, 3);
    let exams = vec![RecordExamination::new(
        r[0].observation().id().clone(),
        &new,
        Some(&latest),
        Some(&f.profile),
    )];
    let report = inspect_support(
        &f.projection,
        &f.contract,
        &mut r,
        &f.request(&exams, &f.bindings()),
        limits(),
    )
    .unwrap();
    not_supported(&report);
    assert_eq!(
        report.inspected_records()[0].validity_issue(),
        Some(&ValidityIssue::CandidateStale(
            BindingStaleness::ContractRevision
        ))
    );
    drop(report);
    let alias = CandidateBindingContract::new(
        TaskContract::new(
            f.verification.task().id().clone(),
            "changed objective",
            "changed criterion",
        )
        .unwrap(),
        rev(1),
        f.verification.candidate().clone(),
    );
    let exams = vec![RecordExamination::new(
        r[0].observation().id().clone(),
        &alias,
        Some(&f.current),
        Some(&f.profile),
    )];
    let before = r.clone();
    assert_eq!(
        inspect_support(
            &f.projection,
            &f.contract,
            &mut r,
            &f.request(&exams, &f.bindings()),
            limits()
        )
        .unwrap_err(),
        SupportInputError::ContractAlias
    );
    assert_eq!(r, before);
    let b = &f.bindings()[0];
    let bindings = vec![WitnessBinding::new(
        b.predicate().clone(),
        b.evidence().clone(),
        ArtifactReference::new(
            b.artifact().id().clone(),
            rev(2),
            b.artifact().candidate().clone(),
        ),
    )];
    let mut fresh = vec![f.record()];
    let report = inspect_support(
        &f.projection,
        &f.contract,
        &mut fresh,
        &f.request(&f.exams(), &bindings),
        limits(),
    )
    .unwrap();
    assert!(has_issue(&report, WitnessIssue::ArtifactMismatch));
}

#[test]
fn a_13_source_profile_changes_stale_but_target_mismatch_does_not_poison_source() {
    let f = Fixture::new();
    let provenance = f.profile.provenance();
    let profiles = vec![
        profile("2"),
        VerificationProfile::new(
            Provenance::new(
                provenance.mechanism(),
                provenance.inputs(),
                "different configuration",
            )
            .unwrap(),
            "1",
            f.profile.environment(),
            &[("component", "1")],
        )
        .unwrap(),
        VerificationProfile::new(
            provenance.clone(),
            "1",
            "another environment",
            &[("component", "1")],
        )
        .unwrap(),
        VerificationProfile::new(
            provenance.clone(),
            "1",
            f.profile.environment(),
            &[("component", "2")],
        )
        .unwrap(),
    ];
    for p in &profiles {
        let mut r = vec![f.record()];
        let exams = vec![RecordExamination::new(
            r[0].observation().id().clone(),
            &f.verification,
            Some(&f.current),
            Some(p),
        )];
        let report = inspect_support(
            &f.projection,
            &f.contract,
            &mut r,
            &f.request(&exams, &f.bindings()),
            limits(),
        )
        .unwrap();
        not_supported(&report);
        assert_eq!(
            report.inspected_records()[0].validity_issue(),
            Some(&ValidityIssue::ProfileChanged)
        );
    }
    let rule = f.remap(
        2,
        vec![PredicateCorrespondence::new(
            PredicateId::new("pAB").unwrap(),
            f.verification.clone(),
            profiles[0].clone(),
            CriterionCoverage::ExactSingleCriterion,
        )],
    );
    let mut r = vec![f.record()];
    let before = r.clone();
    let report = inspect_support(
        &f.projection,
        &rule,
        &mut r,
        &f.request(&f.exams(), &f.bindings()),
        limits(),
    )
    .unwrap();
    not_supported(&report);
    assert!(has_issue(&report, WitnessIssue::ProfileMismatch));
    assert_eq!(
        report.inspected_records()[0].validity(),
        Some(Validity::Current)
    );
    drop(report);
    assert_eq!(r, before);
}
#[test]
fn a_14_missing_incomplete_indeterminate_contexts_stay_unestablished() {
    let f = Fixture::new();
    let complete = Head::Detached {
        commit: candidate("1"),
    };
    let captures = [
        capture_state(
            &f.verification,
            3,
            complete.clone(),
            ObservationCoverage::Incomplete,
            ObservationConsistency::TwoMatchingSamples,
            true,
        ),
        capture_state(
            &f.verification,
            3,
            complete.clone(),
            ObservationCoverage::GitStatus,
            ObservationConsistency::Indeterminate,
            true,
        ),
        capture_state(
            &f.verification,
            3,
            complete,
            ObservationCoverage::GitStatus,
            ObservationConsistency::TwoMatchingSamples,
            false,
        ),
    ];
    for (c, issue) in captures.iter().zip([
        BindingInsufficiency::IncompleteCoverage,
        BindingInsufficiency::IndeterminateConsistency,
        BindingInsufficiency::MissingComparisonBasis,
    ]) {
        let mut r = vec![f.record()];
        let exams = vec![RecordExamination::new(
            r[0].observation().id().clone(),
            &f.verification,
            Some(c),
            Some(&f.profile),
        )];
        let report = inspect_support(
            &f.projection,
            &f.contract,
            &mut r,
            &f.request(&exams, &f.bindings()),
            limits(),
        )
        .unwrap();
        not_supported(&report);
        assert_eq!(report.inspected_records()[0].validity(), None);
        assert_eq!(
            report.inspected_records()[0].validity_issue(),
            Some(&ValidityIssue::CandidateInsufficient(issue))
        );
    }
    for (candidate, p) in [(None, Some(&f.profile)), (Some(&f.current), None)] {
        let mut r = vec![f.record()];
        let exams = vec![RecordExamination::new(
            r[0].observation().id().clone(),
            &f.verification,
            candidate,
            p,
        )];
        let report = inspect_support(
            &f.projection,
            &f.contract,
            &mut r,
            &f.request(&exams, &f.bindings()),
            limits(),
        )
        .unwrap();
        not_supported(&report);
        assert_eq!(report.inspected_records()[0].validity(), None);
    }
    for (p, c) in [(None, Some(&f.consumer)), (Some(&f.producer), None)] {
        let mut r = vec![f.record()];
        let exams = f.exams();
        let bindings = f.bindings();
        let report = inspect_support(
            &f.projection,
            &f.contract,
            &mut r,
            &SupportRequest::new(p, c, &exams, &bindings),
            limits(),
        )
        .unwrap();
        not_supported(&report);
        assert_eq!(
            report.inspected_records()[0].validity(),
            Some(Validity::Current)
        );
        assert!(report.findings()[0].reasons().iter().any(|r| matches!(
            r,
            SupportReason::ProducerContext(_) | SupportReason::ConsumerContext(_)
        )));
    }
    let mut r = vec![f.record()];
    let report = inspect_support(
        &f.projection,
        &f.contract,
        &mut r,
        &f.request(&[], &f.bindings()),
        limits(),
    )
    .unwrap();
    not_supported(&report);
    assert!(!report.inspected_records()[0].was_inspected());
}
#[test]
fn a_15_known_stale_cannot_revive_after_restoring_old_values() {
    let f = Fixture::new();
    let changed = profile("changed");
    let mut r = vec![f.record()];
    let exams = vec![RecordExamination::new(
        r[0].observation().id().clone(),
        &f.verification,
        Some(&f.current),
        Some(&changed),
    )];
    let report = inspect_support(
        &f.projection,
        &f.contract,
        &mut r,
        &f.request(&exams, &f.bindings()),
        limits(),
    )
    .unwrap();
    not_supported(&report);
    drop(report);
    let report = f.inspect(&mut r);
    not_supported(&report);
    assert_eq!(
        report.inspected_records()[0].result(),
        Some(AssessmentResult::Pass)
    );
    assert_eq!(
        report.inspected_records()[0].validity(),
        Some(Validity::Stale)
    );
    assert_eq!(
        report.inspected_records()[0].validity_issue(),
        Some(&ValidityIssue::ProfileChanged)
    );
}
#[test]
fn a_16_explicit_first_invalidation_is_retained() {
    let f = Fixture::new();
    let mut r = vec![f.record()];
    r[0].invalidate("private-canary invalidation").unwrap();
    let before = r.clone();
    let report = f.inspect(&mut r);
    not_supported(&report);
    assert_eq!(
        report.inspected_records()[0].validity_issue(),
        Some(&ValidityIssue::ExplicitInvalidation)
    );
    assert_eq!(
        report.inspected_records()[0].record().invalidation_reason(),
        Some("private-canary invalidation")
    );
    drop(report);
    assert_eq!(r, before);
    assert_eq!(
        r[0].invalidate("another reason"),
        Err(SemanticError::AlreadyInvalidated)
    );
}
#[test]
fn a_17_ambiguous_ids_refuse_atomically_and_compound_assessment_neighbors_work() {
    let f = Fixture::new();
    let mut r = vec![f.record(), f.record_kind(1, passed(), None)];
    let before = r.clone();
    assert_eq!(
        inspect_support(
            &f.projection,
            &f.contract,
            &mut r,
            &f.request(&f.exams(), &f.bindings()),
            limits()
        )
        .unwrap_err(),
        SupportInputError::DuplicateRecordIdentity
    );
    assert_eq!(r, before);
    let mut r = vec![f.record()];
    let before = r.clone();
    let altered = capture_state(
        &f.verification,
        2,
        Head::Detached {
            commit: candidate("9"),
        },
        ObservationCoverage::GitStatus,
        ObservationConsistency::TwoMatchingSamples,
        true,
    );
    let exams = vec![RecordExamination::new(
        r[0].observation().id().clone(),
        &f.verification,
        Some(&altered),
        Some(&f.profile),
    )];
    assert_eq!(
        inspect_support(
            &f.projection,
            &f.contract,
            &mut r,
            &f.request(&exams, &f.bindings()),
            limits()
        )
        .unwrap_err(),
        SupportInputError::IdentityConflict
    );
    assert_eq!(r, before);
    let mut r = vec![
        f.record(),
        f.record_kind(
            2,
            passed(),
            Some((AssessmentResult::Pass, Applicability::Applicable)),
        ),
    ];
    let mut bindings = f.bindings();
    let mut exams = f.exams();
    bindings.push(WitnessBinding::new(
        PredicateId::new("pAB").unwrap(),
        EvidenceReference::new(
            r[1].observation().id().clone(),
            r[1].assessment().map(|a| a.id().clone()),
        ),
        f.contract.scope().requirement().artifact().clone(),
    ));
    exams.push(RecordExamination::new(
        r[1].observation().id().clone(),
        &f.verification,
        Some(&f.current),
        Some(&f.profile),
    ));
    let report = inspect_support(
        &f.projection,
        &f.contract,
        &mut r,
        &f.request(&exams, &bindings),
        limits(),
    )
    .unwrap();
    assert_eq!(
        report.evidence_part(),
        EvidencePart::AllRequiredPredicatesSupported
    );
    assert_eq!(report.findings()[0].witnesses().len(), 2);
}
#[test]
fn a_18_sibling_edge_mapping_is_independent_despite_shared_artifact() {
    let first = Fixture::for_edge("BC", &["pAB"]);
    let sibling = Fixture::for_edge("BF", &["pAB"]);
    let mut r = vec![first.record()];
    assert_eq!(
        first.inspect(&mut r).evidence_part(),
        EvidencePart::AllRequiredPredicatesSupported
    );
    let broader = CandidateBindingContract::new(
        TaskContract::new(task("V.broader"), "Different", "All behavior is verified").unwrap(),
        rev(1),
        sibling.verification.candidate().clone(),
    );
    let rule = sibling.remap(
        2,
        vec![PredicateCorrespondence::new(
            PredicateId::new("pBF").unwrap(),
            broader,
            sibling.profile.clone(),
            CriterionCoverage::ExactSingleCriterion,
        )],
    );
    let report = inspect_support(
        &sibling.projection,
        &rule,
        &mut r,
        &sibling.request(&sibling.exams(), &sibling.bindings()),
        limits(),
    )
    .unwrap();
    not_supported(&report);
    assert!(has_issue(&report, WitnessIssue::CriterionMismatch));
    drop(report);
    // An independently declared identical complete criterion can legitimately support both.
    assert_eq!(
        sibling.inspect(&mut r).evidence_part(),
        EvidencePart::AllRequiredPredicatesSupported
    );
    assert!(matches!(
        first.projection.edge(&edge("BC")).unwrap().decision(),
        EdgeDecision::Held(_)
    ));
    assert!(matches!(
        sibling.projection.edge(&edge("BF")).unwrap().decision(),
        EdgeDecision::Held(_)
    ));
}
#[test]
fn a_19_all_predicates_and_explicit_witnesses_are_required() {
    let f = Fixture::for_edge("AB", &["pAB", "pOther"]);
    let mut r = vec![f.record()];
    let bindings = f.bindings();
    let report = inspect_support(
        &f.projection,
        &f.contract,
        &mut r,
        &f.request(&f.exams(), &bindings[..1]),
        limits(),
    )
    .unwrap();
    not_supported(&report);
    assert_eq!(
        report.findings()[0].disposition(),
        SupportDisposition::SupportedWithinDeclaredContract
    );
    assert!(
        report.findings()[1]
            .reasons()
            .contains(&SupportReason::MissingWitness)
    );
    drop(report);
    assert_eq!(
        f.inspect(&mut r).evidence_part(),
        EvidencePart::AllRequiredPredicatesSupported
    );
    let mut incomplete = bindings.clone();
    incomplete.push(WitnessBinding::new(
        PredicateId::new("pAB").unwrap(),
        EvidenceReference::new(ObservationId::new("absent-report", 1).unwrap(), None),
        f.contract.scope().requirement().artifact().clone(),
    ));
    let report = inspect_support(
        &f.projection,
        &f.contract,
        &mut r,
        &f.request(&f.exams(), &incomplete),
        limits(),
    )
    .unwrap();
    not_supported(&report);
    assert!(has_issue(&report, WitnessIssue::MissingRecord));
}
#[test]
fn a_20_upstream_invalidation_changes_only_affected_semantic_scopes() {
    let mut f = Fixture::for_edge("BC", &["pAB"]);
    let d = Fixture::for_edge("DE", &["pAB"]);
    let mut r = vec![f.record()];
    let mut dr = vec![d.record()];
    let basis = f.inspect(&mut r).basis().clone();
    let independent = d.inspect(&mut dr).basis().clone();
    let root = f.projection.scope(&task("A")).unwrap();
    apply(
        &mut f.projection,
        "invalidate-A",
        Event::Invalidate {
            roots: vec![root],
            cause: source("changed upstream"),
            coverage: DependencyCoverage::DeclaredComplete,
        },
    );
    assert_eq!(
        basis.check_scope(&f.projection, &f.contract),
        Err(SupportInputError::ContextChanged)
    );
    assert!(independent.check_scope(&f.projection, &d.contract).is_ok());
    assert_eq!(f.projection.scope(&task("B")).unwrap().generation(), rev(2));
    assert_eq!(f.projection.scope(&task("F")).unwrap().generation(), rev(2));
    assert_eq!(f.projection.scope(&task("D")).unwrap().generation(), rev(1));
    let before = r.clone();
    assert_eq!(
        inspect_support(
            &f.projection,
            &f.contract,
            &mut r,
            &f.request(&f.exams(), &f.bindings()),
            limits()
        )
        .unwrap_err(),
        SupportInputError::ContextChanged
    );
    assert_eq!(r, before);
}
#[test]
fn a_21_revalidation_uses_new_record_and_assessment_without_reviving_original() {
    let f = Fixture::new();
    let mut old = f.record();
    old.invalidate("known stale source").unwrap();
    let old_before = old.clone();
    let new = f.record_kind(
        2,
        passed(),
        Some((AssessmentResult::Pass, Applicability::Applicable)),
    );
    let mut r = vec![old, new];
    let id = r[1].observation().id().clone();
    let bindings = vec![WitnessBinding::new(
        PredicateId::new("pAB").unwrap(),
        EvidenceReference::new(id.clone(), r[1].assessment().map(|a| a.id().clone())),
        f.contract.scope().requirement().artifact().clone(),
    )];
    let exams = vec![RecordExamination::new(
        id,
        &f.verification,
        Some(&f.current),
        Some(&f.profile),
    )];
    let report = inspect_support(
        &f.projection,
        &f.contract,
        &mut r,
        &f.request(&exams, &bindings),
        limits(),
    )
    .unwrap();
    assert_eq!(
        report.evidence_part(),
        EvidencePart::AllRequiredPredicatesSupported
    );
    assert_eq!(report.inspected_records().len(), 1);
    drop(report);
    assert_eq!(r[0], old_before);
}
#[test]
fn a_22_producer_approval_text_cannot_manufacture_an_assessment_or_review() {
    let f = Fixture::new();
    let graph = f.projection.clone();
    let mut r = vec![f.record_kind(1, passed(), None)];
    not_supported(&f.inspect(&mut r));
    assert!(r[0].assessment().is_none());
    let mut r = vec![f.record()];
    let report = f.inspect(&mut r);
    assert_eq!(report.authority(), ExecutionAuthority::Unavailable);
    assert_eq!(f.projection, graph);
}
#[test]
fn a_23_supported_evidence_has_no_operational_authorization_or_dispatch() {
    let f = Fixture::new();
    let mut r = vec![f.record()];
    let before = f.projection.clone();
    let report = f.inspect(&mut r);
    assert_eq!(
        report.evidence_part(),
        EvidencePart::AllRequiredPredicatesSupported
    );
    assert_eq!(report.authority(), ExecutionAuthority::Unavailable);
    assert_eq!(f.projection, before);
    assert!(!f.projection.mission(&task("B")).unwrap().eligible());
}
#[test]
fn a_24_late_malformed_input_does_not_partially_publish_staleness() {
    let f = Fixture::new();
    let mut r = vec![f.record(), f.record_kind(2, passed(), None)];
    let before = r.clone();
    let changed = profile("2");
    let alias = CandidateBindingContract::new(
        TaskContract::new(task("V.head"), "bad alias", "changed").unwrap(),
        rev(1),
        f.verification.candidate().clone(),
    );
    let exams = vec![
        RecordExamination::new(
            r[0].observation().id().clone(),
            &f.verification,
            Some(&f.current),
            Some(&changed),
        ),
        RecordExamination::new(
            r[1].observation().id().clone(),
            &alias,
            Some(&f.current),
            Some(&f.profile),
        ),
    ];
    assert_eq!(
        inspect_support(
            &f.projection,
            &f.contract,
            &mut r,
            &f.request(&exams, &f.bindings()),
            limits()
        )
        .unwrap_err(),
        SupportInputError::ContractAlias
    );
    assert_eq!(r, before);
    let report = inspect_support(
        &f.projection,
        &f.contract,
        &mut r,
        &f.request(&exams[..1], &f.bindings()),
        limits(),
    )
    .unwrap();
    not_supported(&report);
    assert_eq!(
        report.inspected_records()[0].validity_issue(),
        Some(&ValidityIssue::ProfileChanged)
    );
    drop(report);
    assert_eq!(r[0].observation(), before[0].observation());
    assert_eq!(r[0].assessment(), before[0].assessment());
    assert_eq!(r[1], before[1]);
    assert_ne!(r[0], before[0]);
}
#[test]
fn a_25_new_debug_and_error_diagnostics_redact_private_context() {
    let f = Fixture::new();
    let mut r = vec![f.record()];
    r[0].invalidate("private-canary secret path /sensitive")
        .unwrap();
    let exams = f.exams();
    let bindings = f.bindings();
    let request = f.request(&exams, &bindings);
    let report = inspect_support(&f.projection, &f.contract, &mut r, &request, limits()).unwrap();
    let mut outputs = vec![
        format!("{:?}", f.contract),
        format!("{:?}", f.contract.scope()),
        format!("{:?}", f.contract.correspondences()[0]),
        format!("{request:?}"),
        format!("{:?}", exams[0]),
        format!("{:?}", bindings[0]),
        format!("{:?}", bindings[0].evidence()),
        format!("{report:?}"),
        format!("{:?}", report.basis()),
        format!("{:?}", report.basis().examinations()[0]),
        format!("{:?}", report.findings()[0]),
        format!("{:?}", report.inspected_records()[0]),
    ];
    for error in [
        SupportInputError::ContextChanged,
        SupportInputError::ContractAlias,
        SupportInputError::IdentityConflict,
        SupportInputError::ConflictingExamination,
        SupportInputError::IncompatibleReference,
        SupportInputError::DuplicateRecordIdentity,
        SupportInputError::UnknownPredicate,
        SupportInputError::DuplicateCorrespondence,
        SupportInputError::CorrespondenceChanged,
        SupportInputError::InvalidLimits,
        SupportInputError::Allocation,
        SupportInputError::Limit(SupportLimitKind::MaterialBytes),
        SupportInputError::Binding(BindingError::ContractRevisionReused),
        SupportInputError::Core(ReadinessError::MissingReference),
    ] {
        outputs.push(format!("{error:?}: {error}"));
    }
    for output in outputs {
        assert!(!output.contains("private-canary"));
        assert!(!output.contains("/sensitive"));
    }
    assert!(
        report.inspected_records()[0]
            .record()
            .assessment()
            .unwrap()
            .rationale()
            .contains("private-canary")
    );
    assert!(report.findings()[0].predicate().as_str().contains("pAB"));
}
#[test]
fn a_26_missing_profile_can_recover_but_known_staleness_cannot() {
    let f = Fixture::new();
    let mut r = vec![f.record()];
    let exams = vec![RecordExamination::new(
        r[0].observation().id().clone(),
        &f.verification,
        Some(&f.current),
        None,
    )];
    let report = inspect_support(
        &f.projection,
        &f.contract,
        &mut r,
        &f.request(&exams, &f.bindings()),
        limits(),
    )
    .unwrap();
    not_supported(&report);
    assert_eq!(
        report.inspected_records()[0].validity_issue(),
        Some(&ValidityIssue::MissingProfile)
    );
    drop(report);
    assert_eq!(
        f.inspect(&mut r).evidence_part(),
        EvidencePart::AllRequiredPredicatesSupported
    );
    r[0].invalidate("now known stale").unwrap();
    let report = inspect_support(
        &f.projection,
        &f.contract,
        &mut r,
        &f.request(&exams, &f.bindings()),
        limits(),
    )
    .unwrap();
    not_supported(&report);
    assert_eq!(
        report.inspected_records()[0].validity_issue(),
        Some(&ValidityIssue::ExplicitInvalidation)
    );
}
#[test]
fn a_27_unreferenced_records_and_narrow_source_are_not_poisoned() {
    let f = Fixture::new();
    let mut r = vec![f.record(), f.record_kind(2, passed(), None)];
    let before = r.clone();
    let wrong = profile("2");
    let mut exams = f.exams();
    exams.push(RecordExamination::new(
        r[1].observation().id().clone(),
        &f.verification,
        Some(&f.current),
        Some(&wrong),
    ));
    let report = inspect_support(
        &f.projection,
        &f.contract,
        &mut r,
        &f.request(&exams, &f.bindings()),
        limits(),
    )
    .unwrap();
    assert_eq!(report.inspected_records().len(), 1);
    drop(report);
    assert_eq!(r, before);
    let rule = f.remap(2, vec![]);
    let report = inspect_support(
        &f.projection,
        &rule,
        &mut r,
        &f.request(&f.exams(), &f.bindings()),
        limits(),
    )
    .unwrap();
    not_supported(&report);
    assert!(
        report.findings()[0]
            .reasons()
            .contains(&SupportReason::MissingCorrespondence)
    );
    assert_eq!(
        report.inspected_records()[0].validity(),
        Some(Validity::Current)
    );
}
#[test]
fn a_28_exact_logical_limits_and_immediate_neighbors_refuse_atomically() {
    let f = Fixture::new();
    let mut r = vec![f.record()];
    let usage = f.inspect(&mut r).input_usage();
    let bytes = usage.get(SupportLimitKind::MaterialBytes);
    let exact = SupportLimits::new(1, 1, 1, bytes).unwrap();
    assert_eq!(
        inspect_support(
            &f.projection,
            &f.contract,
            &mut r,
            &f.request(&f.exams(), &f.bindings()),
            exact
        )
        .unwrap()
        .evidence_part(),
        EvidencePart::AllRequiredPredicatesSupported
    );
    let before = r.clone();
    assert_eq!(
        inspect_support(
            &f.projection,
            &f.contract,
            &mut r,
            &f.request(&f.exams(), &f.bindings()),
            SupportLimits::new(1, 1, 1, bytes - 1).unwrap()
        )
        .unwrap_err(),
        SupportInputError::Limit(SupportLimitKind::MaterialBytes)
    );
    assert_eq!(r, before);
    let mut bindings = f.bindings();
    bindings.push(bindings[0].clone());
    assert_eq!(
        inspect_support(
            &f.projection,
            &f.contract,
            &mut r,
            &f.request(&f.exams(), &bindings),
            exact
        )
        .unwrap_err(),
        SupportInputError::Limit(SupportLimitKind::Bindings)
    );
    assert_eq!(r, before);
    let mut extra = r.clone();
    extra.push(f.record_kind(2, passed(), None));
    let prior = extra.clone();
    assert_eq!(
        inspect_support(
            &f.projection,
            &f.contract,
            &mut extra,
            &f.request(&f.exams(), &f.bindings()),
            exact
        )
        .unwrap_err(),
        SupportInputError::Limit(SupportLimitKind::Records)
    );
    assert_eq!(extra, prior);
    let two = Fixture::for_edge("AB", &["pAB", "pOther"]);
    let mut records = vec![two.record()];
    let prior = records.clone();
    assert_eq!(
        inspect_support(
            &two.projection,
            &two.contract,
            &mut records,
            &two.request(&two.exams(), &two.bindings()),
            SupportLimits::new(32, 32, 1, 4_000_000).unwrap()
        )
        .unwrap_err(),
        SupportInputError::Limit(SupportLimitKind::Predicates)
    );
    assert_eq!(records, prior);
    assert_eq!(
        inspect_support(
            &two.projection,
            &two.contract,
            &mut records,
            &two.request(&two.exams(), &two.bindings()),
            SupportLimits::new(1, 2, 2, 4_000_000).unwrap()
        )
        .unwrap()
        .evidence_part(),
        EvidencePart::AllRequiredPredicatesSupported
    );
    // A large nested current comparison buffer must be charged before inspection.
    let large = CandidateObservation::new(
        f.current.id().clone(),
        &f.verification,
        f.current.state().clone(),
        Some(ComparisonBasis::new("private-canary comparison", vec![b'x'; 4000]).unwrap()),
    );
    let exams = vec![RecordExamination::new(
        r[0].observation().id().clone(),
        &f.verification,
        Some(&large),
        Some(&f.profile),
    )];
    assert_eq!(
        inspect_support(
            &f.projection,
            &f.contract,
            &mut r,
            &f.request(&exams, &f.bindings()),
            exact
        )
        .unwrap_err(),
        SupportInputError::Limit(SupportLimitKind::MaterialBytes)
    );
    assert_eq!(r, before);
}
#[test]
fn a_29_omitted_or_wrong_assessment_reference_is_not_automatically_selected() {
    let f = Fixture::new();
    let mut r = vec![f.record()];
    for id in [None, Some(AssessmentId::new("other-judgment").unwrap())] {
        let bindings = vec![WitnessBinding::new(
            PredicateId::new("pAB").unwrap(),
            EvidenceReference::new(r[0].observation().id().clone(), id.clone()),
            f.contract.scope().requirement().artifact().clone(),
        )];
        let report = inspect_support(
            &f.projection,
            &f.contract,
            &mut r,
            &f.request(&f.exams(), &bindings),
            limits(),
        )
        .unwrap();
        not_supported(&report);
        assert!(has_issue(
            &report,
            if id.is_none() {
                WitnessIssue::MissingAssessmentReference
            } else {
                WitnessIssue::AssessmentMismatch
            }
        ));
    }
    assert_eq!(
        f.inspect(&mut r).evidence_part(),
        EvidencePart::AllRequiredPredicatesSupported
    );
}
#[test]
fn a_30_readiness_level_and_mission_kind_are_scope_not_evidence_grants() {
    let f = Fixture::new();
    let mut r = vec![f.record()];
    let basis = f.inspect(&mut r).basis().clone();
    for (kind, level) in [
        (DependencyKind::Artifact, ReadinessLevel::ContractStable),
        (
            DependencyKind::Artifact,
            ReadinessLevel::ImplementationProvisional,
        ),
        (
            DependencyKind::MissionAcceptance,
            ReadinessLevel::IntegratedAccepted,
        ),
    ] {
        let g = f.projection.graph();
        let edges = g
            .edges()
            .iter()
            .map(|e| {
                if e.id() != &edge("AB") {
                    return e.clone();
                }
                DependencyEdge::new(
                    e.id().clone(),
                    e.producer().clone(),
                    e.consumer().clone(),
                    ArtifactRequirement::new(
                        e.requirement().artifact().clone(),
                        kind,
                        level,
                        e.requirement().predicates().to_vec(),
                    )
                    .unwrap(),
                )
            })
            .collect();
        let p = ReadinessProjection::new(
            DependencyGraph::new(
                GraphReference::new(g.reference().id().clone(), rev(2)),
                g.missions().to_vec(),
                edges,
                g.limits(),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(
            basis.check_scope(&p, &f.contract),
            Err(SupportInputError::ContextChanged)
        );
        let c = EdgeEvidenceContract::new(
            EdgeScopeSnapshot::capture(&p, &edge("AB")).unwrap(),
            rev(1),
            f.verification.candidate().clone(),
            f.contract.correspondences().to_vec(),
            source("private-canary mapping"),
            limits(),
        )
        .unwrap();
        let report = inspect_support(
            &p,
            &c,
            &mut r,
            &f.request(&f.exams(), &f.bindings()),
            limits(),
        )
        .unwrap();
        assert_eq!(
            report.evidence_part(),
            EvidencePart::AllRequiredPredicatesSupported
        );
        assert_eq!(report.authority(), ExecutionAuthority::Unavailable);
        assert!(matches!(
            p.edge(&edge("AB")).unwrap().decision(),
            EdgeDecision::Held(_)
        ));
        assert_eq!(
            p.mission(&task("A")).unwrap().status(),
            MissionStatus::Pending
        );
    }
}
#[test]
fn a_31_scope_recheck_rejects_aliases_new_revisions_and_ignores_unrelated_cursors() {
    let mut f = Fixture::new();
    let mut r = vec![f.record()];
    let basis = f.inspect(&mut r).basis().clone();
    let changed = f.remap(
        1,
        vec![PredicateCorrespondence::new(
            PredicateId::new("pAB").unwrap(),
            f.verification.clone(),
            f.profile.clone(),
            CriterionCoverage::Unestablished,
        )],
    );
    assert_eq!(
        basis.check_scope(&f.projection, &changed),
        Err(SupportInputError::ContractAlias)
    );
    let next = f.remap(2, f.contract.correspondences().to_vec());
    assert_eq!(
        basis.check_scope(&f.projection, &next),
        Err(SupportInputError::CorrespondenceChanged)
    );
    apply(
        &mut f.projection,
        "info",
        Event::Signal {
            mission: task("D"),
            message: "information only".to_owned(),
        },
    );
    apply(
        &mut f.projection,
        "capacity",
        Event::Capacity {
            available: 0,
            source: source("finite capacity"),
        },
    );
    let new_scope = EdgeScopeSnapshot::capture(&f.projection, &edge("AB")).unwrap();
    assert_ne!(
        new_scope.notification_cursor(),
        f.contract.scope().notification_cursor()
    );
    let refreshed = EdgeEvidenceContract::new(
        new_scope,
        rev(1),
        f.verification.candidate().clone(),
        f.contract.correspondences().to_vec(),
        f.contract.basis().clone(),
        limits(),
    )
    .unwrap();
    assert!(basis.check_scope(&f.projection, &refreshed).is_ok());
    let a = f.projection.scope(&task("A")).unwrap();
    apply(
        &mut f.projection,
        "change",
        Event::Invalidate {
            roots: vec![a],
            cause: source("new context"),
            coverage: DependencyCoverage::DeclaredComplete,
        },
    );
    assert_eq!(
        basis.check_scope(&f.projection, &refreshed),
        Err(SupportInputError::ContextChanged)
    );
    let c = EdgeEvidenceContract::new(
        EdgeScopeSnapshot::capture(&f.projection, &edge("AB")).unwrap(),
        rev(1),
        f.verification.candidate().clone(),
        f.contract.correspondences().to_vec(),
        f.contract.basis().clone(),
        limits(),
    )
    .unwrap();
    let report = inspect_support(
        &f.projection,
        &c,
        &mut r,
        &f.request(&f.exams(), &f.bindings()),
        limits(),
    )
    .unwrap();
    assert_eq!(
        report.evidence_part(),
        EvidencePart::AllRequiredPredicatesSupported
    );
}
#[test]
fn a_32_sticky_cause_and_requested_context_are_retained_distinctly() {
    let f = Fixture::new();
    let mut r = vec![f.record()];
    let original = r[0].clone();
    let changed = profile("2");
    let exams = vec![RecordExamination::new(
        r[0].observation().id().clone(),
        &f.verification,
        Some(&f.current),
        Some(&changed),
    )];
    drop(
        inspect_support(
            &f.projection,
            &f.contract,
            &mut r,
            &f.request(&exams, &f.bindings()),
            limits(),
        )
        .unwrap(),
    );
    let latest = capture(&f.verification, 4);
    let exams = vec![RecordExamination::new(
        r[0].observation().id().clone(),
        &f.verification,
        Some(&latest),
        Some(&f.profile),
    )];
    let report = inspect_support(
        &f.projection,
        &f.contract,
        &mut r,
        &f.request(&exams, &f.bindings()),
        limits(),
    )
    .unwrap();
    not_supported(&report);
    assert_eq!(
        report.inspected_records()[0].compared_to(),
        Some(f.current.id())
    );
    assert_eq!(
        report.basis().examinations()[0].candidate().unwrap().id(),
        latest.id()
    );
    assert_eq!(
        report.inspected_records()[0].record().observation(),
        original.observation()
    );
    assert_eq!(
        report.inspected_records()[0].record().assessment(),
        original.assessment()
    );
}

#[test]
fn duplicate_notifications_are_redundant_but_charged_and_missing_material_is_visible() {
    let f = Fixture::new();
    let mut r = vec![f.record()];
    let mut bindings = f.bindings();
    bindings.push(bindings[0].clone());
    let mut exams = f.exams();
    exams.push(exams[0].clone());
    let report = inspect_support(
        &f.projection,
        &f.contract,
        &mut r,
        &f.request(&exams, &bindings),
        limits(),
    )
    .unwrap();
    assert_eq!(
        report.evidence_part(),
        EvidencePart::AllRequiredPredicatesSupported
    );
    assert_eq!(report.findings()[0].witnesses().len(), 1);
    assert_eq!(report.inspected_records().len(), 1);
    assert_eq!(report.input_usage().get(SupportLimitKind::Bindings), 2);
    drop(report);
    let mut empty = [];
    let report = f.inspect(&mut empty);
    not_supported(&report);
    assert!(has_issue(&report, WitnessIssue::MissingRecord));
}
#[test]
fn malformed_reference_and_context_variants_are_atomic_with_valid_neighbors() {
    let f = Fixture::new();
    assert_eq!(
        EdgeScopeSnapshot::capture(&f.projection, &edge("absent")).unwrap_err(),
        SupportInputError::Core(ReadinessError::MissingReference)
    );
    let mut r = vec![f.record()];
    let before = r.clone();
    let changed = profile("2");
    let mut exams = f.exams();
    exams.push(RecordExamination::new(
        r[0].observation().id().clone(),
        &f.verification,
        Some(&f.current),
        Some(&changed),
    ));
    assert_eq!(
        inspect_support(
            &f.projection,
            &f.contract,
            &mut r,
            &f.request(&exams, &f.bindings()),
            limits()
        )
        .unwrap_err(),
        SupportInputError::ConflictingExamination
    );
    assert_eq!(r, before);
    let other = contract("V.other", "1", 1, "Another task");
    let exams = vec![RecordExamination::new(
        r[0].observation().id().clone(),
        &other,
        Some(&f.current),
        Some(&f.profile),
    )];
    assert_eq!(
        inspect_support(
            &f.projection,
            &f.contract,
            &mut r,
            &f.request(&exams, &f.bindings()),
            limits()
        )
        .unwrap_err(),
        SupportInputError::IncompatibleReference
    );
    assert_eq!(r, before);
    let bindings = vec![WitnessBinding::new(
        PredicateId::new("foreign").unwrap(),
        f.bindings()[0].evidence().clone(),
        f.contract.scope().requirement().artifact().clone(),
    )];
    assert_eq!(
        inspect_support(
            &f.projection,
            &f.contract,
            &mut r,
            &f.request(&f.exams(), &bindings),
            limits()
        )
        .unwrap_err(),
        SupportInputError::UnknownPredicate
    );
    assert_eq!(r, before);
    let mapping = f.contract.correspondences()[0].clone();
    assert_eq!(
        f.remap_result(vec![mapping.clone(), mapping]).unwrap_err(),
        SupportInputError::DuplicateCorrespondence
    );
    assert_eq!(
        f.remap_result(vec![PredicateCorrespondence::new(
            PredicateId::new("foreign").unwrap(),
            f.verification.clone(),
            f.profile.clone(),
            CriterionCoverage::ExactSingleCriterion
        )])
        .unwrap_err(),
        SupportInputError::UnknownPredicate
    );
}
impl Fixture {
    fn remap_result(
        &self,
        mappings: Vec<PredicateCorrespondence>,
    ) -> Result<EdgeEvidenceContract, SupportInputError> {
        EdgeEvidenceContract::new(
            self.contract.scope().clone(),
            rev(1),
            self.verification.candidate().clone(),
            mappings,
            source("private-canary mapping"),
            limits(),
        )
    }
}

#[test]
fn nested_record_detail_profile_and_assessment_rationale_are_charged() {
    let f = Fixture::new();
    let mut base = vec![f.record()];
    let bytes = f
        .inspect(&mut base)
        .input_usage()
        .get(SupportLimitKind::MaterialBytes);
    let exact = SupportLimits::new(32, 32, 16, bytes).unwrap();
    let old = base[0].observation();
    let mut long_detail = EvidenceRecord::new(
        ReportedObservation::new(
            old.id().clone(),
            old.subject().clone(),
            old.profile().clone(),
            old.times(),
            old.kind().clone(),
            &"d".repeat(4000),
        )
        .unwrap(),
    );
    assess(
        &mut long_detail,
        &f.verification,
        &f.profile,
        AssessmentResult::Pass,
        Applicability::Applicable,
    )
    .unwrap();
    let mut long_rationale = EvidenceRecord::new(old.clone());
    long_rationale
        .assess(
            AssessmentId::new("private-canary-assessment").unwrap(),
            UNIX_EPOCH + Duration::from_secs(102),
            AssessmentResult::Pass,
            Applicability::Applicable,
            &"r".repeat(4000),
            &ValidationContext::new(
                &f.verification,
                Some(&capture(&f.verification, 2)),
                Some(&f.profile),
            ),
        )
        .unwrap();
    let longer = VerificationProfile::new(
        Provenance::new(
            &"m".repeat(4000),
            f.profile.provenance().inputs(),
            f.profile.provenance().configuration(),
        )
        .unwrap(),
        "1",
        f.profile.environment(),
        &[("component", "1")],
    )
    .unwrap();
    let mut long_profile = raw_record(&f.verification, &longer, 1, passed());
    assess(
        &mut long_profile,
        &f.verification,
        &longer,
        AssessmentResult::Pass,
        Applicability::Applicable,
    )
    .unwrap();
    for record in [long_detail, long_rationale, long_profile] {
        let mut records = vec![record];
        let before = records.clone();
        assert_eq!(
            inspect_support(
                &f.projection,
                &f.contract,
                &mut records,
                &f.request(&f.exams(), &f.bindings()),
                exact
            )
            .unwrap_err(),
            SupportInputError::Limit(SupportLimitKind::MaterialBytes)
        );
        assert_eq!(records, before);
    }
}

#[test]
fn invalid_artifact_requirement_and_unknown_dependency_scope_cannot_grant_authority() {
    let f = Fixture::new();
    let wrong = CandidateRequirement::new(
        f.verification.candidate().repository().clone(),
        candidate("9"),
        WorktreeRequirement::Clean,
    );
    assert_eq!(
        EdgeEvidenceContract::new(
            f.contract.scope().clone(),
            rev(1),
            wrong,
            vec![],
            source("mapping"),
            limits()
        )
        .unwrap_err(),
        SupportInputError::IncompatibleReference
    );
    // Unknown mission dependency coverage is a G1 gate, not something a narrow
    // Evidence support finding may override or silently mark as complete.
    let g = f.projection.graph();
    let missions = g
        .missions()
        .iter()
        .map(|m| Mission::new(m.context().clone(), m.kind(), DependencyCoverage::Unknown))
        .collect();
    let p = ReadinessProjection::new(
        DependencyGraph::new(
            GraphReference::new(g.reference().id().clone(), rev(2)),
            missions,
            g.edges().to_vec(),
            g.limits(),
        )
        .unwrap(),
    )
    .unwrap();
    let c = EdgeEvidenceContract::new(
        EdgeScopeSnapshot::capture(&p, &edge("AB")).unwrap(),
        rev(1),
        f.verification.candidate().clone(),
        f.contract.correspondences().to_vec(),
        f.contract.basis().clone(),
        limits(),
    )
    .unwrap();
    let mut records = vec![f.record()];
    let before = p.clone();
    let report = inspect_support(
        &p,
        &c,
        &mut records,
        &f.request(&f.exams(), &f.bindings()),
        limits(),
    )
    .unwrap();
    assert_eq!(
        report.evidence_part(),
        EvidencePart::AllRequiredPredicatesSupported
    );
    assert_eq!(report.authority(), ExecutionAuthority::Unavailable);
    assert_eq!(p, before);
    assert!(
        p.mission(&task("B"))
            .unwrap()
            .blockers()
            .contains(&MissionBlocker::UnknownCoverage)
    );
}
