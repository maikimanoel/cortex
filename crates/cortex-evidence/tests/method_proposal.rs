//! Advisory composition: explicit in-memory model data, no child processes or filesystem effects.
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
            "Caller-supplied deterministic model data; no authentication or execution",
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

use cortex_core::{
    parallel::*,
    program::{context::*, *},
};
use cortex_evidence::methods::proposal::*;

fn rev(n: u64) -> ContractRevision {
    ContractRevision::new(n).unwrap()
}
struct Target {
    state: ProgramState,
    data: CaseData,
    scope: TargetScope,
    expected_profile: VerificationProfile,
    app: ApplicabilityCorrespondence,
}
impl Target {
    fn new(f: &mut Fixture) -> Self {
        let d = f.set.definition(&f.method).unwrap();
        let b = binding("target-trigger", d.context().applicability().trigger());
        let p = f.cases[0].profile.clone();
        let scope = d.context().target().clone();
        let data = CaseData {
            baseline: 1,
            claim: ClaimId::new("trigger").unwrap(),
            case: CaseId::new("trigger").unwrap(),
            binding: b.clone(),
            current: capture(&b, 2, false),
            profile: p.clone(),
            id: ObservationId::new("target-trigger-report", 1).unwrap(),
        };
        f.records.push(record(
            &data,
            execution(0, 1, Some(OracleConclusion::Satisfied)),
            Some((AssessmentResult::Pass, Applicability::Applicable)),
        ));
        let task = Task::new(
            TaskContext::new(b.clone(), Some(capture(&b, 1, false))).unwrap(),
            ConsumerKind::Preview,
            DependencyCoverage::DeclaredComplete,
        );
        let graph = DependencyGraph::new(
            GraphReference::new(GraphId::new("method-program-graph").unwrap(), rev(1)),
            vec![task],
            vec![],
            GraphLimits::new(4, 4, 64, 128, 1_000_000).unwrap(),
        )
        .unwrap();
        let state = ProgramState::new(
            ProgramDefinition::new(
                ProgramId::new("method-proposal-program").unwrap(),
                ProgramOutcome::new(
                    "Compose explicitly selected method",
                    "Retain current correspondence and advisory limits",
                )
                .unwrap(),
                rev(1),
                None,
                source("accepted-program"),
                graph,
                vec![Workstream {
                    id: WorkstreamId::new("methods").unwrap(),
                    responsibility: "Advisory method composition".into(),
                    tasks: vec![b.task().id().clone()],
                }],
                vec![Increment {
                    id: IncrementId::new("method-context").unwrap(),
                    task: b.task().id().clone(),
                    artifacts: vec![],
                    claim_scopes: vec![ClaimScopeId::new("method-context").unwrap()],
                }],
                ProgramLimits::new(4, 128, 1_000_000).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        let app = ApplicabilityCorrespondence::new(
            scope.clone(),
            b,
            p.clone(),
            EvidenceReference::new(
                data.id.clone(),
                Some(AssessmentId::new("case-assessment").unwrap()),
            ),
            CaseCoverage::ExactCriterion,
        );
        Self {
            state,
            data,
            scope,
            expected_profile: p,
            app,
        }
    }
    fn package(&self) -> PackageRequest {
        PackageRequest {
            id: PackageId::new("method-context").unwrap(),
            revision: rev(1),
            task: self.data.binding.task().id().clone(),
            baseline: rev(1),
            generation: rev(1),
            sources: vec![SourceInput {
                source: source("explicit-target-source"),
                candidate: self.data.binding.candidate().clone(),
                text: "Explicit model target context".into(),
            }],
            methods: vec![],
            omissions: "Synthetic captures; no filesystem reread".into(),
            deliverable: "Inspectable advisory proposal".into(),
            stop_rules: "No execution, adoption or authority".into(),
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Normal,
    NoQualification,
    NoApplicability,
    MissingExamination,
    WrongQualificationVersion,
    DuplicateMethod,
    ChangedPackageSource,
    SmallLimits,
}
fn compose<'f>(
    f: &'f mut Fixture,
    t: &Target,
    mode: Mode,
) -> Result<MethodPackage<'f, 'f>, MethodProposalError> {
    let witnesses = f.witnesses();
    let mut exams = examinations(&f.cases);
    if mode == Mode::MissingExamination {
        exams.remove(0);
    }
    if mode != Mode::NoApplicability {
        exams.push(EvidenceExamination::new(
            t.data.id.clone(),
            &t.data.binding,
            Some(&t.data.current),
            Some(&t.data.profile),
        ));
    }
    let qualification = QualificationRequest::new(
        &f.contract,
        f.contract.scope(),
        &witnesses,
        &exams,
        (mode != Mode::NoApplicability).then_some(&t.app),
    );
    let method_source = SourceReference::operator_reported(
        "test caller",
        f.method.id().as_str(),
        &f.method.revision().get().to_string(),
    )
    .unwrap();
    let qualification_source = SourceReference::operator_reported(
        "test caller",
        "explicit model correspondence",
        if mode == Mode::WrongQualificationVersion {
            "2"
        } else {
            "1"
        },
    )
    .unwrap();
    let mut package = t.package();
    if mode == Mode::DuplicateMethod {
        package.methods.push(MethodProposal {
            method: method_source.clone(),
            applicability: "Existing explicit selection".into(),
            qualification: None,
            exceptions: "No exception approval".into(),
        });
    }
    if mode == Mode::ChangedPackageSource {
        package.sources[0].candidate =
            CandidateRequirement::new(repository(), commit("b"), WorktreeRequirement::Clean);
    }
    compose_method_package(
        &f.set,
        &mut f.records,
        &t.state,
        MethodPackageRequest {
            method: &f.method,
            method_source: &method_source,
            qualification: (mode != Mode::NoQualification).then_some(&qualification),
            qualification_source: (mode != Mode::NoQualification).then_some(&qualification_source),
            target_scope: &t.scope,
            current_target: Some(&t.data.current),
            expected_profile: &t.expected_profile,
            current_profile: Some(&t.data.profile),
            package,
        },
        if mode == Mode::SmallLimits {
            InspectionLimits::new(1, 1, 1, 1, 1, 1).unwrap()
        } else {
            limits()
        },
    )
}
fn selected<'a>(r: &'a MethodPackage<'_, '_>) -> &'a MethodProposal {
    &r.package().request().methods[0]
}
fn fixture() -> (Fixture, Target) {
    let mut f = Fixture::new(0);
    let t = Target::new(&mut f);
    (f, t)
}

#[test]
fn explicit_selection_preserves_exact_method_identity() {
    let (mut f, t) = fixture();
    let r = compose(&mut f, &t, Mode::Normal).unwrap();
    assert_eq!(r.package().request().methods.len(), 1);
    assert_eq!(
        selected(&r).method.source(),
        "native.candidate-correspondence"
    );
    assert_eq!(selected(&r).method.version(), "1");
}
#[test]
fn current_qualification_requires_exact_case_and_target_evidence() {
    let (mut f, t) = fixture();
    let r = compose(&mut f, &t, Mode::Normal).unwrap();
    assert_eq!(
        selected(&r).qualification,
        Some(source("explicit model correspondence"))
    );
    assert_eq!(r.inspection().applicability(), Applicability::Applicable);
    assert!(
        r.inspection()
            .claims()
            .iter()
            .all(ClaimInspection::requirements_met)
    );
}
#[test]
fn missing_qualification_is_an_unqualified_advisory_proposal() {
    let (mut f, t) = fixture();
    let r = compose(&mut f, &t, Mode::NoQualification).unwrap();
    assert_eq!(selected(&r).qualification, None);
    assert_eq!(r.inspection().applicability(), Applicability::Uncertain);
    assert!(
        r.inspection().claims()[0].cases()[0]
            .issues()
            .contains(&QualificationIssue::MissingWitness)
    );
}
#[test]
fn unsupported_analysis_remains_blocked_and_unqualified() {
    let (mut f, t) = fixture();
    f.records[0] = record(
        &f.cases[0],
        ObservationKind::Execution(
            VerificationOutcome::new(ExecutionObservation::Unavailable, 0, None).unwrap(),
        ),
        Some((AssessmentResult::Blocked, Applicability::Uncertain)),
    );
    let r = compose(&mut f, &t, Mode::Normal).unwrap();
    assert_eq!(selected(&r).qualification, None);
    assert_eq!(
        r.inspection().claims()[0].cases()[0].witnesses()[0].result(),
        Some(AssessmentResult::Blocked)
    );
}
#[test]
fn changed_definition_rejects_old_qualification_without_mutation() {
    let (mut f, t) = fixture();
    let d = f.set.definition(&f.method).unwrap();
    let changed = MethodDefinition::new(
        d.reference().clone(),
        d.property().clone(),
        MethodMechanism::new(
            d.mechanism().source().clone(),
            "Changed mechanism procedure",
            d.mechanism().observations(),
            d.mechanism().limitations(),
        )
        .unwrap(),
        d.claims().to_vec(),
        d.sources().to_vec(),
        d.context().clone(),
    )
    .unwrap();
    f.set = MethodSet::new(vec![changed], definition_limits()).unwrap();
    let before = f.records.clone();
    assert!(matches!(
        compose(&mut f, &t, Mode::Normal),
        Err(MethodProposalError::Inspection(
            MethodInspectionError::ContextChanged
        ))
    ));
    assert!(f.records == before);
}
#[test]
fn changed_qualification_contract_rejects_old_basis_reference() {
    let (mut f, t) = fixture();
    let old = f.contract.clone();
    f.contract = QualificationContract::new(
        old.definition(),
        old.scope().clone(),
        rev(2),
        old.mappings().to_vec(),
        SourceReference::operator_reported("test caller", "explicit model correspondence", "2")
            .unwrap(),
        limits(),
    )
    .unwrap();
    assert!(matches!(
        compose(&mut f, &t, Mode::Normal),
        Err(MethodProposalError::SourceCorrespondence)
    ));
}
#[test]
fn changed_candidate_commit_is_refused() {
    let (mut f, mut t) = fixture();
    t.data.current = CandidateObservation::new(
        ObservationId::new("target-trigger", 3).unwrap(),
        &t.data.binding,
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
        t.data.current.basis().cloned(),
    );
    assert!(matches!(
        compose(&mut f, &t, Mode::Normal),
        Err(MethodProposalError::TargetNotCurrent(_))
    ));
}
#[test]
fn changed_target_profile_is_refused() {
    let (mut f, mut t) = fixture();
    t.data.profile = VerificationProfile::new(
        Provenance::new("changed", "target", "profile").unwrap(),
        "2",
        "synthetic",
        &[],
    )
    .unwrap();
    assert!(matches!(
        compose(&mut f, &t, Mode::Normal),
        Err(MethodProposalError::TargetProfileMismatch)
    ));
}
#[test]
fn incompatible_scope_is_refused() {
    let (mut f, mut t) = fixture();
    t.scope = TargetScope::new(
        TaskId::new("different-scope").unwrap(),
        rev(1),
        ScopeKind::Interface,
        vec![],
        None,
        None,
        None,
    )
    .unwrap();
    assert!(matches!(
        compose(&mut f, &t, Mode::Normal),
        Err(MethodProposalError::TargetScopeMismatch)
    ));
}
#[test]
fn missing_examination_remains_explicit_and_unqualified() {
    let (mut f, t) = fixture();
    let r = compose(&mut f, &t, Mode::MissingExamination).unwrap();
    assert_eq!(selected(&r).qualification, None);
    assert!(
        r.inspection().claims()[0].cases()[0].witnesses()[0]
            .reasons()
            .contains(&QualificationIssue::MissingExamination)
    );
}
#[test]
fn contradictory_current_witness_prevents_qualification() {
    let (mut f, t) = fixture();
    f.records[0] = record(
        &f.cases[0],
        execution(1, 1, Some(OracleConclusion::Violated)),
        Some((AssessmentResult::Fail, Applicability::Applicable)),
    );
    let r = compose(&mut f, &t, Mode::Normal).unwrap();
    assert_eq!(selected(&r).qualification, None);
    assert!(r.inspection().claims()[0].cases()[0].witnesses()[0].contradicts());
}
#[test]
fn unresolved_protected_exception_is_retained_without_qualification() {
    let starter = starter_methods(definition_limits()).unwrap();
    let d = &starter.definitions()[0];
    let c = d.context();
    let changed = MethodDefinition::new(
        d.reference().clone(),
        d.property().clone(),
        d.mechanism().clone(),
        d.claims().to_vec(),
        d.sources().to_vec(),
        MethodContext::new(
            c.target().clone(),
            c.applicability().clone(),
            c.requirements().to_vec(),
            vec![source("unresolved-protected-exception")],
            c.authority_requirements().clone(),
            c.dependencies().to_vec(),
        ),
    )
    .unwrap();
    let mut f = Fixture::from_definition(changed);
    let t = Target::new(&mut f);
    let r = compose(&mut f, &t, Mode::Normal).unwrap();
    assert_eq!(selected(&r).qualification, None);
    assert_eq!(
        r.inspection().exceptions(),
        &[source("unresolved-protected-exception")]
    );
    assert!(
        selected(&r)
            .exceptions
            .contains("unresolved-protected-exception")
    );
}
#[test]
fn reviewer_qualification_version_substitution_is_refused() {
    let (mut f, t) = fixture();
    assert!(matches!(
        compose(&mut f, &t, Mode::WrongQualificationVersion),
        Err(MethodProposalError::SourceCorrespondence)
    ));
}
#[test]
fn reviewer_structural_basis_match_cannot_hide_dirty_current_target() {
    let (mut f, mut t) = fixture();
    let witnesses = f.witnesses();
    let exams = examinations(&f.cases);
    let q = QualificationRequest::new(&f.contract, f.contract.scope(), &witnesses, &exams, None);
    let mut old_records = f.records.clone();
    let old = inspect_method(&f.set, &f.method, Some(&q), &mut old_records, limits()).unwrap();
    old.check_basis(
        f.set.definition(&f.method).unwrap(),
        Some(&f.contract),
        Some(f.contract.scope()),
    )
    .unwrap();
    t.data.current = capture(&t.data.binding, 3, true);
    let before = f.records.clone();
    assert!(matches!(
        compose(&mut f, &t, Mode::Normal),
        Err(MethodProposalError::TargetNotCurrent(_))
    ));
    assert!(f.records == before);
}
#[test]
fn independent_valid_neighbor_remains_available_after_a_refusal() {
    let (mut f, mut t) = fixture();
    t.data.current = capture(&t.data.binding, 3, true);
    assert!(compose(&mut f, &t, Mode::Normal).is_err());
    let mut neighbor = Fixture::new(1);
    let target = Target::new(&mut neighbor);
    let r = compose(&mut neighbor, &target, Mode::Normal).unwrap();
    assert_eq!(selected(&r).method.source(), "native.assessment-adequacy");
    assert!(selected(&r).qualification.is_some());
}
#[test]
fn unsupported_non_rust_target_is_not_assumed_applicable() {
    let (mut f, mut t) = fixture();
    t.scope = TargetScope::new(
        t.scope.id().clone(),
        rev(1),
        ScopeKind::Component,
        vec![
            ComponentScope::new(
                TaskId::new("python-component").unwrap(),
                Some("Python"),
                None,
                None,
                vec![],
            )
            .unwrap(),
        ],
        None,
        None,
        None,
    )
    .unwrap();
    assert!(matches!(
        compose(&mut f, &t, Mode::Normal),
        Err(MethodProposalError::TargetScopeMismatch)
    ));
}
#[test]
fn assembled_package_preserves_exact_selected_basis() {
    let (mut f, t) = fixture();
    let r = compose(&mut f, &t, Mode::Normal).unwrap();
    let proposal = selected(&r);
    assert_eq!(proposal.method, source("native.candidate-correspondence"));
    assert_eq!(
        proposal.qualification,
        Some(source("explicit model correspondence"))
    );
    r.package().check(&t.state, r.package().bytes()).unwrap();
    assert!(
        std::str::from_utf8(r.package().bytes())
            .unwrap()
            .contains("EXPLICIT_PROPOSAL_ONLY")
    );
}
#[test]
fn package_source_requirement_change_is_refused_without_record_mutation() {
    let (mut f, t) = fixture();
    let before = f.records.clone();
    assert!(matches!(
        compose(&mut f, &t, Mode::ChangedPackageSource),
        Err(MethodProposalError::Program(ProgramError::Stale))
    ));
    assert!(f.records == before);
}
#[test]
fn terminal_assessments_remain_unchanged_after_stale_case_examination() {
    let (mut f, t) = fixture();
    let terminal: Vec<_> = f.records.iter().map(|r| r.assessment().cloned()).collect();
    f.cases[0].current = capture(&f.cases[0].binding, 3, true);
    let r = compose(&mut f, &t, Mode::Normal).unwrap();
    assert_eq!(selected(&r).qualification, None);
    assert_eq!(
        r.inspection()
            .records()
            .iter()
            .map(|r| r.assessment().cloned())
            .collect::<Vec<_>>(),
        terminal
    );
    assert!(r.inspection().records()[0].invalidation_reason().is_some());
}
#[test]
fn composition_neither_adopts_method_nor_mutates_program() {
    let (mut f, t) = fixture();
    let original_set = f.set.clone();
    let original_state = t.state.clone();
    let r = compose(&mut f, &t, Mode::Normal).unwrap();
    assert!(r.inspection().is_advisory());
    drop(r);
    assert!(f.set == original_set);
    assert!(t.state == original_state);
}
#[test]
fn qualification_cases_without_target_applicability_remain_unqualified() {
    let (mut f, t) = fixture();
    let r = compose(&mut f, &t, Mode::NoApplicability).unwrap();
    assert!(
        r.inspection()
            .claims()
            .iter()
            .all(ClaimInspection::requirements_met)
    );
    assert_eq!(r.inspection().applicability(), Applicability::Uncertain);
    assert_eq!(selected(&r).qualification, None);
}
#[test]
fn duplicate_method_selection_is_refused() {
    let (mut f, t) = fixture();
    assert!(matches!(
        compose(&mut f, &t, Mode::DuplicateMethod),
        Err(MethodProposalError::DuplicateMethod)
    ));
}
#[test]
fn bounded_material_rejection_preserves_all_records() {
    let (mut f, t) = fixture();
    let before = f.records.clone();
    assert!(matches!(
        compose(&mut f, &t, Mode::SmallLimits),
        Err(MethodProposalError::Inspection(
            MethodInspectionError::Limit(_)
        ))
    ));
    assert!(f.records == before);
}
