//! Advisory qualification inspection for exact native method definitions.
//!
//! Caller correspondence and reports are unauthenticated data. This consumer does
//! not assess, adopt, select, execute, grant permission or release a dependency.
//! Malformed input returns Err without mutation. Admitted negative findings retain
//! legitimate first invalidation from unified inspection in referenced records.
//! Ordinary allocation panics/aborts are outside typed-error atomicity guarantees.

//! ```
//! use cortex_core::methods::*;
//! use cortex_evidence::methods::*;
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let definitions = starter_methods(DefinitionLimits::new(2, 8, 16, 128, 100_000)?)?;
//! let method = definitions.definitions()[0].reference();
//! let mut records = Vec::new();
//! let report = inspect_method(&definitions, method, None, &mut records,
//!     InspectionLimits::new(1, 1, 16, 128, 100_000, 128)?)?;
//! assert!(report.is_advisory());
//! assert!(!report.claims()[0].requirements_met());
//! # Ok(()) }
//! ```

use crate::unified::{
    Applicability, AssessmentId, AssessmentResult, EvidenceRecord, ObservationKind, ScopeFinding,
    ValidationContext, Validity, ValidityIssue, VerificationProfile,
};
use cortex_core::{
    binding::{
        CandidateBindingContract, CandidateObservation, CandidateRequirement, ContractRevision,
        Head, ObservationId,
    },
    methods::*,
    temporal::{SourceOrigin, SourceReference},
};
use std::{error::Error, fmt};
macro_rules! opaque {($($t:ty),+ $(,)?)=>{$(impl fmt::Debug for $t{fn fmt(&self,f:&mut fmt::Formatter<'_>)->fmt::Result{f.write_str(concat!(stringify!($t),"([redacted])"))}})+};}

/// Logical admission dimension, never a host resource grant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InspectionLimitKind {
    /// Pool record plus examination occurrences.
    Records,
    /// Witness assignment occurrences before duplicate collapse.
    Assignments,
    /// Definition and correspondence case occurrences.
    Cases,
    /// Nested source/component/requirement references.
    References,
    /// Nested source/identifier/text/path/comparison byte occurrences.
    MaterialBytes,
    /// Claim/case/witness/issue and source-gap output slots.
    Findings,
}
impl InspectionLimitKind {
    fn index(self) -> usize {
        self as usize
    }
}
/// Explicit positive finite limits; no unlimited default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InspectionLimits([usize; 6]);
impl InspectionLimits {
    /// Validate each caller-selected bound independently.
    pub fn new(
        records: usize,
        assignments: usize,
        cases: usize,
        references: usize,
        material: usize,
        findings: usize,
    ) -> Result<Self, MethodInspectionError> {
        let v = [records, assignments, cases, references, material, findings];
        if v.iter().any(|n| *n == 0 || *n > isize::MAX as usize) {
            return Err(MethodInspectionError::InvalidLimits);
        }
        Ok(Self(v))
    }
    /// Read one logical bound.
    pub fn get(self, kind: InspectionLimitKind) -> usize {
        self.0[kind.index()]
    }
}
/// Logical input/output usage; fixed numeric/enum storage and allocation overhead excluded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct InspectionUsage([usize; 6]);
impl InspectionUsage {
    /// Read charged usage.
    pub fn get(self, kind: InspectionLimitKind) -> usize {
        self.0[kind.index()]
    }
    fn add(
        &mut self,
        kind: InspectionLimitKind,
        count: usize,
        limits: InspectionLimits,
    ) -> Result<(), MethodInspectionError> {
        let n = &mut self.0[kind.index()];
        *n = n
            .checked_add(count)
            .ok_or(MethodInspectionError::Overflow)?;
        if *n > limits.get(kind) {
            return Err(MethodInspectionError::Limit(kind));
        }
        Ok(())
    }
}
/// Structural refusal; all supplied records remain unchanged on typed errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MethodInspectionError {
    /// Core definition validation refused an input.
    Definition(MethodDefinitionError),
    /// A limit is zero or unrepresentable.
    InvalidLimits,
    /// Exact definition/scope/correspondence changed; reinspection is required.
    ContextChanged,
    /// Report/capture/task-revision identity has ambiguous contents.
    IdentityConflict,
    /// A case mapping, reference or examination is malformed.
    InvalidCorrespondence,
    /// One source report has conflicting examinations.
    ConflictingExamination,
    /// A logical budget was exceeded.
    Limit(InspectionLimitKind),
    /// Logical arithmetic overflowed.
    Overflow,
    /// Fallible storage reservation failed before publication.
    Allocation,
}
impl From<MethodDefinitionError> for MethodInspectionError {
    fn from(e: MethodDefinitionError) -> Self {
        Self::Definition(e)
    }
}
impl fmt::Display for MethodInspectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Definition(_) => "invalid method definition reference",
            Self::InvalidLimits => "inspection limits must be positive and finite",
            Self::ContextChanged => "method qualification context changed",
            Self::IdentityConflict => "qualification identity contents conflict",
            Self::InvalidCorrespondence => "qualification correspondence is invalid",
            Self::ConflictingExamination => "qualification examinations conflict",
            Self::Limit(_) => "qualification exceeds a logical budget",
            Self::Overflow => "qualification accounting overflow",
            Self::Allocation => "qualification allocation unavailable",
        })
    }
}
impl Error for MethodInspectionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Definition(e) => Some(e),
            _ => None,
        }
    }
}

