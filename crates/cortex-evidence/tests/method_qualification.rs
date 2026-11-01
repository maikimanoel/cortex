//! MI-01..34 consumer tests. Model-data fixtures are distinct from actual child executions.
use cortex_core::{
    CandidateId, TaskContract, TaskId, binding::*, methods::*, temporal::SourceReference,
};
use cortex_evidence::{ExecutionObservation, Provenance, methods::*, unified::*};
use std::{
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

fn limits() -> InspectionLimits {
    InspectionLimits::new(256, 256, 256, 4096, 8_000_000, 8192).unwrap()
}
fn definition_limits() -> DefinitionLimits {
    DefinitionLimits::new(16, 64, 128, 2048, 1_000_000).unwrap()
}
fn source(name: &str) -> SourceReference {
    SourceReference::operator_reported("test caller", name, "1").unwrap()
}
fn time(n: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(n)
}
fn repository() -> RepositoryIdentity {
    RepositoryIdentity::new(
        PathBuf::from("/synthetic/method-fixture"),
        PathBuf::from("/synthetic/method-fixture/.git"),
        PathBuf::from("/synthetic/method-fixture/.git"),
    )
    .unwrap()
}
fn commit(n: &str) -> CandidateId {
    CandidateId::from_git_commit(&n.repeat(40)).unwrap()
}
fn binding(id: &str, criterion: &str) -> CandidateBindingContract {
    CandidateBindingContract::new(
        TaskContract::new(
            TaskId::new(id).unwrap(),
            "Qualify one declared native mechanism case",
            criterion,
        )
        .unwrap(),
        ContractRevision::new(1).unwrap(),
        CandidateRequirement::new(repository(), commit("a"), WorktreeRequirement::Clean),
    )
}
fn capture(c: &CandidateBindingContract, n: u64, dirty: bool) -> CandidateObservation {
    CandidateObservation::new(
        ObservationId::new(c.task().id().as_str(), n).unwrap(),
        c,
        RepositoryState::new(
            Some(repository()),
            Head::Detached {
                commit: commit("a"),
            },
            if dirty {
                vec![Entry::Untracked {
                    path: b"changed".to_vec(),
                }]
            } else {
                vec![]
            },
            ObservationCoverage::GitStatus,
            ObservationConsistency::TwoMatchingSamples,
        )
        .unwrap(),
        Some(ComparisonBasis::new("synthetic model", b"fixed comparison".to_vec()).unwrap()),
    )
}
fn profile(d: &MethodDefinition, c: &QualificationCase) -> VerificationProfile {
    let mut components = vec![
        ("oracle-source".to_owned(), c.oracle().source().to_owned()),
        ("oracle-version".to_owned(), c.oracle().version().to_owned()),
        ("fixture-source".to_owned(), c.fixture().source().to_owned()),
        (
            "fixture-version".to_owned(),
            c.fixture().version().to_owned(),
        ),
    ];
    for (i, dep) in d.context().dependencies().iter().enumerate() {
        components.push((format!("dependency-{i}-source"), dep.source().to_owned()));
        components.push((format!("dependency-{i}-version"), dep.version().to_owned()));
    }
    VerificationProfile::new(
        Provenance::new(
            "native case fixture",
            "one explicit case",
            "independent literal-output oracle",
        )
        .unwrap(),
        d.mechanism().source().version(),
        &format!(
            "{}-{}; profile={}; synthetic candidate data, no authentication",
            std::env::consts::OS,
            std::env::consts::ARCH,
            if cfg!(debug_assertions) {
                "debug"
            } else {
                "release"
            }
        ),
        &components
            .iter()
            .map(|(a, b)| (a.as_str(), b.as_str()))
            .collect::<Vec<_>>(),
    )
    .unwrap()
}

fn execution(code: u32, checks: u64, oracle: Option<OracleConclusion>) -> ObservationKind {
    ObservationKind::Execution(
        VerificationOutcome::new(
            ExecutionObservation::Exited { code },
            checks,
            oracle.map(|o| {
                OracleEvaluation::new(o, "Literal expected observation for one declared case")
                    .unwrap()
            }),
        )
        .unwrap(),
    )
}
#[derive(Clone)]
struct CaseData {
    baseline: u64,
    claim: ClaimId,
    case: CaseId,
    binding: CandidateBindingContract,
    current: CandidateObservation,
    profile: VerificationProfile,
    id: ObservationId,
}
fn record(
    data: &CaseData,
    kind: ObservationKind,
    result: Option<(AssessmentResult, Applicability)>,
) -> EvidenceRecord {
    let mut r = EvidenceRecord::new(
        ReportedObservation::new(
            data.id.clone(),
            capture(&data.binding, data.baseline, false),
            data.profile.clone(),
            ObservationTimes::new(time(10), time(11)).unwrap(),
            kind,
            "Caller supplied model data unless the actual child-driver test supplies it",
        )
        .unwrap(),
    );
    if let Some((result, applicability)) = result {
        r.assess(
            AssessmentId::new("case-assessment").unwrap(),
            time(12),
            result,
            applicability,
            "One explicitly assessed case, no permission",
            &ValidationContext::new(&data.binding, Some(&data.current), Some(&data.profile)),
        )
        .unwrap();
    }
    r
}
struct Fixture {
    set: MethodSet,
    method: MethodReference,
    contract: QualificationContract,
    cases: Vec<CaseData>,
    records: Vec<EvidenceRecord>,
}
impl Fixture {
    fn new(index: usize) -> Self {
        let starter = starter_methods(definition_limits()).unwrap();
        Self::from_definition(starter.definitions()[index].clone())
    }
    fn from_definition(d: MethodDefinition) -> Self {
        let set = MethodSet::new(vec![d.clone()], definition_limits()).unwrap();
        let method = d.reference().clone();
        let mut cases = Vec::new();
        let mut mappings = Vec::new();
        let mut records = Vec::new();
        for cl in d.claims() {
            for case in cl.cases() {
                let c = binding(
                    &format!(
                        "check.{}.{}",
                        d.reference().id().as_str(),
                        case.id().as_str()
                    ),
                    case.expected(),
                );
                let p = profile(&d, case);
                let data = CaseData {
                    baseline: 1,
                    claim: cl.id().clone(),
                    case: case.id().clone(),
                    current: capture(&c, 2, false),
                    profile: p.clone(),
                    id: ObservationId::new(&format!("report.{}", case.id().as_str()), 1).unwrap(),
                    binding: c.clone(),
                };
                records.push(record(
                    &data,
                    execution(0, 1, Some(OracleConclusion::Satisfied)),
                    Some((AssessmentResult::Pass, Applicability::Applicable)),
                ));
                mappings.push(CaseCorrespondence::new(
                    cl.id().clone(),
                    case.id().clone(),
                    c,
                    p,
                    case.oracle().clone(),
                    CaseCoverage::ExactCriterion,
                ));
                cases.push(data);
            }
        }
        let contract = QualificationContract::new(
            &d,
            d.context().target().clone(),
            ContractRevision::new(1).unwrap(),
            mappings,
            source("explicit model correspondence"),
            limits(),
        )
        .unwrap();
        Self {
            set,
            method,
            contract,
            cases,
            records,
        }
    }
    fn witnesses(&self) -> Vec<CaseWitness> {
        self.cases
            .iter()
            .map(|d| {
                CaseWitness::new(
                    d.claim.clone(),
                    d.case.clone(),
                    EvidenceReference::new(
                        d.id.clone(),
                        Some(AssessmentId::new("case-assessment").unwrap()),
                    ),
                )
            })
            .collect()
    }
}
fn examinations(cases: &[CaseData]) -> Vec<EvidenceExamination<'_>> {
    cases
        .iter()
        .map(|d| {
            EvidenceExamination::new(d.id.clone(), &d.binding, Some(&d.current), Some(&d.profile))
        })
        .collect()
}
fn run(f: &mut Fixture) -> MethodInspection<'_, '_> {
    let w = f.witnesses();
    let e = examinations(&f.cases);
    let q = QualificationRequest::new(&f.contract, f.contract.scope(), &w, &e, None);
    inspect_method(&f.set, &f.method, Some(&q), &mut f.records, limits()).unwrap()
}
fn complete(r: &MethodInspection<'_, '_>) -> bool {
    r.claims().iter().all(ClaimInspection::requirements_met)
}
fn first<'a>(r: &'a MethodInspection<'_, '_>) -> &'a WitnessFinding {
    &r.claims()[0].cases()[0].witnesses()[0]
}