/// Declared full single-criterion coverage, not semantic authentication.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaseCoverage {
    /// Caller asserts the entire criterion covers this exact case.
    ExactCriterion,
    /// Adequate coverage has not been established.
    Unestablished,
}
/// Exact case-to-criterion/profile/oracle declaration.
#[derive(Clone, PartialEq, Eq)]
pub struct CaseCorrespondence {
    claim: ClaimId,
    case: CaseId,
    verification: CandidateBindingContract,
    profile: VerificationProfile,
    oracle: SourceReference,
    coverage: CaseCoverage,
}
impl CaseCorrespondence {
    /// Declare correspondence without asserting that verification actually ran.
    pub fn new(
        claim: ClaimId,
        case: CaseId,
        verification: CandidateBindingContract,
        profile: VerificationProfile,
        oracle: SourceReference,
        coverage: CaseCoverage,
    ) -> Self {
        Self {
            claim,
            case,
            verification,
            profile,
            oracle,
            coverage,
        }
    }
    /// Read claim identity.
    pub fn claim(&self) -> &ClaimId {
        &self.claim
    }
    /// Read case identity.
    pub fn case(&self) -> &CaseId {
        &self.case
    }
    /// Read complete case criterion and candidate requirements.
    pub fn verification(&self) -> &CandidateBindingContract {
        &self.verification
    }
    /// Read exact expected mechanism context.
    pub fn profile(&self) -> &VerificationProfile {
        &self.profile
    }
    /// Read declared oracle reference.
    pub fn oracle(&self) -> &SourceReference {
        &self.oracle
    }
    /// Read declared coverage, not an assessment.
    pub fn coverage(&self) -> CaseCoverage {
        self.coverage
    }
}
/// Immutable exact qualification context; no adopted method or stored validity flag.
#[derive(Clone, PartialEq, Eq)]
pub struct QualificationContract {
    definition: MethodDefinition,
    scope: TargetScope,
    revision: ContractRevision,
    mappings: Vec<CaseCorrespondence>,
    basis: SourceReference,
}
impl QualificationContract {
    /// Require exactly one mapping per declared case before copying the definition.
    /// All case roles are required within this declared finite qualification scope.
    pub fn new(
        definition: &MethodDefinition,
        scope: TargetScope,
        revision: ContractRevision,
        mappings: Vec<CaseCorrespondence>,
        basis: SourceReference,
        limits: InspectionLimits,
    ) -> Result<Self, MethodInspectionError> {
        let mut usage = InspectionUsage::default();
        charge_definition(&mut usage, definition, limits)?;
        charge_scope(&mut usage, &scope, limits)?;
        let mut m = Material::new(limits.get(InspectionLimitKind::MaterialBytes));
        m.bytes = usage.get(InspectionLimitKind::MaterialBytes);
        m.source(&basis)?;
        usage.add(InspectionLimitKind::Cases, mappings.len(), limits)?;
        for (i, c) in mappings.iter().enumerate() {
            if mappings[..i]
                .iter()
                .any(|p| p.claim == c.claim && p.case == c.case)
            {
                return Err(MethodInspectionError::InvalidCorrespondence);
            }
            let case = definition
                .claims()
                .iter()
                .find(|cl| cl.id() == &c.claim)
                .and_then(|cl| cl.cases().iter().find(|case| case.id() == &c.case))
                .ok_or(MethodInspectionError::InvalidCorrespondence)?;
            if case.expected() != c.verification.task().acceptance() || case.oracle() != &c.oracle {
                return Err(MethodInspectionError::InvalidCorrespondence);
            }
            m.mapping(c)?;
        }
        if mappings.len()
            != definition.claims().iter().try_fold(0usize, |n, c| {
                n.checked_add(c.cases().len())
                    .ok_or(MethodInspectionError::Overflow)
            })?
        {
            return Err(MethodInspectionError::InvalidCorrespondence);
        }
        usage.add(InspectionLimitKind::References, m.references, limits)?;
        Ok(Self {
            definition: definition.clone(),
            scope,
            revision,
            mappings,
            basis,
        })
    }
    /// Read the retained exact definition.
    pub fn definition(&self) -> &MethodDefinition {
        &self.definition
    }
    /// Read declared qualification scope, not inferred applicability.
    pub fn scope(&self) -> &TargetScope {
        &self.scope
    }
    /// Read correspondence revision.
    pub fn revision(&self) -> ContractRevision {
        self.revision
    }
    /// Read every exact case mapping.
    pub fn mappings(&self) -> &[CaseCorrespondence] {
        &self.mappings
    }
    /// Read caller correspondence rationale/provenance.
    pub fn basis(&self) -> &SourceReference {
        &self.basis
    }
}
/// Compound identity; assessment identities are not globally unique.
#[derive(Clone, PartialEq, Eq)]
pub struct EvidenceReference {
    observation: ObservationId,
    assessment: Option<AssessmentId>,
}
impl EvidenceReference {
    /// An absent assessment reference remains absent even if a record has a judgment.
    pub fn new(observation: ObservationId, assessment: Option<AssessmentId>) -> Self {
        Self {
            observation,
            assessment,
        }
    }
    /// Read verification report identity.
    pub fn observation(&self) -> &ObservationId {
        &self.observation
    }
    /// Read requested assessment identity.
    pub fn assessment(&self) -> Option<&AssessmentId> {
        self.assessment.as_ref()
    }
}
/// One exact case assignment, independent of sibling cases and methods.
#[derive(Clone, PartialEq, Eq)]
pub struct CaseWitness {
    claim: ClaimId,
    case: CaseId,
    evidence: EvidenceReference,
}
impl CaseWitness {
    /// Assign a source without manufacturing assessment/capture authority.
    pub fn new(claim: ClaimId, case: CaseId, evidence: EvidenceReference) -> Self {
        Self {
            claim,
            case,
            evidence,
        }
    }
    /// Read assigned claim.
    pub fn claim(&self) -> &ClaimId {
        &self.claim
    }
    /// Read assigned case.
    pub fn case(&self) -> &CaseId {
        &self.case
    }
    /// Read exact source reference.
    pub fn evidence(&self) -> &EvidenceReference {
        &self.evidence
    }
}
/// Explicit current source context; missing candidate/profile remains unresolved.
#[derive(Clone, PartialEq, Eq)]
pub struct EvidenceExamination<'a> {
    observation: ObservationId,
    contract: &'a CandidateBindingContract,
    candidate: Option<&'a CandidateObservation>,
    profile: Option<&'a VerificationProfile>,
}
impl<'a> EvidenceExamination<'a> {
    /// Retain borrowed inputs for existing unified inspection.
    pub fn new(
        observation: ObservationId,
        contract: &'a CandidateBindingContract,
        candidate: Option<&'a CandidateObservation>,
        profile: Option<&'a VerificationProfile>,
    ) -> Self {
        Self {
            observation,
            contract,
            candidate,
            profile,
        }
    }
    /// Read report identity.
    pub fn observation(&self) -> &ObservationId {
        &self.observation
    }
}
/// Optional target-trigger examination, distinct from qualification fixtures.
#[derive(Clone, PartialEq, Eq)]
pub struct ApplicabilityCorrespondence {
    scope: TargetScope,
    verification: CandidateBindingContract,
    profile: VerificationProfile,
    evidence: EvidenceReference,
    coverage: CaseCoverage,
}
impl ApplicabilityCorrespondence {
    /// Criterion must be the definition's entire declared trigger; absence needs N/A evidence.
    pub fn new(
        scope: TargetScope,
        verification: CandidateBindingContract,
        profile: VerificationProfile,
        evidence: EvidenceReference,
        coverage: CaseCoverage,
    ) -> Self {
        Self {
            scope,
            verification,
            profile,
            evidence,
            coverage,
        }
    }
    /// Read requested target scope.
    pub fn scope(&self) -> &TargetScope {
        &self.scope
    }
    /// Read the full target-trigger criterion and candidate declaration.
    pub fn verification(&self) -> &CandidateBindingContract {
        &self.verification
    }
    /// Read exact target-trigger verification context.
    pub fn profile(&self) -> &VerificationProfile {
        &self.profile
    }
    /// Read declared full criterion coverage.
    pub fn coverage(&self) -> CaseCoverage {
        self.coverage
    }
    /// Read the original source reference.
    pub fn evidence(&self) -> &EvidenceReference {
        &self.evidence
    }
}
/// Borrowed request; no hidden capture, lookup or callback.
pub struct QualificationRequest<'a> {
    contract: &'a QualificationContract,
    scope: &'a TargetScope,
    witnesses: &'a [CaseWitness],
    examinations: &'a [EvidenceExamination<'a>],
    applicability: Option<&'a ApplicabilityCorrespondence>,
}
impl<'a> QualificationRequest<'a> {
    /// Declare all inputs; structural admission happens before any mutation.
    pub fn new(
        contract: &'a QualificationContract,
        scope: &'a TargetScope,
        witnesses: &'a [CaseWitness],
        examinations: &'a [EvidenceExamination<'a>],
        applicability: Option<&'a ApplicabilityCorrespondence>,
    ) -> Self {
        Self {
            contract,
            scope,
            witnesses,
            examinations,
            applicability,
        }
    }
}
/// Precise non-support cause, not a replacement Evidence verdict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QualificationIssue {
    /// No source was assigned.
    MissingWitness,
    /// Assigned report is absent from the pool.
    MissingRecord,
    /// Requested terminal judgment was not named.
    MissingAssessmentReference,
    /// Requested terminal judgment does not match this report.
    AssessmentMismatch,
    /// No current context examination was supplied.
    MissingExamination,
    /// Report's full criterion/candidate contract differs from the case mapping.
    CriterionMismatch,
    /// Report profile differs from the exact mapped profile.
    ProfileMismatch,
    /// Declared mechanism/oracle/fixture profile references do not match definition.
    MechanismProfileMismatch,
    /// Full single-criterion coverage remains unestablished.
    CoverageUnestablished,
    /// Existing unified semantics do not establish a current applicable PASS.
    NotCurrentPass,
    /// Requested target exceeds exact declared qualification scope.
    ScopeMismatch,
    /// Required source is unavailable, summary-only or unassessed.
    RequiredSourceUnavailable,
    /// Required source statements remain in conflict.
    SourceDisagreement,
    /// No violation case has been declared.
    MissingViolation,
    /// No legitimate neighboring case has been declared.
    MissingNeighbor,
}
/// One witness's historical dimensions and current support reasons.
#[derive(Clone, PartialEq, Eq)]
pub struct WitnessFinding {
    reference: EvidenceReference,
    result: Option<AssessmentResult>,
    applicability: Option<Applicability>,
    validity: Option<Validity>,
    issue: Option<ValidityIssue>,
    compared_to: Option<ObservationId>,
    reasons: Vec<QualificationIssue>,
}
impl WitnessFinding {
    /// Read requested source identity without filling missing assessments.
    pub fn reference(&self) -> &EvidenceReference {
        &self.reference
    }
    /// Read historical result, including a stored assessment not requested for support.
    pub fn result(&self) -> Option<AssessmentResult> {
        self.result
    }
    /// Read historical applicability independently of current validity.
    pub fn applicability(&self) -> Option<Applicability> {
        self.applicability
    }
    /// Read CURRENT/STALE or unestablished validity.
    pub fn validity(&self) -> Option<Validity> {
        self.validity
    }
    /// Read existing unified invalidation/uncertainty cause.
    pub fn validity_issue(&self) -> Option<&ValidityIssue> {
        self.issue.as_ref()
    }
    /// Read compared capture or sticky first-invalidation reference.
    pub fn compared_to(&self) -> Option<&ObservationId> {
        self.compared_to.as_ref()
    }
    /// Read all non-support causes.
    pub fn reasons(&self) -> &[QualificationIssue] {
        &self.reasons
    }
    /// Conditional case support only, never permission or an Evidence verdict.
    pub fn requirements_met(&self) -> bool {
        self.reasons.is_empty()
    }
    /// A matching CURRENT applicable FAIL contradicts this case, not the broader property.
    pub fn contradicts(&self) -> bool {
        self.result == Some(AssessmentResult::Fail)
            && self.applicability == Some(Applicability::Applicable)
            && self.validity == Some(Validity::Current)
            && self
                .reasons
                .iter()
                .all(|r| *r == QualificationIssue::NotCurrentPass)
    }
}
/// Conjunctive support for one explicitly required case.
#[derive(Clone, PartialEq, Eq)]
pub struct CaseInspection {
    case: CaseId,
    witnesses: Vec<WitnessFinding>,
    issues: Vec<QualificationIssue>,
}
impl CaseInspection {
    /// Read exact case identity.
    pub fn case(&self) -> &CaseId {
        &self.case
    }
    /// Inspect every distinct assigned witness.
    pub fn witnesses(&self) -> &[WitnessFinding] {
        &self.witnesses
    }
    /// Inspect case-level missing coverage.
    pub fn issues(&self) -> &[QualificationIssue] {
        &self.issues
    }
    /// Every assigned witness and all case requirements must be satisfied.
    pub fn requirements_met(&self) -> bool {
        self.issues.is_empty()
            && !self.witnesses.is_empty()
            && self.witnesses.iter().all(WitnessFinding::requirements_met)
    }
    /// Retain a contradictory witness despite another positive witness.
    pub fn contradicted(&self) -> bool {
        self.issues.is_empty() && self.witnesses.iter().any(WitnessFinding::contradicts)
    }
}
/// Claim-scoped coverage; partial support and exclusions remain inspectable.
#[derive(Clone, PartialEq, Eq)]
pub struct ClaimInspection {
    claim: ClaimId,
    cases: Vec<CaseInspection>,
    issues: Vec<QualificationIssue>,
}
impl ClaimInspection {
    /// Read exact claim identity.
    pub fn claim(&self) -> &ClaimId {
        &self.claim
    }
    /// Inspect all required cases.
    pub fn cases(&self) -> &[CaseInspection] {
        &self.cases
    }
    /// Inspect role/source/scope gaps.
    pub fn issues(&self) -> &[QualificationIssue] {
        &self.issues
    }
    /// Conditional complete coverage of this declared finite scope, not adoption.
    pub fn requirements_met(&self) -> bool {
        self.issues.is_empty()
            && !self.cases.is_empty()
            && self.cases.iter().all(CaseInspection::requirements_met)
    }
}
/// Owned full examination basis, not a new validity or capture authority.
#[derive(Clone, PartialEq, Eq)]
pub struct ExaminationBasis {
    observation: ObservationId,
    contract: CandidateBindingContract,
    candidate: Option<CandidateObservation>,
    profile: Option<VerificationProfile>,
}
impl ExaminationBasis {
    /// Read report identity.
    pub fn observation(&self) -> &ObservationId {
        &self.observation
    }
    /// Read supplied complete contract.
    pub fn contract(&self) -> &CandidateBindingContract {
        &self.contract
    }
    /// Read supplied current capture; not authenticated.
    pub fn candidate(&self) -> Option<&CandidateObservation> {
        self.candidate.as_ref()
    }
    /// Read supplied expected profile.
    pub fn profile(&self) -> Option<&VerificationProfile> {
        self.profile.as_ref()
    }
}
/// Complete retained correspondence, separate from the original source records.
#[derive(Clone, PartialEq, Eq)]
pub struct QualificationBasis {
    contract: Option<QualificationContract>,
    examinations: Vec<ExaminationBasis>,
    witnesses: Vec<CaseWitness>,
    applicability: Option<ApplicabilityCorrespondence>,
}
impl QualificationBasis {
    /// Read exact frozen definition/scope/correspondence, if supplied.
    pub fn contract(&self) -> Option<&QualificationContract> {
        self.contract.as_ref()
    }
    /// Read all supplied examinations and their current captures.
    pub fn examinations(&self) -> &[ExaminationBasis] {
        &self.examinations
    }
    /// Read all assigned sources, including duplicates charged at admission.
    pub fn witnesses(&self) -> &[CaseWitness] {
        &self.witnesses
    }
    /// Read optional target applicability evidence basis.
    pub fn applicability(&self) -> Option<&ApplicabilityCorrespondence> {
        self.applicability.as_ref()
    }
}
/// Final borrowed result; positive coverage is narrower than adoption or readiness.
pub struct MethodInspection<'d, 'r> {
    definition: &'d MethodDefinition,
    basis: QualificationBasis,
    claims: Vec<ClaimInspection>,
    applicability: Applicability,
    app_witness: Option<WitnessFinding>,
    records: &'r [EvidenceRecord],
    usage: InspectionUsage,
}
impl<'d, 'r> MethodInspection<'d, 'r> {
    /// Inspect immutable definition, exclusions and source provenance explicitly.
    pub fn definition(&self) -> &'d MethodDefinition {
        self.definition
    }
    /// Inspect retained exact correspondence and examination basis.
    pub fn basis(&self) -> &QualificationBasis {
        &self.basis
    }
    /// Inspect per-claim/case/witness support and contradictions.
    pub fn claims(&self) -> &[ClaimInspection] {
        &self.claims
    }
    /// Inspect concrete applicability; descriptive relevance is separate.
    pub fn applicability(&self) -> Applicability {
        self.applicability
    }
    /// Inspect optional target-trigger evidence and unresolved reasons.
    pub fn applicability_witness(&self) -> Option<&WitnessFinding> {
        self.app_witness.as_ref()
    }
    /// Original caller records, including untouched unrelated records; private getters are opt-in.
    pub fn records(&self) -> &'r [EvidenceRecord] {
        self.records
    }
    /// Inherited requirements cannot be removed by an exception or favorable finding.
    pub fn requirements(&self) -> &[SourceReference] {
        self.definition.context().requirements()
    }
    /// Exceptions are unresolved declarations, never approved amendments.
    pub fn exceptions(&self) -> &[SourceReference] {
        self.definition.context().exceptions()
    }
    /// Expose unavailable/summary/unassessed source entries, including optional background gaps.
    pub fn source_gaps(&self) -> impl Iterator<Item = &SourceEntry> {
        self.definition
            .sources()
            .iter()
            .filter(|s| s.availability() != SourceAvailability::Inspected)
    }
    /// Read admitted logical accounting; not heap or OS usage.
    pub fn input_usage(&self) -> InspectionUsage {
        self.usage
    }
    /// Always advisory; no conversion to an authority or G1 release type exists.
    pub fn is_advisory(&self) -> bool {
        true
    }
    /// Structural basis recheck, never live capture or transitive freshness.
    pub fn check_basis(
        &self,
        definition: &MethodDefinition,
        contract: Option<&QualificationContract>,
        scope: Option<&TargetScope>,
    ) -> Result<(), MethodInspectionError> {
        if self.definition != definition
            || self.basis.contract.as_ref() != contract
            || contract.map(|c| &c.scope) != scope
        {
            return Err(MethodInspectionError::ContextChanged);
        }
        Ok(())
    }
}
opaque!(
    CaseCorrespondence,
    QualificationContract,
    EvidenceReference,
    CaseWitness,
    EvidenceExamination<'_>,
    ApplicabilityCorrespondence,
    QualificationRequest<'_>,
    WitnessFinding,
    CaseInspection,
    ClaimInspection,
    ExaminationBasis,
    QualificationBasis,
    MethodInspection<'_, '_>
);

/// Inspect exactly one method without assessing, adopting, executing or granting authority.
///
/// Logical accounting charges occurrences before deduplication, nested retained contexts,
/// text/path/comparison bytes, plus a 128-byte allowance per opaque invalidation scope.
/// Typed errors leave every supplied record unchanged. Successful negative findings may
/// retain first sticky invalidation; source reports and terminal judgments never change.
/// No callback, I/O or clock is used. Allocation panics/aborts are not recoverable here.
pub fn inspect_method<'d, 'r>(
    definitions: &'d MethodSet,
    method: &MethodReference,
    qualification: Option<&QualificationRequest<'_>>,
    records: &'r mut [EvidenceRecord],
    limits: InspectionLimits,
) -> Result<MethodInspection<'d, 'r>, MethodInspectionError> {
    let definition = definitions.definition(method)?;
    let mut usage = measure(definition, qualification, records, limits)?;
    if let Some(q) = qualification {
        validate(definition, q, records)?;
    }
    let mut drafts = reserved(records.len())?;
    if let Some(q) = qualification {
        for (i, r) in records.iter().enumerate() {
            if !referenced(q, r.observation().id()) {
                continue;
            }
            let mut draft = r.clone();
            let dimensions = q
                .examinations
                .iter()
                .find(|e| &e.observation == r.observation().id())
                .map(|e| {
                    let v =
                        draft.inspect(&ValidationContext::new(e.contract, e.candidate, e.profile));
                    (
                        v.validity(),
                        v.validity_issue().cloned(),
                        v.compared_to().cloned(),
                        v.current_pass(),
                    )
                });
            drafts.push((i, draft, dimensions));
        }
    }
    let mut claims = reserved(definition.claims().len())?;
    for claim in definition.claims() {
        let mut issues = Vec::new();
        if !claim
            .cases()
            .iter()
            .any(|c| c.role() == CaseRole::Violation)
        {
            issues.push(QualificationIssue::MissingViolation);
        }
        if !claim
            .cases()
            .iter()
            .any(|c| c.role() == CaseRole::LegitimateNeighbor)
        {
            issues.push(QualificationIssue::MissingNeighbor);
        }
        if !claim.conflicts().is_empty() {
            issues.push(QualificationIssue::SourceDisagreement);
        }
        if claim
            .required_sources()
            .iter()
            .chain(definition.context().dependencies())
            .any(|r| {
                !definition.sources().iter().any(|s| {
                    s.reference() == r && s.availability() == SourceAvailability::Inspected
                })
            })
        {
            issues.push(QualificationIssue::RequiredSourceUnavailable);
        }
        if qualification.is_some_and(|q| q.scope != definition.context().target()) {
            issues.push(QualificationIssue::ScopeMismatch);
        }
        let mut cases = reserved(claim.cases().len())?;
        for case in claim.cases() {
            let mut witnesses = Vec::new();
            let mut case_issues = Vec::new();
            if let Some(q) = qualification {
                let mapping = q
                    .contract
                    .mappings
                    .iter()
                    .find(|m| m.claim == *claim.id() && m.case == *case.id())
                    .ok_or(MethodInspectionError::InvalidCorrespondence)?;
                if mapping.coverage == CaseCoverage::Unestablished {
                    case_issues.push(QualificationIssue::CoverageUnestablished);
                }
                if !profile_corresponds(definition, case, mapping) {
                    case_issues.push(QualificationIssue::MechanismProfileMismatch);
                }
                for (i, w) in q.witnesses.iter().enumerate() {
                    if w.claim != *claim.id()
                        || w.case != *case.id()
                        || q.witnesses[..i].contains(w)
                    {
                        continue;
                    }
                    let mut finding = witness(
                        &w.evidence,
                        &mapping.verification,
                        &mapping.profile,
                        &drafts,
                    );
                    // A current source judgment is not sufficient without the
                    // case's declared coverage and mechanism/fixture/oracle links.
                    finding.reasons.extend(case_issues.iter().cloned());
                    witnesses.push(finding);
                }
            }
            if witnesses.is_empty() {
                case_issues.push(QualificationIssue::MissingWitness);
            }
            cases.push(CaseInspection {
                case: case.id().clone(),
                witnesses,
                issues: case_issues,
            });
        }
        claims.push(ClaimInspection {
            claim: claim.id().clone(),
            cases,
            issues,
        });
    }
    let mut app_witness = None;
    let mut applicability = Applicability::Uncertain;
    if let Some(q) = qualification
        && let Some(a) = q.applicability
    {
        let mut w = witness(&a.evidence, &a.verification, &a.profile, &drafts);
        if a.coverage == CaseCoverage::Unestablished {
            w.reasons.push(QualificationIssue::CoverageUnestablished);
        }
        if a.verification.task().acceptance() != definition.context().applicability().trigger() {
            w.reasons.push(QualificationIssue::CriterionMismatch);
        }
        if w.requirements_met() {
            applicability = Applicability::Applicable;
        } else if w.result == Some(AssessmentResult::NotApplicable)
            && w.applicability == Some(Applicability::NotApplicable)
            && w.validity == Some(Validity::Current)
            && w.reasons == [QualificationIssue::NotCurrentPass]
            && drafts.iter().any(|(_, r, _)| {
                r.observation().id() == &a.evidence.observation
                    && matches!(
                        r.observation().kind(),
                        ObservationKind::ScopeInspection(ScopeFinding::Absent)
                    )
            })
        {
            applicability = Applicability::NotApplicable;
        }
        app_witness = Some(w);
    }
    let basis = if let Some(q) = qualification {
        QualificationBasis {
            contract: Some(q.contract.clone()),
            examinations: q
                .examinations
                .iter()
                .map(|e| ExaminationBasis {
                    observation: e.observation.clone(),
                    contract: e.contract.clone(),
                    candidate: e.candidate.cloned(),
                    profile: e.profile.cloned(),
                })
                .collect(),
            witnesses: q.witnesses.to_vec(),
            applicability: q.applicability.cloned(),
        }
    } else {
        QualificationBasis {
            contract: None,
            examinations: Vec::new(),
            witnesses: Vec::new(),
            applicability: None,
        }
    };
    for claim in &claims {
        usage.add(InspectionLimitKind::Findings, 1, limits)?;
        usage.add(InspectionLimitKind::Findings, claim.issues.len(), limits)?;
        for case in &claim.cases {
            usage.add(InspectionLimitKind::Findings, 1, limits)?;
            usage.add(InspectionLimitKind::Findings, case.issues.len(), limits)?;
            for w in &case.witnesses {
                usage.add(InspectionLimitKind::Findings, 1, limits)?;
                usage.add(InspectionLimitKind::Findings, w.reasons.len(), limits)?;
            }
        }
    }
    if let Some(w) = &app_witness {
        usage.add(InspectionLimitKind::Findings, 1, limits)?;
        usage.add(InspectionLimitKind::Findings, w.reasons.len(), limits)?;
    }
    usage.add(
        InspectionLimitKind::Findings,
        definition
            .sources()
            .iter()
            .filter(|s| s.availability() != SourceAvailability::Inspected)
            .count(),
        limits,
    )?;
    // Publication boundary: all validation, cloning, output construction and fallible
    // work is complete. No callbacks/reservations/clones occur after this point.
    for (index, draft, _) in drafts {
        records[index] = draft;
    }
    Ok(MethodInspection {
        definition,
        basis,
        claims,
        applicability,
        app_witness,
        records,
        usage,
    })
}