#[test]
fn mi08_unassessed_definition_and_mi20_mi21_no_authority() {
    let mut f = Fixture::new(0);
    let before = f.records.clone();
    let report = inspect_method(&f.set, &f.method, None, &mut f.records, limits()).unwrap();
    assert!(!complete(&report));
    assert!(report.is_advisory());
    assert_eq!(report.applicability(), Applicability::Uncertain);
    assert!(report.basis().contract().is_none());
    assert!(
        report.claims()[0]
            .cases()
            .iter()
            .all(|c| c.issues() == [QualificationIssue::MissingWitness])
    );
    assert_eq!(report.records(), before);
}
#[test]
fn mi11_model_complete_support_is_exact_and_mi24_inspection_has_no_effects() {
    let mut f = Fixture::new(0);
    let before = f.records.clone();
    let snapshot = f.set.clone();
    let report = run(&mut f);
    assert!(complete(&report));
    assert!(report.is_advisory());
    assert_eq!(first(&report).result(), Some(AssessmentResult::Pass));
    assert_eq!(first(&report).validity(), Some(Validity::Current));
    assert_eq!(report.records(), before);
    assert_eq!(
        report.definition().claims()[0].exclusions(),
        snapshot.definitions()[0].claims()[0].exclusions()
    );
    assert!(report.requirements().len() == 1);
    assert_eq!(f.set, snapshot);
}
#[test]
fn mi09_not_run_blocked_and_unassessed_do_not_support() {
    for (kind, result) in [
        (
            ObservationKind::Execution(
                VerificationOutcome::new(ExecutionObservation::NotRun, 0, None).unwrap(),
            ),
            Some((AssessmentResult::NotRun, Applicability::Uncertain)),
        ),
        (
            ObservationKind::Execution(
                VerificationOutcome::new(ExecutionObservation::Unavailable, 0, None).unwrap(),
            ),
            Some((AssessmentResult::Blocked, Applicability::Uncertain)),
        ),
        (execution(0, 1, Some(OracleConclusion::Satisfied)), None),
    ] {
        let mut f = Fixture::new(0);
        let expected = result.map(|r| r.0);
        f.records[0] = record(&f.cases[0], kind, result);
        let report = run(&mut f);
        assert!(!complete(&report));
        assert_eq!(first(&report).result(), expected);
        assert!(
            first(&report)
                .reasons()
                .contains(&QualificationIssue::NotCurrentPass)
        );
    }
}
#[test]
fn mi10_zero_checks_missing_or_insufficient_oracle_bare_success() {
    for (checks, oracle) in [
        (0, None),
        (1, None),
        (1, Some(OracleConclusion::Insufficient)),
    ] {
        let mut f = Fixture::new(0);
        f.records[0] = record(&f.cases[0], execution(0, checks, oracle), None);
        let r = run(&mut f);
        assert!(!complete(&r));
        assert_eq!(first(&r).result(), None);
        assert_eq!(first(&r).validity(), Some(Validity::Current));
    }
}
#[test]
fn mi12_missing_one_neighbor_preserves_other_case_support() {
    let mut f = Fixture::new(0);
    f.records.remove(1);
    let r = run(&mut f);
    assert!(!complete(&r));
    assert!(r.claims()[0].cases()[0].requirements_met());
    assert_eq!(
        r.claims()[0].cases()[1].witnesses()[0].reasons(),
        [QualificationIssue::MissingRecord]
    );
}
#[test]
fn mi13_mi31_contradictory_extra_witness_cannot_hide_behind_positive() {
    let mut f = Fixture::new(0);
    let mut second = f.cases[0].clone();
    second.id = ObservationId::new("contradictory-report", 1).unwrap();
    f.records.push(record(
        &second,
        execution(0, 1, Some(OracleConclusion::Violated)),
        Some((AssessmentResult::Fail, Applicability::Applicable)),
    ));
    let mut w = f.witnesses();
    w.push(CaseWitness::new(
        second.claim.clone(),
        second.case.clone(),
        EvidenceReference::new(
            second.id.clone(),
            Some(AssessmentId::new("case-assessment").unwrap()),
        ),
    ));
    f.cases.push(second);
    let e = examinations(&f.cases);
    let q = QualificationRequest::new(&f.contract, f.contract.scope(), &w, &e, None);
    let r = inspect_method(&f.set, &f.method, Some(&q), &mut f.records, limits()).unwrap();
    assert!(!complete(&r));
    let case = &r.claims()[0].cases()[0];
    assert_eq!(case.witnesses().len(), 2);
    assert!(case.witnesses()[0].requirements_met());
    assert!(case.contradicted());
    assert_eq!(case.witnesses()[1].result(), Some(AssessmentResult::Fail));
}
#[test]
fn mi15_profile_change_retains_staleness_and_mi29_restoration_cannot_revive() {
    let mut f = Fixture::new(0);
    let original = f.cases[0].profile.clone();
    let changed = VerificationProfile::new(
        original.provenance().clone(),
        original.version(),
        "changed relevant environment",
        original
            .components()
            .iter()
            .map(|(a, b)| (a.as_str(), b.as_str()))
            .collect::<Vec<_>>()
            .as_slice(),
    )
    .unwrap();
    let before = f.records[0].clone();
    f.cases[0].profile = changed;
    {
        let r = run(&mut f);
        assert!(!complete(&r));
        assert_eq!(first(&r).validity(), Some(Validity::Stale));
        assert_eq!(
            first(&r).validity_issue(),
            Some(&ValidityIssue::ProfileChanged)
        );
    }
    assert_eq!(f.records[0].observation(), before.observation());
    assert_eq!(f.records[0].assessment(), before.assessment());
    f.cases[0].profile = original;
    assert_eq!(first(&run(&mut f)).validity(), Some(Validity::Stale));
}
#[test]
fn mi16_mi28_candidate_change_invalidates_only_affected_case() {
    let mut f = Fixture::new(0);
    let before = f.records.clone();
    f.cases[0].current = capture(&f.cases[0].binding, 3, true);
    let r = run(&mut f);
    assert!(!complete(&r));
    assert_eq!(first(&r).validity(), Some(Validity::Stale));
    assert!(r.claims()[0].cases()[1].requirements_met());
    assert_eq!(&r.records()[1..], &before[1..]);
    assert_eq!(r.records()[0].observation(), before[0].observation());
    assert_eq!(r.records()[0].assessment(), before[0].assessment());
    assert_ne!(r.records()[0], before[0]);
}
#[test]
fn mi15_basis_rejects_scope_correspondence_and_definition_revision_changes() {
    let mut f = Fixture::new(0);
    let original = f.contract.clone();
    let method = original.definition();
    let changed = QualificationContract::new(
        method,
        original.scope().clone(),
        ContractRevision::new(2).unwrap(),
        original.mappings().to_vec(),
        original.basis().clone(),
        limits(),
    )
    .unwrap();
    let r = run(&mut f);
    assert!(
        r.check_basis(method, Some(&original), Some(original.scope()))
            .is_ok()
    );
    assert_eq!(
        r.check_basis(method, Some(&changed), Some(changed.scope())),
        Err(MethodInspectionError::ContextChanged)
    );
    let newer = MethodDefinition::new(
        MethodReference::new(
            method.reference().id().clone(),
            ContractRevision::new(2).unwrap(),
        ),
        method.property().clone(),
        method.mechanism().clone(),
        method.claims().to_vec(),
        method.sources().to_vec(),
        method.context().clone(),
    )
    .unwrap();
    assert_eq!(
        r.check_basis(&newer, Some(&original), Some(original.scope())),
        Err(MethodInspectionError::ContextChanged)
    );
}
#[test]
fn mi27_duplicate_report_and_capture_identity_are_atomic_errors() {
    let mut f = Fixture::new(0);
    f.records.push(f.records[0].clone());
    let before = f.records.clone();
    let w = f.witnesses();
    let e = examinations(&f.cases);
    let q = QualificationRequest::new(&f.contract, f.contract.scope(), &w, &e, None);
    assert_eq!(
        inspect_method(&f.set, &f.method, Some(&q), &mut f.records, limits()).unwrap_err(),
        MethodInspectionError::IdentityConflict
    );
    assert_eq!(f.records, before);
}
#[test]
fn mi34_conflicting_late_examination_preserves_entire_pool() {
    let mut f = Fixture::new(0);
    let before = f.records.clone();
    let w = f.witnesses();
    let mut e = examinations(&f.cases);
    e.push(EvidenceExamination::new(
        f.cases[0].id.clone(),
        &f.cases[0].binding,
        None,
        Some(&f.cases[0].profile),
    ));
    let q = QualificationRequest::new(&f.contract, f.contract.scope(), &w, &e, None);
    assert_eq!(
        inspect_method(&f.set, &f.method, Some(&q), &mut f.records, limits()).unwrap_err(),
        MethodInspectionError::ConflictingExamination
    );
    assert_eq!(f.records, before);
}
#[test]
fn mi31_reusing_report_for_sibling_case_is_refused() {
    let mut f = Fixture::new(0);
    let before = f.records.clone();
    let mut w = f.witnesses();
    w[1] = CaseWitness::new(
        f.cases[1].claim.clone(),
        f.cases[1].case.clone(),
        w[0].evidence().clone(),
    );
    let e = examinations(&f.cases);
    let q = QualificationRequest::new(&f.contract, f.contract.scope(), &w, &e, None);
    assert_eq!(
        inspect_method(&f.set, &f.method, Some(&q), &mut f.records, limits()).unwrap_err(),
        MethodInspectionError::IdentityConflict
    );
    assert_eq!(f.records, before);
}
#[test]
fn mi33_missing_assessment_reference_does_not_get_filled() {
    let mut f = Fixture::new(0);
    let mut w = f.witnesses();
    w[0] = CaseWitness::new(
        f.cases[0].claim.clone(),
        f.cases[0].case.clone(),
        EvidenceReference::new(f.cases[0].id.clone(), None),
    );
    let e = examinations(&f.cases);
    let q = QualificationRequest::new(&f.contract, f.contract.scope(), &w, &e, None);
    let r = inspect_method(&f.set, &f.method, Some(&q), &mut f.records, limits()).unwrap();
    assert!(!complete(&r));
    assert_eq!(first(&r).reference().assessment(), None);
    assert_eq!(first(&r).result(), Some(AssessmentResult::Pass));
    assert!(
        first(&r)
            .reasons()
            .contains(&QualificationIssue::MissingAssessmentReference)
    );
}
#[test]
fn mi06_missing_current_capture_preserves_unestablished_validity() {
    let mut f = Fixture::new(0);
    let w = f.witnesses();
    let e = vec![EvidenceExamination::new(
        f.cases[0].id.clone(),
        &f.cases[0].binding,
        None,
        Some(&f.cases[0].profile),
    )];
    let q = QualificationRequest::new(&f.contract, f.contract.scope(), &w, &e, None);
    let r = inspect_method(&f.set, &f.method, Some(&q), &mut f.records, limits()).unwrap();
    assert_eq!(first(&r).validity(), None);
    assert!(!complete(&r));
    assert_eq!(r.applicability(), Applicability::Uncertain);
}
#[test]
fn mi32_duplicate_assignments_are_idempotent_and_charged() {
    let mut f = Fixture::new(0);
    let before = run(&mut f).claims().to_vec();
    let original = f.records.clone();
    let mut w = f.witnesses();
    w.push(w[0].clone());
    let e = examinations(&f.cases);
    let q = QualificationRequest::new(&f.contract, f.contract.scope(), &w, &e, None);
    let r = inspect_method(&f.set, &f.method, Some(&q), &mut f.records, limits()).unwrap();
    assert_eq!(r.claims(), before);
    assert_eq!(r.input_usage().get(InspectionLimitKind::Assignments), 5);
    assert_eq!(r.records(), original);
}

fn replace_definition(
    f: &mut Fixture,
    claims: Vec<MethodClaim>,
    sources: Vec<SourceEntry>,
    context: MethodContext,
) {
    let old = f.set.definition(&f.method).unwrap();
    let d = MethodDefinition::new(
        old.reference().clone(),
        old.property().clone(),
        old.mechanism().clone(),
        claims,
        sources,
        context,
    )
    .unwrap();
    f.contract = QualificationContract::new(
        &d,
        d.context().target().clone(),
        ContractRevision::new(1).unwrap(),
        f.contract.mappings().to_vec(),
        source("revised definition fixture"),
        limits(),
    )
    .unwrap();
    f.set = MethodSet::new(vec![d], definition_limits()).unwrap();
}
#[test]
fn mi18_mi19_required_sources_and_conflicts_are_not_qualification() {
    let mut f = Fixture::new(0);
    let d = f.set.definition(&f.method).unwrap().clone();
    let src = d.sources()[0].reference().clone();
    let cl = &d.claims()[0];
    let claim = MethodClaim::new(
        cl.id().clone(),
        cl.statement(),
        cl.exclusions(),
        cl.cases().to_vec(),
        vec![src.clone()],
        vec![src.clone()],
    )
    .unwrap();
    let unavailable = SourceEntry::new(
        src,
        SourceAvailability::Unavailable,
        SourceKind::Original,
        "missing-original",
        "summary cannot replace original",
    )
    .unwrap();
    replace_definition(&mut f, vec![claim], vec![unavailable], d.context().clone());
    let r = run(&mut f);
    assert!(!complete(&r));
    assert!(
        r.claims()[0]
            .cases()
            .iter()
            .all(CaseInspection::requirements_met)
    );
    assert_eq!(r.source_gaps().count(), 1);
    assert!(
        r.claims()[0]
            .issues()
            .contains(&QualificationIssue::RequiredSourceUnavailable)
    );
    assert!(
        r.claims()[0]
            .issues()
            .contains(&QualificationIssue::SourceDisagreement)
    );
}
#[test]
fn mi19_optional_unavailable_background_does_not_erase_scoped_support() {
    let mut f = Fixture::new(0);
    let d = f.set.definition(&f.method).unwrap().clone();
    let mut sources = d.sources().to_vec();
    sources.push(
        SourceEntry::new(
            source("optional-background"),
            SourceAvailability::SummaryOnly,
            SourceKind::Synthesis,
            "excerpt",
            "unassessed optional claim",
        )
        .unwrap(),
    );
    replace_definition(&mut f, d.claims().to_vec(), sources, d.context().clone());
    let r = run(&mut f);
    assert!(complete(&r));
    assert_eq!(r.source_gaps().count(), 1);
}
#[test]
fn mi22_inherited_requirements_and_exceptions_cannot_cancel_each_other() {
    let mut f = Fixture::new(0);
    let d = f.set.definition(&f.method).unwrap().clone();
    let ctx = d.context();
    let context = MethodContext::new(
        ctx.target().clone(),
        ctx.applicability().clone(),
        vec![source("protected-inherited-rule")],
        vec![source("unapproved-exception")],
        source("unresolved-adoption-authority"),
        ctx.dependencies().to_vec(),
    );
    replace_definition(&mut f, d.claims().to_vec(), d.sources().to_vec(), context);
    let r = run(&mut f);
    assert_eq!(r.requirements()[0].source(), "protected-inherited-rule");
    assert_eq!(r.exceptions()[0].source(), "unapproved-exception");
    assert!(r.is_advisory());
    assert!(complete(&r));
}
#[test]
fn mi23_mixed_language_target_cannot_use_native_fixture_scope_as_end_to_end_support() {
    let mut f = Fixture::new(0);
    let scope = TargetScope::new(
        TaskId::new("rust-js-id-boundary").unwrap(),
        ContractRevision::new(1).unwrap(),
        ScopeKind::Interface,
        vec![
            ComponentScope::new(
                TaskId::new("producer").unwrap(),
                Some("Rust"),
                None,
                None,
                vec![],
            )
            .unwrap(),
            ComponentScope::new(
                TaskId::new("consumer").unwrap(),
                Some("JavaScript"),
                None,
                None,
                vec![],
            )
            .unwrap(),
        ],
        None,
        Some("9007199254740993 identifier preservation"),
        None,
    )
    .unwrap();
    let d = f.set.definition(&f.method).unwrap();
    f.contract = QualificationContract::new(
        d,
        scope,
        ContractRevision::new(1).unwrap(),
        f.contract.mappings().to_vec(),
        source("unsupported interface qualification"),
        limits(),
    )
    .unwrap();
    let r = run(&mut f);
    assert!(!complete(&r));
    assert_eq!(r.applicability(), Applicability::Uncertain);
    assert!(
        r.claims()[0]
            .issues()
            .contains(&QualificationIssue::ScopeMismatch)
    );
}