type Dimensions = (
    Option<Validity>,
    Option<ValidityIssue>,
    Option<ObservationId>,
    bool,
);
type Draft = (usize, EvidenceRecord, Option<Dimensions>);
fn witness(
    reference: &EvidenceReference,
    contract: &CandidateBindingContract,
    profile: &VerificationProfile,
    drafts: &[Draft],
) -> WitnessFinding {
    let mut w = WitnessFinding {
        reference: reference.clone(),
        result: None,
        applicability: None,
        validity: None,
        issue: None,
        compared_to: None,
        reasons: Vec::new(),
    };
    if reference.assessment.is_none() {
        w.reasons
            .push(QualificationIssue::MissingAssessmentReference);
    }
    let Some((_, r, view)) = drafts
        .iter()
        .find(|(_, r, _)| r.observation().id() == &reference.observation)
    else {
        w.reasons.push(QualificationIssue::MissingRecord);
        return w;
    };
    w.result = r.assessment().map(|a| a.result());
    w.applicability = r.assessment().map(|a| a.applicability());
    if reference
        .assessment
        .as_ref()
        .is_some_and(|id| r.assessment().is_none_or(|a| a.id() != id))
    {
        w.reasons.push(QualificationIssue::AssessmentMismatch);
    }
    if r.observation().subject().contract() != contract {
        w.reasons.push(QualificationIssue::CriterionMismatch);
    }
    if r.observation().profile() != profile {
        w.reasons.push(QualificationIssue::ProfileMismatch);
    }
    if let Some((validity, issue, id, pass)) = view {
        w.validity = *validity;
        w.issue = issue.clone();
        w.compared_to = id.clone();
        if !pass {
            w.reasons.push(QualificationIssue::NotCurrentPass);
        }
    } else {
        w.reasons.push(QualificationIssue::MissingExamination);
    }
    w
}
fn profile_corresponds(
    definition: &MethodDefinition,
    case: &QualificationCase,
    mapping: &CaseCorrespondence,
) -> bool {
    let p = &mapping.profile;
    let dependencies_match =
        definition
            .context()
            .dependencies()
            .iter()
            .enumerate()
            .all(|(i, dependency)| {
                [
                    (format!("dependency-{i}-source"), dependency.source()),
                    (format!("dependency-{i}-version"), dependency.version()),
                ]
                .iter()
                .all(|(key, value)| {
                    p.components()
                        .iter()
                        .any(|(name, v)| name == key && v == value)
                })
            });
    dependencies_match
        && p.version() == definition.mechanism().source().version()
        && [
            ("oracle-source", case.oracle().source()),
            ("oracle-version", case.oracle().version()),
            ("fixture-source", case.fixture().source()),
            ("fixture-version", case.fixture().version()),
        ]
        .iter()
        .all(|(n, v)| {
            p.components()
                .iter()
                .any(|(name, value)| name == n && value == v)
        })
}
fn referenced(q: &QualificationRequest<'_>, id: &ObservationId) -> bool {
    q.witnesses.iter().any(|w| &w.evidence.observation == id)
        || q.applicability
            .is_some_and(|a| &a.evidence.observation == id)
}
fn validate(
    definition: &MethodDefinition,
    q: &QualificationRequest<'_>,
    records: &[EvidenceRecord],
) -> Result<(), MethodInspectionError> {
    if definition != &q.contract.definition
        || q.scope != &q.contract.scope
        || q.applicability.is_some_and(|a| &a.scope != q.scope)
    {
        return Err(MethodInspectionError::ContextChanged);
    }
    for (i, r) in records.iter().enumerate() {
        if records[..i]
            .iter()
            .any(|p| p.observation().id() == r.observation().id())
        {
            return Err(MethodInspectionError::IdentityConflict);
        }
    }
    for (i, w) in q.witnesses.iter().enumerate() {
        if !q
            .contract
            .mappings
            .iter()
            .any(|m| m.claim == w.claim && m.case == w.case)
        {
            return Err(MethodInspectionError::InvalidCorrespondence);
        }
        if q.witnesses[..i].iter().any(|p| {
            p.evidence.observation == w.evidence.observation
                && (p.claim != w.claim || p.case != w.case || p.evidence != w.evidence)
        }) {
            return Err(MethodInspectionError::IdentityConflict);
        }
        if q.applicability
            .is_some_and(|a| a.evidence.observation == w.evidence.observation)
        {
            return Err(MethodInspectionError::InvalidCorrespondence);
        }
    }
    for (i, e) in q.examinations.iter().enumerate() {
        if !referenced(q, &e.observation) {
            return Err(MethodInspectionError::InvalidCorrespondence);
        }
        if q.examinations[..i]
            .iter()
            .any(|p| p.observation == e.observation && p != e)
        {
            return Err(MethodInspectionError::ConflictingExamination);
        }
    }
    let mut captures = Vec::new();
    let mut contracts = Vec::new();
    for r in records {
        captures.push(r.observation().subject());
        if let Some(c) = r.assessment().and_then(|a| a.candidate_at_assessment()) {
            captures.push(c);
        }
    }
    for e in q.examinations {
        captures.extend(e.candidate);
        contracts.push(e.contract);
    }
    for mapping in &q.contract.mappings {
        contracts.push(&mapping.verification);
    }
    if let Some(a) = q.applicability {
        contracts.push(&a.verification);
    }
    for (i, c) in captures.iter().enumerate() {
        if captures[..i].iter().any(|p| p.id() == c.id() && p != c) {
            return Err(MethodInspectionError::IdentityConflict);
        }
        contracts.push(c.contract());
    }
    for (i, c) in contracts.iter().enumerate() {
        if contracts[..i]
            .iter()
            .any(|p| p.task().id() == c.task().id() && p.revision() == c.revision() && p != c)
        {
            return Err(MethodInspectionError::IdentityConflict);
        }
    }
    Ok(())
}
fn reserved<T>(n: usize) -> Result<Vec<T>, MethodInspectionError> {
    let mut v = Vec::new();
    v.try_reserve(n)
        .map_err(|_| MethodInspectionError::Allocation)?;
    Ok(v)
}
fn charge_definition(
    u: &mut InspectionUsage,
    d: &MethodDefinition,
    l: InspectionLimits,
) -> Result<(), MethodInspectionError> {
    let v = d.input_usage();
    for (a, b) in [
        (InspectionLimitKind::Cases, DefinitionLimitKind::Cases),
        (
            InspectionLimitKind::References,
            DefinitionLimitKind::References,
        ),
        (
            InspectionLimitKind::MaterialBytes,
            DefinitionLimitKind::MaterialBytes,
        ),
    ] {
        u.add(a, v.get(b), l)?;
    }
    Ok(())
}
fn charge_scope(
    u: &mut InspectionUsage,
    s: &TargetScope,
    l: InspectionLimits,
) -> Result<(), MethodInspectionError> {
    let v = s.input_usage()?;
    u.add(
        InspectionLimitKind::References,
        v.get(DefinitionLimitKind::References),
        l,
    )?;
    u.add(
        InspectionLimitKind::MaterialBytes,
        v.get(DefinitionLimitKind::MaterialBytes),
        l,
    )
}
fn measure(
    definition: &MethodDefinition,
    q: Option<&QualificationRequest<'_>>,
    records: &[EvidenceRecord],
    limits: InspectionLimits,
) -> Result<InspectionUsage, MethodInspectionError> {
    let mut u = InspectionUsage::default();
    u.add(InspectionLimitKind::Records, records.len(), limits)?;
    charge_definition(&mut u, definition, limits)?;
    let minimum_findings = definition
        .claims()
        .len()
        .checked_add(definition.input_usage().get(DefinitionLimitKind::Cases))
        .ok_or(MethodInspectionError::Overflow)?;
    if minimum_findings > limits.get(InspectionLimitKind::Findings) {
        return Err(MethodInspectionError::Limit(InspectionLimitKind::Findings));
    }
    let mut m = Material::new(limits.get(InspectionLimitKind::MaterialBytes));
    m.bytes = u.get(InspectionLimitKind::MaterialBytes);
    for r in records {
        m.record(r)?;
    }
    if let Some(q) = q {
        charge_definition(&mut u, &q.contract.definition, limits)?;
        charge_scope(&mut u, &q.contract.scope, limits)?;
        charge_scope(&mut u, q.scope, limits)?;
        m.add(
            u.get(InspectionLimitKind::MaterialBytes)
                .checked_sub(
                    definition
                        .input_usage()
                        .get(DefinitionLimitKind::MaterialBytes),
                )
                .ok_or(MethodInspectionError::Overflow)?,
        )?;
        u.add(
            InspectionLimitKind::Cases,
            q.contract.mappings.len(),
            limits,
        )?;
        m.source(&q.contract.basis)?;
        for c in &q.contract.mappings {
            m.mapping(c)?;
        }
        u.add(InspectionLimitKind::Assignments, q.witnesses.len(), limits)?;
        for w in q.witnesses {
            m.add(w.claim.as_str().len())?;
            m.add(w.case.as_str().len())?;
            m.reference(&w.evidence)?;
        }
        u.add(InspectionLimitKind::Records, q.examinations.len(), limits)?;
        for e in q.examinations {
            m.observation(&e.observation)?;
            m.binding(e.contract)?;
            if let Some(c) = e.candidate {
                m.capture(c)?;
            }
            if let Some(p) = e.profile {
                m.profile(p)?;
            }
        }
        if let Some(a) = q.applicability {
            let v = a.scope.input_usage()?;
            m.add(v.get(DefinitionLimitKind::MaterialBytes))?;
            m.references = m
                .references
                .checked_add(v.get(DefinitionLimitKind::References))
                .ok_or(MethodInspectionError::Overflow)?;
            m.binding(&a.verification)?;
            m.profile(&a.profile)?;
            m.reference(&a.evidence)?;
        }
    }
    u.0[InspectionLimitKind::MaterialBytes.index()] = m.bytes;
    u.add(InspectionLimitKind::References, m.references, limits)?;
    Ok(u)
}
struct Material {
    bytes: usize,
    limit: usize,
    references: usize,
}
impl Material {
    fn new(limit: usize) -> Self {
        Self {
            bytes: 0,
            limit,
            references: 0,
        }
    }
    fn add(&mut self, count: usize) -> Result<(), MethodInspectionError> {
        self.bytes = self
            .bytes
            .checked_add(count)
            .ok_or(MethodInspectionError::Overflow)?;
        if self.bytes > self.limit {
            return Err(MethodInspectionError::Limit(
                InspectionLimitKind::MaterialBytes,
            ));
        }
        Ok(())
    }
    fn observation(&mut self, id: &ObservationId) -> Result<(), MethodInspectionError> {
        self.add(id.scope().len())
    }
    fn requirement(&mut self, c: &CandidateRequirement) -> Result<(), MethodInspectionError> {
        for p in [
            c.repository().root(),
            c.repository().git_dir(),
            c.repository().common_dir(),
        ] {
            self.add(p.as_os_str().as_encoded_bytes().len())?;
        }
        self.add(c.commit().as_str().len())
    }
    fn binding(&mut self, c: &CandidateBindingContract) -> Result<(), MethodInspectionError> {
        self.add(c.task().id().as_str().len())?;
        self.add(c.task().objective().len())?;
        self.add(c.task().acceptance().len())?;
        self.requirement(c.candidate())
    }
    fn capture(&mut self, c: &CandidateObservation) -> Result<(), MethodInspectionError> {
        self.observation(c.id())?;
        self.binding(c.contract())?;
        if let Some(r) = c.state().repository() {
            for p in [r.root(), r.git_dir(), r.common_dir()] {
                self.add(p.as_os_str().as_encoded_bytes().len())?;
            }
        }
        match c.state().head() {
            Head::Attached { branch, commit } => {
                self.add(branch.len())?;
                self.add(commit.as_str().len())?;
            }
            Head::Detached { commit } => self.add(commit.as_str().len())?,
            Head::Unborn { branch } => self.add(branch.len())?,
            Head::Unknown => {}
        }
        for e in c.state().entries() {
            self.add(e.path().len())?;
        }
        if let Some(b) = c.basis() {
            self.add(b.profile().len())?;
            self.add(b.material().len())?;
        }
        Ok(())
    }
    fn profile(&mut self, p: &VerificationProfile) -> Result<(), MethodInspectionError> {
        for s in [
            p.provenance().mechanism(),
            p.provenance().inputs(),
            p.provenance().configuration(),
            p.version(),
            p.environment(),
        ] {
            self.add(s.len())?;
        }
        self.references = self
            .references
            .checked_add(p.components().len())
            .ok_or(MethodInspectionError::Overflow)?;
        for (n, v) in p.components() {
            self.add(n.len())?;
            self.add(v.len())?;
        }
        Ok(())
    }
    fn record(&mut self, r: &EvidenceRecord) -> Result<(), MethodInspectionError> {
        let o = r.observation();
        self.observation(o.id())?;
        self.capture(o.subject())?;
        self.profile(o.profile())?;
        self.add(o.detail().len())?;
        if let crate::unified::ObservationKind::Execution(outcome) = o.kind()
            && let Some(oracle) = outcome.oracle()
        {
            self.add(oracle.reasoning().len())?;
        }
        if let Some(a) = r.assessment() {
            self.add(a.id().as_str().len())?;
            self.add(a.rationale().len())?;
            if let Some(c) = a.candidate_at_assessment() {
                self.capture(c)?;
            }
            if let Some(p) = a.profile_at_assessment() {
                self.profile(p)?;
            }
            if let Some(id) = a.compared_to() {
                self.observation(id)?;
            }
        }
        // Existing ContextCheck is opaque. Charge its maximum ObservationId scope
        // even before invalidation; inspect can add one. Fixed enum/sequence sizes
        // and allocator overhead are outside this logical material budget.
        self.add(cortex_core::MAX_TASK_ID_BYTES)?;
        if let Some(reason) = r.invalidation_reason() {
            self.add(reason.len())?;
        }
        Ok(())
    }
}