fn applicability_fixture(
    f: &Fixture,
    kind: ObservationKind,
    result: (AssessmentResult, Applicability),
) -> (CaseData, EvidenceRecord, ApplicabilityCorrespondence) {
    let d = f.set.definition(&f.method).unwrap();
    let b = binding("target-trigger", d.context().applicability().trigger());
    let p = f.cases[0].profile.clone();
    let data = CaseData {
        baseline: 1,
        claim: ClaimId::new("trigger").unwrap(),
        case: CaseId::new("trigger").unwrap(),
        current: capture(&b, 2, false),
        profile: p.clone(),
        binding: b.clone(),
        id: ObservationId::new("target-trigger-report", 1).unwrap(),
    };
    let r = record(&data, kind, Some(result));
    let a = ApplicabilityCorrespondence::new(
        f.contract.scope().clone(),
        b,
        p,
        EvidenceReference::new(
            data.id.clone(),
            Some(AssessmentId::new("case-assessment").unwrap()),
        ),
        CaseCoverage::ExactCriterion,
    );
    (data, r, a)
}
#[test]
fn mi05_mi07_target_applicability_needs_its_own_current_assessed_basis() {
    for (kind, result, expected) in [
        (
            execution(0, 1, Some(OracleConclusion::Satisfied)),
            (AssessmentResult::Pass, Applicability::Applicable),
            Applicability::Applicable,
        ),
        (
            ObservationKind::ScopeInspection(ScopeFinding::Absent),
            (
                AssessmentResult::NotApplicable,
                Applicability::NotApplicable,
            ),
            Applicability::NotApplicable,
        ),
        (
            ObservationKind::Execution(
                VerificationOutcome::new(ExecutionObservation::Unavailable, 0, None).unwrap(),
            ),
            (AssessmentResult::Blocked, Applicability::Uncertain),
            Applicability::Uncertain,
        ),
    ] {
        let mut f = Fixture::new(0);
        let (data, record, app) = applicability_fixture(&f, kind, result);
        f.records.push(record);
        let w = f.witnesses();
        let mut e = examinations(&f.cases);
        e.push(EvidenceExamination::new(
            data.id.clone(),
            &data.binding,
            Some(&data.current),
            Some(&data.profile),
        ));
        let q = QualificationRequest::new(&f.contract, f.contract.scope(), &w, &e, Some(&app));
        let r = inspect_method(&f.set, &f.method, Some(&q), &mut f.records, limits()).unwrap();
        assert_eq!(r.applicability(), expected);
        assert_eq!(r.applicability_witness().unwrap().result(), Some(result.0));
        assert!(complete(&r));
    }
}
#[test]
fn mi15_wrong_definition_profile_and_unestablished_coverage_cannot_support() {
    let mut f = Fixture::new(0);
    let d = f.set.definition(&f.method).unwrap();
    let mappings = f
        .contract
        .mappings()
        .iter()
        .enumerate()
        .map(|(i, m)| {
            CaseCorrespondence::new(
                m.claim().clone(),
                m.case().clone(),
                m.verification().clone(),
                m.profile().clone(),
                m.oracle().clone(),
                if i == 0 {
                    CaseCoverage::Unestablished
                } else {
                    CaseCoverage::ExactCriterion
                },
            )
        })
        .collect();
    f.contract = QualificationContract::new(
        d,
        f.contract.scope().clone(),
        ContractRevision::new(2).unwrap(),
        mappings,
        source("incomplete-coverage"),
        limits(),
    )
    .unwrap();
    let r = run(&mut f);
    assert!(!complete(&r));
    assert_eq!(
        r.claims()[0].cases()[0].issues(),
        [QualificationIssue::CoverageUnestablished]
    );
}
#[test]
fn mi32_wrong_source_criterion_cannot_support_matching_named_case() {
    let mut f = Fixture::new(0);
    let mut different = f.cases[0].clone();
    different.binding = binding("unrelated-task", "An unrelated acceptance criterion");
    different.current = capture(&different.binding, 2, false);
    f.records[0] = record(
        &different,
        execution(0, 1, Some(OracleConclusion::Satisfied)),
        Some((AssessmentResult::Pass, Applicability::Applicable)),
    );
    f.cases[0] = different;
    let r = run(&mut f);
    assert!(!complete(&r));
    assert_eq!(first(&r).validity(), Some(Validity::Current));
    assert!(
        first(&r)
            .reasons()
            .contains(&QualificationIssue::CriterionMismatch)
    );
}
#[test]
fn mi25_all_inspection_limits_and_mi34_late_budget_refusal_are_atomic() {
    let mut f = Fixture::new(0);
    let usage = run(&mut f).input_usage();
    let kinds = [
        InspectionLimitKind::Records,
        InspectionLimitKind::Assignments,
        InspectionLimitKind::Cases,
        InspectionLimitKind::References,
        InspectionLimitKind::MaterialBytes,
        InspectionLimitKind::Findings,
    ];
    assert_eq!(usage.get(InspectionLimitKind::Records), 8);
    assert_eq!(usage.get(InspectionLimitKind::Assignments), 4);
    assert_eq!(usage.get(InspectionLimitKind::Cases), 12);
    assert_eq!(usage.get(InspectionLimitKind::Findings), 9);
    let exact = kinds.map(|k| usage.get(k));
    let w = f.witnesses();
    let e = examinations(&f.cases);
    let q = QualificationRequest::new(&f.contract, f.contract.scope(), &w, &e, None);
    let l =
        InspectionLimits::new(exact[0], exact[1], exact[2], exact[3], exact[4], exact[5]).unwrap();
    assert!(inspect_method(&f.set, &f.method, Some(&q), &mut f.records, l).is_ok());
    for (i, k) in kinds.into_iter().enumerate() {
        let mut v = exact;
        v[i] -= 1;
        let l = InspectionLimits::new(v[0], v[1], v[2], v[3], v[4], v[5]).unwrap();
        let before = f.records.clone();
        assert_eq!(
            inspect_method(&f.set, &f.method, Some(&q), &mut f.records, l).unwrap_err(),
            MethodInspectionError::Limit(k)
        );
        assert_eq!(f.records, before);
    }
    for i in 0..6 {
        let mut v = [1; 6];
        v[i] = 0;
        assert_eq!(
            InspectionLimits::new(v[0], v[1], v[2], v[3], v[4], v[5]),
            Err(MethodInspectionError::InvalidLimits)
        );
        v[i] = usize::MAX;
        assert_eq!(
            InspectionLimits::new(v[0], v[1], v[2], v[3], v[4], v[5]),
            Err(MethodInspectionError::InvalidLimits)
        );
    }
    // One claim, four cases and four witnesses fit exactly. This cap is above
    // the early claim-plus-case minimum (5), unlike an early-refusal cap of 1.
    let restricted = InspectionLimits::new(256, 256, 256, 4096, 8_000_000, 9).unwrap();
    let sufficient = InspectionLimits::new(256, 256, 256, 4096, 8_000_000, 10).unwrap();
    let before = f.records.clone();
    {
        let r = inspect_method(&f.set, &f.method, Some(&q), &mut f.records, restricted).unwrap();
        assert!(complete(&r));
        assert_eq!(r.claims().len(), 1);
        assert_eq!(r.claims()[0].cases().len(), 4);
        assert!(
            r.claims()[0]
                .cases()
                .iter()
                .all(|case| case.witnesses().len() == 1)
        );
        assert_eq!(r.input_usage().get(InspectionLimitKind::Findings), 9);
        assert_eq!(first(&r).validity(), Some(Validity::Current));
        assert_eq!(r.records(), before);
    }
    // Changing only the examination profile stages a sticky invalidation and
    // adds one NotCurrentPass reason. Final accounting (10) must refuse at 9
    // before any of those drafts are published to the original record pool.
    let changed = VerificationProfile::new(
        f.cases[0].profile.provenance().clone(),
        "changed",
        "changed",
        &[],
    )
    .unwrap();
    let mut e = examinations(&f.cases);
    e[0] = EvidenceExamination::new(
        f.cases[0].id.clone(),
        &f.cases[0].binding,
        Some(&f.cases[0].current),
        Some(&changed),
    );
    let q = QualificationRequest::new(&f.contract, f.contract.scope(), &w, &e, None);
    assert_eq!(
        inspect_method(&f.set, &f.method, Some(&q), &mut f.records, restricted).unwrap_err(),
        MethodInspectionError::Limit(InspectionLimitKind::Findings)
    );
    assert_eq!(f.records, before);

    // The identical changed request succeeds at the exact next cap, exposing
    // both the late count and the staged state the refused call could not publish.
    let first_invalidation;
    {
        let r = inspect_method(&f.set, &f.method, Some(&q), &mut f.records, sufficient).unwrap();
        assert_eq!(r.input_usage().get(InspectionLimitKind::Findings), 10);
        assert!(!complete(&r));
        assert_eq!(first(&r).validity(), Some(Validity::Stale));
        assert_eq!(
            first(&r).validity_issue(),
            Some(&ValidityIssue::ProfileChanged)
        );
        assert_eq!(first(&r).reasons(), [QualificationIssue::NotCurrentPass]);
        assert!(
            r.claims()[0].cases()[1..]
                .iter()
                .all(CaseInspection::requirements_met)
        );
        first_invalidation = (
            first(&r).validity_issue().cloned(),
            first(&r).compared_to().cloned(),
        );
        for (record, original) in r.records().iter().zip(&before) {
            assert_eq!(record.observation(), original.observation());
            assert_eq!(record.assessment(), original.assessment());
        }
        assert_eq!(&r.records()[1..], &before[1..]);
        assert_ne!(r.records()[0], before[0]);
    }
    let invalidated = f.records.clone();
    {
        let restored = run(&mut f);
        assert!(!complete(&restored));
        assert_eq!(first(&restored).validity(), Some(Validity::Stale));
        assert_eq!(
            (
                first(&restored).validity_issue().cloned(),
                first(&restored).compared_to().cloned(),
            ),
            first_invalidation
        );
    }
    assert_eq!(f.records, invalidated);
}
#[test]
fn mi25_debug_and_error_chains_do_not_repeat_private_payloads() {
    let mut f = Fixture::new(0);
    let marker = "private-report-marker";
    let mut data = f.cases[0].clone();
    data.id = ObservationId::new(marker, 1).unwrap();
    f.records[0] = record(
        &data,
        execution(0, 1, Some(OracleConclusion::Satisfied)),
        Some((AssessmentResult::Pass, Applicability::Applicable)),
    );
    f.cases[0] = data;
    let r = run(&mut f);
    let debug = format!("{r:?} {:?} {:?} {:?}", r.basis(), r.claims(), first(&r));
    assert!(!debug.contains(marker));
    assert_eq!(first(&r).reference().observation().scope(), marker);
    let error = MethodInspectionError::ContextChanged;
    assert!(!format!("{error} {error:?}").contains(marker));
}
#[test]
fn mi12_two_claims_preserve_support_without_whole_method_success() {
    let f0 = Fixture::new(0);
    let d = f0.set.definition(&f0.method).unwrap();
    let c = &d.claims()[0];
    let extra = MethodClaim::new(
        ClaimId::new("unsupported-claim").unwrap(),
        "A separate unsupported claim",
        "No source/context coverage",
        vec![],
        vec![],
        vec![],
    )
    .unwrap();
    let def = MethodDefinition::new(
        d.reference().clone(),
        d.property().clone(),
        d.mechanism().clone(),
        vec![c.clone(), extra],
        d.sources().to_vec(),
        d.context().clone(),
    )
    .unwrap();
    let mut f = Fixture::from_definition(def);
    let r = run(&mut f);
    assert!(r.claims()[0].requirements_met());
    assert!(!r.claims()[1].requirements_met());
    assert_eq!(
        r.claims()[1].issues(),
        [
            QualificationIssue::MissingViolation,
            QualificationIssue::MissingNeighbor
        ]
    );
}