impl Material {
    fn source(&mut self, s: &SourceReference) -> Result<(), MethodInspectionError> {
        self.references = self
            .references
            .checked_add(1)
            .ok_or(MethodInspectionError::Overflow)?;
        self.add(s.source().len())?;
        self.add(s.version().len())?;
        match s.origin() {
            SourceOrigin::OperatorReported { author } => self.add(author.len()),
            SourceOrigin::ObservationReference(id) => self.observation(id),
        }
    }
    fn reference(&mut self, r: &EvidenceReference) -> Result<(), MethodInspectionError> {
        self.observation(&r.observation)?;
        if let Some(a) = &r.assessment {
            self.add(a.as_str().len())?;
        }
        Ok(())
    }
    fn mapping(&mut self, c: &CaseCorrespondence) -> Result<(), MethodInspectionError> {
        self.add(c.claim.as_str().len())?;
        self.add(c.case.as_str().len())?;
        self.binding(&c.verification)?;
        self.profile(&c.profile)?;
        self.source(&c.oracle)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn checked_accounting_overflow_refuses() {
        let mut u = InspectionUsage([usize::MAX; 6]);
        assert_eq!(
            u.add(
                InspectionLimitKind::Records,
                1,
                InspectionLimits::new(1, 1, 1, 1, 1, 1).unwrap()
            ),
            Err(MethodInspectionError::Overflow)
        );
    }
}