// Bounded test-only child protocol. The product inspector never creates a process.
#[test]
#[ignore = "test-only protocol selected explicitly by the starter qualification parent"]
fn native_fixture_child() {
    let case = std::env::var("CORTEX_METHOD_CASE").expect("parent supplies a fixture case");
    let method = std::env::var("CORTEX_METHOD_KIND").unwrap();
    let faulty = std::env::var_os("CORTEX_METHOD_FAULTY").is_some();
    let c = binding(
        "native-child-fixture",
        "independent parent evaluates the emitted mechanism observation",
    );
    let actual = if method == "binding" {
        let base = capture(&c, 1, false);
        let current = capture(&c, 2, false);
        match case.as_str() {
            "mismatch" => {
                let wrong = CandidateObservation::new(
                    ObservationId::new(c.task().id().as_str(), 2).unwrap(),
                    &c,
                    RepositoryState::new(
                        Some(repository()),
                        Head::Detached {
                            commit: commit("b"),
                        },
                        vec![],
                        ObservationCoverage::GitStatus,
                        ObservationConsistency::TwoMatchingSamples,
                    )
                    .unwrap(),
                    current.basis().cloned(),
                );
                if bind_candidate(&c, Some(&base), Some(&wrong)).unwrap()
                    == BindingDecision::Incompatible(BindingMismatch::Commit)
                {
                    "commit-mismatch"
                } else {
                    "wrong-classification"
                }
            }
            "detached" => {
                if matches!(
                    bind_candidate(&c, Some(&base), Some(&current)).unwrap(),
                    BindingDecision::Compatible(_)
                ) {
                    if faulty {
                        "rejected-neighbor"
                    } else {
                        "detached-compatible"
                    }
                } else {
                    "wrong-classification"
                }
            }
            "dirty" => {
                let allow = CandidateBindingContract::new(
                    c.task().clone(),
                    c.revision(),
                    CandidateRequirement::new(
                        repository(),
                        commit("a"),
                        WorktreeRequirement::AllowChanges,
                    ),
                );
                if bind_candidate(
                    &allow,
                    Some(&capture(&allow, 1, true)),
                    Some(&capture(&allow, 2, true)),
                )
                .unwrap()
                    == BindingDecision::Insufficient(BindingInsufficiency::UncommittedIdentity)
                {
                    "dirty-unresolved"
                } else {
                    "wrong-classification"
                }
            }
            "matching" => {
                if matches!(
                    bind_candidate(&c, Some(&base), Some(&current)).unwrap(),
                    BindingDecision::Compatible(_)
                ) {
                    "matching-compatible"
                } else {
                    "wrong-classification"
                }
            }
            _ => panic!("unknown fixture case"),
        }
    } else {
        let p = VerificationProfile::new(
            Provenance::new(
                "child native Evidence fixture",
                "synthetic records",
                "no effect authority",
            )
            .unwrap(),
            "1",
            "fixture",
            &[],
        )
        .unwrap();
        let data = CaseData {
            baseline: 1,
            claim: ClaimId::new("claim").unwrap(),
            case: CaseId::new("case").unwrap(),
            current: capture(&c, 2, false),
            profile: p.clone(),
            binding: c.clone(),
            id: ObservationId::new("child-evidence", 1).unwrap(),
        };
        match case.as_str() {
            "zero-checks" => {
                let mut r = record(&data, execution(0, 0, None), None);
                if r.assess(
                    AssessmentId::new("attempt").unwrap(),
                    time(12),
                    AssessmentResult::Pass,
                    Applicability::Applicable,
                    "must not pass without relevant checks",
                    &ValidationContext::new(&c, Some(&data.current), Some(&p)),
                ) == Err(SemanticError::MissingOracle)
                {
                    "zero-checks-refused"
                } else {
                    "wrong-classification"
                }
            }
            "legitimate-fail" => {
                let mut r = record(
                    &data,
                    execution(0, 1, Some(OracleConclusion::Violated)),
                    Some((AssessmentResult::Fail, Applicability::Applicable)),
                );
                let v = r.inspect(&ValidationContext::new(&c, Some(&data.current), Some(&p)));
                if v.result() == Some(AssessmentResult::Fail)
                    && v.validity() == Some(Validity::Current)
                {
                    "legitimate-fail"
                } else {
                    "wrong-classification"
                }
            }
            "legitimate-unknown" => {
                let mut r = record(
                    &data,
                    execution(0, 0, None),
                    Some((AssessmentResult::Unknown, Applicability::Applicable)),
                );
                if r.inspect(&ValidationContext::new(&c, Some(&data.current), Some(&p)))
                    .result()
                    == Some(AssessmentResult::Unknown)
                {
                    "legitimate-unknown"
                } else {
                    "wrong-classification"
                }
            }
            "stale" => {
                let mut r = record(
                    &data,
                    execution(0, 1, Some(OracleConclusion::Satisfied)),
                    Some((AssessmentResult::Pass, Applicability::Applicable)),
                );
                let changed =
                    VerificationProfile::new(p.provenance().clone(), "2", "changed", &[]).unwrap();
                assert_eq!(
                    r.inspect(&ValidationContext::new(
                        &c,
                        Some(&data.current),
                        Some(&changed)
                    ))
                    .validity(),
                    Some(Validity::Stale)
                );
                if r.inspect(&ValidationContext::new(&c, Some(&data.current), Some(&p)))
                    .validity()
                    == Some(Validity::Stale)
                {
                    "sticky-stale"
                } else {
                    "wrong-classification"
                }
            }
            _ => panic!("unknown fixture case"),
        }
    };
    println!("METHOD-OBSERVED:{actual}");
}
fn actual_outcome(method: &str, case: &str, faulty: bool) -> (u32, OracleConclusion) {
    use std::{
        io::Read,
        process::{Command, Stdio},
        time::Instant,
    };
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            "native_fixture_child",
            "--ignored",
            "--nocapture",
        ])
        .env("CORTEX_METHOD_KIND", method)
        .env("CORTEX_METHOD_CASE", case)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if faulty {
        command.env("CORTEX_METHOD_FAULTY", "1");
    } else {
        command.env_remove("CORTEX_METHOD_FAULTY");
    }
    let mut child = command.spawn().unwrap();
    let start = Instant::now();
    let status = loop {
        if let Some(s) = child.try_wait().unwrap() {
            break s;
        }
        if start.elapsed() > Duration::from_secs(5) {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("qualification child exceeded five-second fixture limit");
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let mut stdout = String::new();
    child
        .stdout
        .take()
        .unwrap()
        .take(16_384)
        .read_to_string(&mut stdout)
        .unwrap();
    let mut stderr = String::new();
    child
        .stderr
        .take()
        .unwrap()
        .take(16_384)
        .read_to_string(&mut stderr)
        .unwrap();
    assert!(
        status.success(),
        "fixture must execute successfully: {stdout} {stderr}"
    );
    assert!(stdout.contains("test result: ok. 1 passed"));
    let observations: Vec<_> = stdout
        .lines()
        .filter_map(|s| s.strip_prefix("METHOD-OBSERVED:"))
        .collect();
    assert_eq!(observations.len(), 1);
    // Literal independent oracle, not calculated using the inspector or mechanism.
    let expected = match case {
        "mismatch" => "commit-mismatch",
        "detached" => "detached-compatible",
        "dirty" => "dirty-unresolved",
        "matching" => "matching-compatible",
        "zero-checks" => "zero-checks-refused",
        "legitimate-fail" => "legitimate-fail",
        "legitimate-unknown" => "legitimate-unknown",
        "stale" => "sticky-stale",
        _ => panic!("undeclared parent case"),
    };
    let oracle = if observations[0] == expected {
        OracleConclusion::Satisfied
    } else {
        OracleConclusion::Violated
    };
    println!(
        "qualification execution: {method}/{case}; exit={}; oracle={oracle:?}; faulty-wrapper={faulty}",
        status.code().unwrap()
    );
    (status.code().unwrap().try_into().unwrap(), oracle)
}
fn actual_record(data: &CaseData, code: u32, oracle: OracleConclusion) -> EvidenceRecord {
    let observed = SystemTime::now();
    let mut r=EvidenceRecord::new(ReportedObservation::new(data.id.clone(),capture(&data.binding,data.baseline,false),data.profile.clone(),ObservationTimes::new(observed,SystemTime::now()).unwrap(),execution(code,1,Some(oracle)),"Actual test-only native case child execution; candidate captures remain synthetic domain inputs").unwrap());
    r.assess(AssessmentId::new("case-assessment").unwrap(),SystemTime::now(),if oracle==OracleConclusion::Satisfied{AssessmentResult::Pass}else{AssessmentResult::Fail},Applicability::Applicable,"Parent compares an actually emitted native mechanism observation with a literal expected value",&ValidationContext::new(&data.binding,Some(&data.current),Some(&data.profile))).unwrap();
    r
}
#[test]
fn mi11_actual_starter_cases_are_executed_and_inspected() {
    for index in 0..2 {
        let mut f = Fixture::new(index);
        let kind = if index == 0 { "binding" } else { "evidence" };
        for i in 0..f.cases.len() {
            let (code, oracle) = actual_outcome(kind, f.cases[i].case.as_str(), false);
            assert_eq!(oracle, OracleConclusion::Satisfied);
            f.records[i] = actual_record(&f.cases[i], code, oracle);
        }
        let r = run(&mut f);
        assert!(complete(&r));
        assert!(
            r.claims()[0]
                .cases()
                .iter()
                .all(|c| c.witnesses()[0].result() == Some(AssessmentResult::Pass))
        );
    }
}
#[test]
fn mi14_actual_overbroad_wrapper_fails_its_legitimate_neighbor() {
    let native = Fixture::new(0);
    let d = native.set.definition(&native.method).unwrap();
    let wrapper = source("test-only-overbroad-wrapper");
    let cl = &d.claims()[0];
    let claim = MethodClaim::new(
        cl.id().clone(),
        cl.statement(),
        cl.exclusions(),
        cl.cases().to_vec(),
        vec![wrapper.clone()],
        vec![],
    )
    .unwrap();
    let mutant = MethodDefinition::new(
        MethodReference::new(
            MethodId::new("test.overbroad-wrapper").unwrap(),
            ContractRevision::new(1).unwrap(),
        ),
        d.property().clone(),
        MethodMechanism::new(
            wrapper.clone(),
            "Deliberately reject detached HEAD after calling the native mechanism",
            "Actual child observations",
            "Test-only negative control, not the native mechanism",
        )
        .unwrap(),
        vec![claim],
        {
            let mut sources = d.sources().to_vec();
            sources.push(
                SourceEntry::new(
                    wrapper,
                    SourceAvailability::Inspected,
                    SourceKind::Original,
                    "test fixture",
                    "controlled false-positive negative control",
                )
                .unwrap(),
            );
            sources
        },
        d.context().clone(),
    )
    .unwrap();
    let mut f = Fixture::from_definition(mutant);
    for i in 0..f.cases.len() {
        let (code, oracle) = actual_outcome("binding", f.cases[i].case.as_str(), true);
        f.records[i] = actual_record(&f.cases[i], code, oracle);
    }
    let r = run(&mut f);
    assert!(!complete(&r));
    assert!(r.claims()[0].cases()[0].requirements_met());
    assert!(r.claims()[0].cases()[1].contradicted());
    assert_eq!(
        r.claims()[0].cases()[1].witnesses()[0].result(),
        Some(AssessmentResult::Fail)
    );
}
#[test]
fn mi30_actual_revalidation_uses_new_execution_observation_and_assessment() {
    let mut f = Fixture::new(0);
    let (code, oracle) = actual_outcome("binding", "mismatch", false);
    f.records[0] = actual_record(&f.cases[0], code, oracle);
    f.records[0]
        .invalidate("known affected dependency change")
        .unwrap();
    assert_eq!(first(&run(&mut f)).validity(), Some(Validity::Stale));
    let old = f.records[0].clone();
    f.cases[0].baseline = 5;
    f.cases[0].current = capture(&f.cases[0].binding, 6, false);
    f.cases[0].id = ObservationId::new("fresh-verification", 2).unwrap();
    let (code, oracle) = actual_outcome("binding", "mismatch", false);
    f.records[0] = actual_record(&f.cases[0], code, oracle);
    let r = run(&mut f);
    assert!(complete(&r));
    assert_ne!(r.records()[0].observation().id(), old.observation().id());
    assert_ne!(
        r.records()[0]
            .assessment()
            .unwrap()
            .candidate_at_assessment()
            .unwrap()
            .id(),
        old.assessment()
            .unwrap()
            .candidate_at_assessment()
            .unwrap()
            .id()
    );
    assert_eq!(
        old.invalidation_reason(),
        Some("known affected dependency change")
    );
}
#[test]
fn mi15_wrong_source_profile_is_visible_even_when_current_in_its_own_context() {
    let mut f = Fixture::new(0);
    let p = VerificationProfile::new(
        f.cases[0].profile.provenance().clone(),
        "wrong-mechanism-version",
        "different-profile",
        &[],
    )
    .unwrap();
    f.cases[0].profile = p;
    f.records[0] = record(
        &f.cases[0],
        execution(0, 1, Some(OracleConclusion::Satisfied)),
        Some((AssessmentResult::Pass, Applicability::Applicable)),
    );
    let r = run(&mut f);
    assert!(!complete(&r));
    assert_eq!(first(&r).validity(), Some(Validity::Current));
    assert!(
        first(&r)
            .reasons()
            .contains(&QualificationIssue::ProfileMismatch)
    );
}
#[test]
fn mi15_mapped_mechanism_profile_cannot_omit_oracle_and_fixture_identity() {
    let mut f = Fixture::new(0);
    let d = f.set.definition(&f.method).unwrap();
    let old = &f.cases[0].profile;
    let inadequate = VerificationProfile::new(
        old.provenance().clone(),
        old.version(),
        old.environment(),
        &[],
    )
    .unwrap();
    let mappings = f
        .contract
        .mappings()
        .iter()
        .enumerate()
        .map(|(i, m)| {
            CaseCorrespondence::new(
                m.claim().clone(),
                m.case().clone(),
                m.verification().clone(),
                if i == 0 {
                    inadequate.clone()
                } else {
                    m.profile().clone()
                },
                m.oracle().clone(),
                m.coverage(),
            )
        })
        .collect();
    f.contract = QualificationContract::new(
        d,
        f.contract.scope().clone(),
        ContractRevision::new(2).unwrap(),
        mappings,
        source("inadequate-profile"),
        limits(),
    )
    .unwrap();
    f.cases[0].profile = inadequate;
    f.records[0] = record(
        &f.cases[0],
        execution(0, 1, Some(OracleConclusion::Satisfied)),
        Some((AssessmentResult::Pass, Applicability::Applicable)),
    );
    let r = run(&mut f);
    assert!(!complete(&r));
    assert!(
        r.claims()[0].cases()[0]
            .issues()
            .contains(&QualificationIssue::MechanismProfileMismatch)
    );
    assert!(!first(&r).requirements_met());
    assert_eq!(first(&r).validity(), Some(Validity::Current));
}
#[test]
fn mi27_same_task_revision_alias_and_capture_alias_refuse_without_mutation() {
    let mut f = Fixture::new(0);
    let d = &f.cases[0];
    let changed = CandidateBindingContract::new(
        TaskContract::new(
            d.binding.task().id().clone(),
            "same identity different contents",
            d.binding.task().acceptance(),
        )
        .unwrap(),
        d.binding.revision(),
        d.binding.candidate().clone(),
    );
    let w = f.witnesses();
    let mut e = examinations(&f.cases);
    e[0] = EvidenceExamination::new(d.id.clone(), &changed, None, Some(&d.profile));
    let before = f.records.clone();
    let q = QualificationRequest::new(&f.contract, f.contract.scope(), &w, &e, None);
    assert_eq!(
        inspect_method(&f.set, &f.method, Some(&q), &mut f.records, limits()).unwrap_err(),
        MethodInspectionError::IdentityConflict
    );
    assert_eq!(f.records, before);
    let conflicting = capture(&f.cases[0].binding, 2, true);
    let mut e = examinations(&f.cases);
    e[0] = EvidenceExamination::new(
        f.cases[0].id.clone(),
        &f.cases[0].binding,
        Some(&conflicting),
        Some(&f.cases[0].profile),
    );
    let q = QualificationRequest::new(&f.contract, f.contract.scope(), &w, &e, None);
    assert_eq!(
        inspect_method(&f.set, &f.method, Some(&q), &mut f.records, limits()).unwrap_err(),
        MethodInspectionError::IdentityConflict
    );
    assert_eq!(f.records, before);
}
#[test]
fn mi15_well_formed_new_contract_revision_retains_legitimate_staleness() {
    let mut f = Fixture::new(0);
    let old = &f.cases[0].binding;
    let newer = CandidateBindingContract::new(
        old.task().clone(),
        ContractRevision::new(2).unwrap(),
        old.candidate().clone(),
    );
    let current = capture(&newer, 3, false);
    let w = f.witnesses();
    let mut e = examinations(&f.cases);
    e[0] = EvidenceExamination::new(
        f.cases[0].id.clone(),
        &newer,
        Some(&current),
        Some(&f.cases[0].profile),
    );
    let q = QualificationRequest::new(&f.contract, f.contract.scope(), &w, &e, None);
    let r = inspect_method(&f.set, &f.method, Some(&q), &mut f.records, limits()).unwrap();
    assert!(!complete(&r));
    assert_eq!(first(&r).validity(), Some(Validity::Stale));
    assert_eq!(
        first(&r).validity_issue(),
        Some(&ValidityIssue::CandidateStale(
            BindingStaleness::ContractRevision
        ))
    );
}
#[test]
fn mi32_full_scope_and_mapping_integrity_precede_any_state_change() {
    let f = Fixture::new(0);
    let d = f.set.definition(&f.method).unwrap();
    let original = f.contract.mappings()[0].clone();
    let wrong = binding(
        "different-criterion",
        "Not the declared qualification criterion",
    );
    let mut mappings = f.contract.mappings().to_vec();
    mappings[0] = CaseCorrespondence::new(
        original.claim().clone(),
        original.case().clone(),
        wrong,
        original.profile().clone(),
        original.oracle().clone(),
        CaseCoverage::ExactCriterion,
    );
    assert_eq!(
        QualificationContract::new(
            d,
            f.contract.scope().clone(),
            ContractRevision::new(1).unwrap(),
            mappings,
            source("wrong-criterion"),
            limits()
        ),
        Err(MethodInspectionError::InvalidCorrespondence)
    );
    let mut mappings = f.contract.mappings().to_vec();
    mappings.pop();
    assert_eq!(
        QualificationContract::new(
            d,
            f.contract.scope().clone(),
            ContractRevision::new(1).unwrap(),
            mappings,
            source("omitted-case"),
            limits()
        ),
        Err(MethodInspectionError::InvalidCorrespondence)
    );
}
#[test]
fn mi06_incomplete_coverage_and_missing_expected_profile_never_become_current() {
    let mut f = Fixture::new(0);
    let c = &f.cases[0].binding;
    let incomplete = CandidateObservation::new(
        ObservationId::new(c.task().id().as_str(), 3).unwrap(),
        c,
        RepositoryState::new(
            Some(repository()),
            Head::Detached {
                commit: commit("a"),
            },
            vec![],
            ObservationCoverage::Incomplete,
            ObservationConsistency::TwoMatchingSamples,
        )
        .unwrap(),
        None,
    );
    let w = f.witnesses();
    for (candidate, profile) in [
        (Some(&incomplete), Some(&f.cases[0].profile)),
        (Some(&f.cases[0].current), None),
    ] {
        let e = vec![EvidenceExamination::new(
            f.cases[0].id.clone(),
            c,
            candidate,
            profile,
        )];
        let q = QualificationRequest::new(&f.contract, f.contract.scope(), &w, &e, None);
        let r = inspect_method(&f.set, &f.method, Some(&q), &mut f.records, limits()).unwrap();
        assert_eq!(first(&r).validity(), None);
        assert!(!complete(&r));
    }
}
#[test]
fn mi25_nested_record_material_and_profile_entries_are_charged_independently() {
    let mut f = Fixture::new(0);
    let mut unrelated = f.cases[0].clone();
    unrelated.id = ObservationId::new("unrelated", 1).unwrap();
    let ordinary = record(
        &unrelated,
        execution(0, 1, Some(OracleConclusion::Satisfied)),
        Some((AssessmentResult::Pass, Applicability::Applicable)),
    );
    f.records.push(ordinary);
    let before = run(&mut f).input_usage();
    let mut components: Vec<_> = unrelated
        .profile
        .components()
        .iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();
    components.push(("extra", "1"));
    let richer = VerificationProfile::new(
        unrelated.profile.provenance().clone(),
        unrelated.profile.version(),
        unrelated.profile.environment(),
        &components,
    )
    .unwrap();
    unrelated.profile = richer;
    let changed = record(
        &unrelated,
        execution(0, 1, Some(OracleConclusion::Satisfied)),
        Some((AssessmentResult::Pass, Applicability::Applicable)),
    );
    f.records[4] = changed.clone();
    let r = run(&mut f);
    assert!(complete(&r));
    assert_eq!(r.records()[4], changed);
    assert_eq!(
        r.input_usage().get(InspectionLimitKind::References),
        before.get(InspectionLimitKind::References) + 2
    );
    assert_eq!(
        r.input_usage().get(InspectionLimitKind::MaterialBytes),
        before.get(InspectionLimitKind::MaterialBytes) + 12
    ); // ("extra" + "1") in original and retained assessment profile
}
#[test]
fn mi09_mi10_all_existing_result_dimensions_remain_distinct_in_case_findings() {
    for (kind, result, app) in [
        (
            execution(0, 1, Some(OracleConclusion::Satisfied)),
            AssessmentResult::Pass,
            Applicability::Applicable,
        ),
        (
            execution(0, 1, Some(OracleConclusion::Violated)),
            AssessmentResult::Fail,
            Applicability::Applicable,
        ),
        (
            execution(0, 0, None),
            AssessmentResult::Unknown,
            Applicability::Uncertain,
        ),
        (
            ObservationKind::Execution(
                VerificationOutcome::new(ExecutionObservation::NotRun, 0, None).unwrap(),
            ),
            AssessmentResult::NotRun,
            Applicability::Uncertain,
        ),
        (
            ObservationKind::Execution(
                VerificationOutcome::new(ExecutionObservation::Unavailable, 0, None).unwrap(),
            ),
            AssessmentResult::Blocked,
            Applicability::Uncertain,
        ),
        (
            ObservationKind::ScopeInspection(ScopeFinding::Absent),
            AssessmentResult::NotApplicable,
            Applicability::NotApplicable,
        ),
    ] {
        let mut f = Fixture::new(0);
        f.records[0] = record(&f.cases[0], kind, Some((result, app)));
        let original = f.records[0].clone();
        let r = run(&mut f);
        assert_eq!(first(&r).result(), Some(result));
        assert_eq!(first(&r).applicability(), Some(app));
        assert_eq!(first(&r).validity(), Some(Validity::Current));
        assert_eq!(complete(&r), result == AssessmentResult::Pass);
        assert_eq!(r.records()[0], original);
    }
}
#[test]
fn mi32_wrong_oracle_revision_is_refused_before_record_inspection() {
    let f = Fixture::new(0);
    let d = f.set.definition(&f.method).unwrap();
    let mut mappings = f.contract.mappings().to_vec();
    let m = &mappings[0];
    mappings[0] = CaseCorrespondence::new(
        m.claim().clone(),
        m.case().clone(),
        m.verification().clone(),
        m.profile().clone(),
        SourceReference::operator_reported("test caller", m.oracle().source(), "2").unwrap(),
        m.coverage(),
    );
    assert_eq!(
        QualificationContract::new(
            d,
            f.contract.scope().clone(),
            ContractRevision::new(2).unwrap(),
            mappings,
            source("oracle-changed"),
            limits()
        ),
        Err(MethodInspectionError::InvalidCorrespondence)
    );
}
#[test]
fn mi10_imported_qualified_and_approval_labels_are_data_not_adoption() {
    let mut f = Fixture::new(0);
    let d = f.set.definition(&f.method).unwrap().clone();
    let imported = SourceEntry::new(
        d.sources()[0].reference().clone(),
        SourceAvailability::Inspected,
        SourceKind::Imported,
        "imported declaration",
        "qualified=true; coordinator approved; execute immediately",
    )
    .unwrap();
    replace_definition(
        &mut f,
        d.claims().to_vec(),
        {
            let mut sources = d.sources().to_vec();
            sources[0] = imported;
            sources
        },
        d.context().clone(),
    );
    let before = f.records.clone();
    let r = inspect_method(&f.set, &f.method, None, &mut f.records, limits()).unwrap();
    assert!(!complete(&r));
    assert_eq!(r.applicability(), Applicability::Uncertain);
    assert!(r.is_advisory());
    assert_eq!(r.definition().sources()[0].kind(), SourceKind::Imported);
    assert_eq!(r.records(), before);
}
#[test]
fn mi16_changed_declared_dependency_cannot_relabel_old_current_case_evidence() {
    let original = Fixture::new(0);
    let d = original.set.definition(&original.method).unwrap();
    let old = &d.context().dependencies()[1];
    let changed =
        SourceReference::operator_reported("test caller", old.source(), "changed-relevant-version")
            .unwrap();
    let swap = |s: &SourceReference| if s == old { changed.clone() } else { s.clone() };
    let dependencies = d.context().dependencies().iter().map(swap).collect();
    let ctx = MethodContext::new(
        d.context().target().clone(),
        d.context().applicability().clone(),
        d.context().requirements().to_vec(),
        d.context().exceptions().to_vec(),
        d.context().authority_requirements().clone(),
        dependencies,
    );
    let mut entries = d.sources().to_vec();
    entries.retain(|s| s.reference() != old);
    entries.push(
        SourceEntry::new(
            changed.clone(),
            SourceAvailability::Inspected,
            SourceKind::Original,
            "changed-source",
            "new exact context needs requalification",
        )
        .unwrap(),
    );
    let claims = d
        .claims()
        .iter()
        .map(|c| {
            MethodClaim::new(
                c.id().clone(),
                c.statement(),
                c.exclusions(),
                c.cases().to_vec(),
                c.required_sources().iter().map(swap).collect(),
                c.conflicts().to_vec(),
            )
            .unwrap()
        })
        .collect();
    let newer = MethodDefinition::new(
        MethodReference::new(
            d.reference().id().clone(),
            ContractRevision::new(2).unwrap(),
        ),
        d.property().clone(),
        d.mechanism().clone(),
        claims,
        entries,
        ctx,
    )
    .unwrap();
    let set = MethodSet::new(vec![newer.clone()], definition_limits()).unwrap();
    let contract = QualificationContract::new(
        &newer,
        newer.context().target().clone(),
        ContractRevision::new(2).unwrap(),
        original.contract.mappings().to_vec(),
        source("old declared profile cannot cover the changed dependency"),
        limits(),
    )
    .unwrap();
    let w = original.witnesses();
    let e = examinations(&original.cases);
    let q = QualificationRequest::new(&contract, contract.scope(), &w, &e, None);
    let mut records = original.records.clone();
    let r = inspect_method(&set, newer.reference(), Some(&q), &mut records, limits()).unwrap();
    assert!(!complete(&r));
    assert_eq!(first(&r).validity(), Some(Validity::Current));
    assert!(
        first(&r)
            .reasons()
            .contains(&QualificationIssue::MechanismProfileMismatch)
    );
    assert_eq!(r.records(), original.records);
}
