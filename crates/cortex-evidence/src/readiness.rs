//! Advisory Evidence support for one exact G1 dependency edge.
//!
//! Caller-declared criterion correspondence is data, not authenticated provenance.
//! This module reuses unified Evidence inspection; it never assesses a record,
//! reviews/releases a G1 edge, authenticates execution or grants permission.
//! Inspection can retain sticky invalidation on caller-owned records. Malformed
//! requests return an error without mutation; admitted negative findings retain
//! legitimate inspection-driven invalidation. No I/O, clock, dispatcher or store.
//!
//! Bounds charge logical input occurrences/material, not total allocator/RSS use.
//! Ordinary allocation panics/aborts are outside the typed-error atomicity claim.

use crate::unified::{
    Applicability, AssessmentId, AssessmentResult, EvidenceRecord, ValidationContext, Validity,
    ValidityIssue, VerificationProfile,
};
use cortex_core::{
    binding::{
        BindingDecision, BindingError, BindingInsufficiency, BindingMismatch, BindingStaleness,
        CandidateBindingContract, CandidateObservation, CandidateRequirement, ContractRevision,
        Head, ObservationId, bind_candidate,
    },
    parallel::{
        ArtifactReference, ArtifactRequirement, EdgeId, ExecutionAuthority, GraphReference,
        MissionScope, PredicateId, ReadinessError, ReadinessProjection,
    },
    temporal::{SourceOrigin, SourceReference},
};
use std::{error::Error, fmt};

macro_rules! redacted {
    ($($ty:ty),+ $(,)?) => {$ (
        impl fmt::Debug for $ty {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(concat!(stringify!($ty), "([redacted])"))
            }
        }
    )+};
}

/// Logical admission budget; never an operational resource grant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupportLimitKind {
    /// Record-pool and examination counts, each bounded independently.
    Records,
    /// Witness-binding occurrences, including redundant duplicates.
    Bindings,
    /// Required-predicate and correspondence counts, each bounded independently.
    Predicates,
    /// Charged nested text/path/comparison material occurrences.
    MaterialBytes,
}

/// Malformed request or resource refusal; all caller records remain unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SupportInputError {
    /// A supplied bound is zero or exceeds isize::MAX.
    InvalidLimits,
    /// A named graph/edge reference is unavailable.
    Core(ReadinessError),
    /// Existing binding validation refused a context.
    Binding(BindingError),
    /// The frozen semantic edge/mission/artifact scope changed.
    ContextChanged,
    /// Same task/revision or support revision has conflicting contents.
    ContractAlias,
    /// A new correspondence revision requires fresh inspection.
    CorrespondenceChanged,
    /// Artifact or examination declaration contradicts its reference.
    IncompatibleReference,
    /// Record-pool identities are ambiguous, even for equal duplicate entries.
    DuplicateRecordIdentity,
    /// One candidate capture identity has conflicting contents.
    IdentityConflict,
    /// More than one unequal examination context names the same report.
    ConflictingExamination,
    /// A binding or correspondence names an undeclared predicate.
    UnknownPredicate,
    /// More than one correspondence names the same predicate.
    DuplicateCorrespondence,
    /// A finite logical input budget was exceeded, or its sum overflowed.
    Limit(SupportLimitKind),
    /// Fallible storage reservation failed before publication.
    Allocation,
}
impl fmt::Display for SupportInputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidLimits => "support limits must be positive and finite",
            Self::Core(_) => "support graph reference is unavailable",
            Self::Binding(_) => "support binding input is invalid",
            Self::ContextChanged => "support scope changed; inspect the current context",
            Self::ContractAlias => "support contract revision has conflicting contents",
            Self::CorrespondenceChanged => "correspondence revision changed; inspect again",
            Self::IncompatibleReference => "support reference is incompatible",
            Self::DuplicateRecordIdentity => "support record identity is ambiguous",
            Self::IdentityConflict => "candidate capture identity has conflicting contents",
            Self::ConflictingExamination => "source examination contexts conflict",
            Self::UnknownPredicate => "support predicate is not declared by the edge",
            Self::DuplicateCorrespondence => "predicate correspondence is ambiguous",
            Self::Limit(_) => "support input exceeds a logical budget",
            Self::Allocation => "support storage reservation failed",
        })
    }
}
impl Error for SupportInputError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Core(e) => Some(e),
            Self::Binding(e) => Some(e),
            _ => None,
        }
    }
}

/// Explicit positive caller-selected limits; no implicit production defaults.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SupportLimits {
    records: usize,
    bindings: usize,
    predicates: usize,
    material: usize,
}
impl SupportLimits {
    /// Validate finite collection and material bounds without allocating.
    pub fn new(
        records: usize,
        bindings: usize,
        predicates: usize,
        material: usize,
    ) -> Result<Self, SupportInputError> {
        if [records, bindings, predicates, material]
            .iter()
            .any(|n| *n == 0 || *n > isize::MAX as usize)
        {
            return Err(SupportInputError::InvalidLimits);
        }
        Ok(Self {
            records,
            bindings,
            predicates,
            material,
        })
    }
    /// Read one selected bound.
    pub fn get(self, kind: SupportLimitKind) -> usize {
        match kind {
            SupportLimitKind::Records => self.records,
            SupportLimitKind::Bindings => self.bindings,
            SupportLimitKind::Predicates => self.predicates,
            SupportLimitKind::MaterialBytes => self.material,
        }
    }
}

/// Charged logical input use, not an allocation or host-memory measurement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SupportUsage {
    records: usize,
    bindings: usize,
    predicates: usize,
    material: usize,
}
impl SupportUsage {
    /// Read the admitted usage; paired collection budgets use the greater count.
    pub fn get(self, kind: SupportLimitKind) -> usize {
        match kind {
            SupportLimitKind::Records => self.records,
            SupportLimitKind::Bindings => self.bindings,
            SupportLimitKind::Predicates => self.predicates,
            SupportLimitKind::MaterialBytes => self.material,
        }
    }
}

/// Frozen semantic edge basis plus an informational notification cursor.
#[derive(Clone, PartialEq, Eq)]
pub struct EdgeScopeSnapshot {
    graph: GraphReference,
    edge: EdgeId,
    producer: MissionScope,
    consumer: MissionScope,
    requirement: ArtifactRequirement,
    cursor: usize,
}
impl EdgeScopeSnapshot {
    /// Copy the existing public G1 scope; no recapture or authority is implied.
    pub fn capture(
        projection: &ReadinessProjection,
        edge: &EdgeId,
    ) -> Result<Self, SupportInputError> {
        let declaration = projection
            .graph()
            .edges()
            .iter()
            .find(|e| e.id() == edge)
            .ok_or(SupportInputError::Core(ReadinessError::MissingReference))?;
        Ok(Self {
            graph: projection.graph().reference().clone(),
            edge: edge.clone(),
            producer: projection
                .scope(declaration.producer())
                .map_err(SupportInputError::Core)?,
            consumer: projection
                .scope(declaration.consumer())
                .map_err(SupportInputError::Core)?,
            requirement: declaration.requirement().clone(),
            cursor: projection.notifications().len(),
        })
    }
    /// Exact graph identity and revision.
    pub fn graph(&self) -> &GraphReference {
        &self.graph
    }
    /// Exact consumer edge.
    pub fn edge(&self) -> &EdgeId {
        &self.edge
    }
    /// Frozen producer context and generation.
    pub fn producer(&self) -> &MissionScope {
        &self.producer
    }
    /// Frozen consumer context and generation.
    pub fn consumer(&self) -> &MissionScope {
        &self.consumer
    }
    /// Exact artifact/kind/level/predicate requirement.
    pub fn requirement(&self) -> &ArtifactRequirement {
        &self.requirement
    }
    /// Historical cursor only; unrelated notifications do not stale this scope.
    pub fn notification_cursor(&self) -> usize {
        self.cursor
    }
    fn same_scope(&self, other: &Self) -> bool {
        self.graph == other.graph
            && self.edge == other.edge
            && self.producer == other.producer
            && self.consumer == other.consumer
            && self.requirement == other.requirement
    }
}

/// Declared coverage, never machine-proved semantic entailment or qualification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CriterionCoverage {
    /// Caller maps the complete frozen single verification criterion to the predicate.
    ExactSingleCriterion,
    /// Required coverage has not been established; it cannot support the predicate.
    Unestablished,
}

/// Edge-local mapping to an existing full verification contract and profile.
#[derive(Clone, PartialEq, Eq)]
pub struct PredicateCorrespondence {
    predicate: PredicateId,
    verification: CandidateBindingContract,
    profile: VerificationProfile,
    coverage: CriterionCoverage,
}
impl PredicateCorrespondence {
    /// Retain validated inputs; the enclosing contract checks predicate/artifact membership.
    pub fn new(
        predicate: PredicateId,
        verification: CandidateBindingContract,
        profile: VerificationProfile,
        coverage: CriterionCoverage,
    ) -> Self {
        Self {
            predicate,
            verification,
            profile,
            coverage,
        }
    }
    /// Exact required predicate.
    pub fn predicate(&self) -> &PredicateId {
        &self.predicate
    }
    /// Entire verification criterion, revision and candidate requirement.
    pub fn verification(&self) -> &CandidateBindingContract {
        &self.verification
    }
    /// Entire expected mechanism/version/environment/profile.
    pub fn profile(&self) -> &VerificationProfile {
        &self.profile
    }
    /// Caller-declared coverage; not independently authenticated.
    pub fn coverage(&self) -> CriterionCoverage {
        self.coverage
    }
}

/// Immutable, versioned support interpretation for one exact edge scope.
#[derive(Clone, PartialEq, Eq)]
pub struct EdgeEvidenceContract {
    scope: EdgeScopeSnapshot,
    revision: ContractRevision,
    artifact: CandidateRequirement,
    correspondences: Vec<PredicateCorrespondence>,
    basis: SourceReference,
}
impl EdgeEvidenceContract {
    /// Validate declared correspondence and finite material before accepting it.
    /// Missing predicate mappings remain representable and unsupported at inspection.
    pub fn new(
        scope: EdgeScopeSnapshot,
        revision: ContractRevision,
        artifact: CandidateRequirement,
        mut correspondences: Vec<PredicateCorrespondence>,
        basis: SourceReference,
        limits: SupportLimits,
    ) -> Result<Self, SupportInputError> {
        check_count(
            correspondences
                .len()
                .max(scope.requirement.predicates().len()),
            limits.predicates,
            SupportLimitKind::Predicates,
        )?;
        if artifact.commit() != scope.requirement.artifact().candidate() {
            return Err(SupportInputError::IncompatibleReference);
        }
        correspondences.sort_by(|a, b| a.predicate.cmp(&b.predicate));
        for (i, c) in correspondences.iter().enumerate() {
            if !scope.requirement.predicates().contains(&c.predicate) {
                return Err(SupportInputError::UnknownPredicate);
            }
            if c.verification.candidate() != &artifact {
                return Err(SupportInputError::IncompatibleReference);
            }
            if i > 0 && correspondences[i - 1].predicate == c.predicate {
                return Err(SupportInputError::DuplicateCorrespondence);
            }
        }
        let value = Self {
            scope,
            revision,
            artifact,
            correspondences,
            basis,
        };
        let mut material = Material::new(limits.material);
        material.contract(&value)?;
        // Reuse the same alias checks as inspection, without inventing a global registry.
        validate_identities(&value, &[], &SupportRequest::new(None, None, &[], &[]))?;
        Ok(value)
    }
    /// Original edge scope and historical cursor.
    pub fn scope(&self) -> &EdgeScopeSnapshot {
        &self.scope
    }
    /// Exact edge-local interpretation revision.
    pub fn revision(&self) -> ContractRevision {
        self.revision
    }
    /// Artifact's explicit repository/worktree requirement, distinct from either mission.
    pub fn artifact_requirement(&self) -> &CandidateRequirement {
        &self.artifact
    }
    /// Ordered edge-local correspondences, not a method registry.
    pub fn correspondences(&self) -> &[PredicateCorrespondence] {
        &self.correspondences
    }
    /// Caller-declared original mapping provenance, not approval.
    pub fn basis(&self) -> &SourceReference {
        &self.basis
    }
    fn same_definition(&self, other: &Self) -> bool {
        self.artifact == other.artifact
            && self.correspondences == other.correspondences
            && self.basis == other.basis
    }
}

/// Compound report/optional expected assessment reference; no global uniqueness claim.
#[derive(Clone, PartialEq, Eq)]
pub struct EvidenceReference {
    observation: ObservationId,
    assessment: Option<AssessmentId>,
}
impl EvidenceReference {
    /// Name exactly the expected judgment; None never silently selects an assessment.
    pub fn new(observation: ObservationId, assessment: Option<AssessmentId>) -> Self {
        Self {
            observation,
            assessment,
        }
    }
    /// Verification report identity, not its candidate capture.
    pub fn observation(&self) -> &ObservationId {
        &self.observation
    }
    /// Exact expected terminal judgment within that report.
    pub fn assessment(&self) -> Option<&AssessmentId> {
        self.assessment.as_ref()
    }
}

/// One explicitly required witness assignment. Multiple assignments are conjunctive.
#[derive(Clone, PartialEq, Eq)]
pub struct WitnessBinding {
    predicate: PredicateId,
    evidence: EvidenceReference,
    artifact: ArtifactReference,
}
impl WitnessBinding {
    /// Retain declared association; exact correspondence is evaluated at inspection.
    pub fn new(
        predicate: PredicateId,
        evidence: EvidenceReference,
        artifact: ArtifactReference,
    ) -> Self {
        Self {
            predicate,
            evidence,
            artifact,
        }
    }
    /// Required predicate.
    pub fn predicate(&self) -> &PredicateId {
        &self.predicate
    }
    /// Exact compound Evidence reference.
    pub fn evidence(&self) -> &EvidenceReference {
        &self.evidence
    }
    /// Declared artifact/revision/candidate, not authenticated capture provenance.
    pub fn artifact(&self) -> &ArtifactReference {
        &self.artifact
    }
}

/// Legitimate source examination context, separate from the target predicate mapping.
#[derive(Clone, PartialEq, Eq)]
pub struct RecordExamination<'a> {
    observation: ObservationId,
    contract: &'a CandidateBindingContract,
    candidate: Option<&'a CandidateObservation>,
    profile: Option<&'a VerificationProfile>,
}
impl<'a> RecordExamination<'a> {
    /// Retain explicit borrowed current context. Absence is never self-filled from a record.
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
    /// Source verification report being examined.
    pub fn observation(&self) -> &ObservationId {
        &self.observation
    }
    /// Expected source contract, not a substituted broader predicate criterion.
    pub fn contract(&self) -> &CandidateBindingContract {
        self.contract
    }
    /// Supplied current capture; this API does not perform it.
    pub fn candidate(&self) -> Option<&CandidateObservation> {
        self.candidate
    }
    /// Supplied expected source profile.
    pub fn profile(&self) -> Option<&VerificationProfile> {
        self.profile
    }
}

/// Bounded borrowed request; no roles, approvals, effect callbacks or executors.
#[derive(Clone, PartialEq, Eq)]
pub struct SupportRequest<'a> {
    producer: Option<&'a CandidateObservation>,
    consumer: Option<&'a CandidateObservation>,
    examinations: &'a [RecordExamination<'a>],
    bindings: &'a [WitnessBinding],
}
impl<'a> SupportRequest<'a> {
    /// Supply independently obtained current mission captures and explicit source contexts.
    pub fn new(
        producer: Option<&'a CandidateObservation>,
        consumer: Option<&'a CandidateObservation>,
        examinations: &'a [RecordExamination<'a>],
        bindings: &'a [WitnessBinding],
    ) -> Self {
        Self {
            producer,
            consumer,
            examinations,
            bindings,
        }
    }
}

/// Owned requested source context, not a reconstructed historical invalidation log.
#[derive(Clone, PartialEq, Eq)]
pub struct ExaminationBasis {
    observation: ObservationId,
    contract: CandidateBindingContract,
    candidate: Option<CandidateObservation>,
    profile: Option<VerificationProfile>,
}
impl ExaminationBasis {
    /// Exact report reference.
    pub fn observation(&self) -> &ObservationId {
        &self.observation
    }
    /// Full expected source contract.
    pub fn contract(&self) -> &CandidateBindingContract {
        &self.contract
    }
    /// Current capture supplied in this request, distinct from first invalidation.
    pub fn candidate(&self) -> Option<&CandidateObservation> {
        self.candidate.as_ref()
    }
    /// Expected source profile supplied in this request.
    pub fn profile(&self) -> Option<&VerificationProfile> {
        self.profile.as_ref()
    }
}

/// Owned historical decision basis. Recheck scope AND reinspect source context before reuse.
#[derive(Clone, PartialEq, Eq)]
pub struct SupportBasis {
    contract: EdgeEvidenceContract,
    producer: Option<CandidateObservation>,
    consumer: Option<CandidateObservation>,
    examinations: Vec<ExaminationBasis>,
    bindings: Vec<WitnessBinding>,
}
impl SupportBasis {
    /// Frozen edge/correspondence definition used for this decision.
    pub fn contract(&self) -> &EdgeEvidenceContract {
        &self.contract
    }
    /// Requested producer capture, not a freshness guarantee.
    pub fn producer(&self) -> Option<&CandidateObservation> {
        self.producer.as_ref()
    }
    /// Requested consumer capture, not a freshness guarantee.
    pub fn consumer(&self) -> Option<&CandidateObservation> {
        self.consumer.as_ref()
    }
    /// Explicit source examinations; missing entries remain missing.
    pub fn examinations(&self) -> &[ExaminationBasis] {
        &self.examinations
    }
    /// Original witness assignments, including redundant input occurrences.
    pub fn bindings(&self) -> &[WitnessBinding] {
        &self.bindings
    }
    /// Structural applicability only. Does not recapture or establish current Evidence.
    /// Retain this basis to detect aliases; construction alone has no cross-call ledger.
    pub fn check_scope(
        &self,
        projection: &ReadinessProjection,
        current: &EdgeEvidenceContract,
    ) -> Result<(), SupportInputError> {
        let live = EdgeScopeSnapshot::capture(projection, &self.contract.scope.edge)?;
        if !self.contract.scope.same_scope(&live) || !current.scope.same_scope(&live) {
            return Err(SupportInputError::ContextChanged);
        }
        if self.contract.revision != current.revision {
            return Err(SupportInputError::CorrespondenceChanged);
        }
        if !self.contract.same_definition(current) {
            return Err(SupportInputError::ContractAlias);
        }
        Ok(())
    }
}

/// Evidence-only completeness; not an Evidence result, readiness level or release.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidencePart {
    /// Every declared required predicate and assigned witness is supported.
    AllRequiredPredicatesSupported,
    /// At least one required predicate lacks sufficient support.
    Incomplete,
}
/// Predicate support relative to caller-declared exact correspondence, never authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupportDisposition {
    /// Existing current Evidence supports the exact declared criterion mapping.
    SupportedWithinDeclaredContract,
    /// Required support is missing, incompatible, stale or insufficient.
    NotSupported,
}
/// Existing Core binding cause for an unresolved current mission context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContextFinding {
    /// Existing binding reports insufficient source material.
    Insufficient(BindingInsufficiency),
    /// Existing binding reports known incompatibility.
    Incompatible(BindingMismatch),
    /// Existing binding reports staleness.
    Stale(BindingStaleness),
}
/// Reason for one required witness not supplying support; original dimensions remain inspectable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WitnessIssue {
    /// Declared artifact/revision/candidate does not match.
    ArtifactMismatch,
    /// Named record absent.
    MissingRecord,
    /// Judgment reference was omitted.
    MissingAssessmentReference,
    /// Exact named judgment is absent or different.
    AssessmentMismatch,
    /// No explicit legitimate source examination was supplied.
    MissingExamination,
    /// Full verification task/revision/criterion differs from required interpretation.
    CriterionMismatch,
    /// Complete profile differs from required interpretation.
    ProfileMismatch,
    /// The existing view is not an applicable CURRENT explicitly assessed PASS.
    NotCurrentPass,
}
/// Redacted reason with a bounded input ordinal, never private identifiers or rationale.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SupportReason {
    /// Producer original-to-current correspondence is unresolved.
    ProducerContext(ContextFinding),
    /// Consumer original-to-current correspondence is unresolved.
    ConsumerContext(ContextFinding),
    /// Required predicate has no declared interpretation.
    MissingCorrespondence,
    /// Declared coverage is unestablished.
    CoverageUnestablished,
    /// No required witnesses were assigned.
    MissingWitness,
    /// An assigned witness failed an exact predicate.
    Witness {
        /// Ordinal in the original bounded binding input.
        binding: usize,
        /// Redacted issue; retained record provides explicit private detail.
        issue: WitnessIssue,
    },
}
/// Per-predicate conditional finding; no Evidence judgment or coordinator attestation.
#[derive(Clone, PartialEq, Eq)]
pub struct PredicateFinding {
    predicate: PredicateId,
    reasons: Vec<SupportReason>,
    witnesses: Vec<EvidenceReference>,
}
impl PredicateFinding {
    /// Exact predicate, scoped by the retained edge contract.
    pub fn predicate(&self) -> &PredicateId {
        &self.predicate
    }
    /// Evidence support only, not edge acceptance or execution eligibility.
    pub fn disposition(&self) -> SupportDisposition {
        if self.reasons.is_empty() {
            SupportDisposition::SupportedWithinDeclaredContract
        } else {
            SupportDisposition::NotSupported
        }
    }
    /// Deterministic redacted causes.
    pub fn reasons(&self) -> &[SupportReason] {
        &self.reasons
    }
    /// Compound references for distinct required witness bindings.
    pub fn witnesses(&self) -> &[EvidenceReference] {
        &self.witnesses
    }
}

#[derive(Clone, PartialEq, Eq)]
struct Inspection {
    validity: Option<Validity>,
    issue: Option<ValidityIssue>,
    compared_to: Option<ObservationId>,
    current_pass: bool,
}
/// Borrowed committed original and a snapshot of existing EvidenceView dimensions.
#[derive(PartialEq, Eq)]
pub struct InspectedRecord<'e> {
    record: &'e EvidenceRecord,
    inspection: Option<Inspection>,
}
impl InspectedRecord<'_> {
    /// Original report and terminal assessment remain accessible through explicit getters.
    pub fn record(&self) -> &EvidenceRecord {
        self.record
    }
    /// Whether a source context was supplied and existing inspect was called.
    pub fn was_inspected(&self) -> bool {
        self.inspection.is_some()
    }
    /// Historical result; None means unassessed, not NOT_RUN.
    pub fn result(&self) -> Option<AssessmentResult> {
        self.record.assessment().map(|a| a.result())
    }
    /// Historical applicability, independent of contextual validity.
    pub fn applicability(&self) -> Option<Applicability> {
        self.record.assessment().map(|a| a.applicability())
    }
    /// Existing view validity; None is unestablished, not CURRENT.
    pub fn validity(&self) -> Option<Validity> {
        self.inspection.as_ref().and_then(|v| v.validity)
    }
    /// Existing precise contextual cause, if inspection was possible.
    pub fn validity_issue(&self) -> Option<&ValidityIssue> {
        self.inspection.as_ref().and_then(|v| v.issue.as_ref())
    }
    /// Existing current/first-sticky comparison identity; may differ from this request's capture.
    pub fn compared_to(&self) -> Option<&ObservationId> {
        self.inspection
            .as_ref()
            .and_then(|v| v.compared_to.as_ref())
    }
}
/// Complete immutable support findings borrowing committed records; no G1 input conversion.
#[derive(PartialEq, Eq)]
pub struct SupportReport<'e> {
    basis: SupportBasis,
    findings: Vec<PredicateFinding>,
    records: Vec<InspectedRecord<'e>>,
    usage: SupportUsage,
}
impl SupportReport<'_> {
    /// Retained exact interpretation/context; callers must revalidate before current use.
    pub fn basis(&self) -> &SupportBasis {
        &self.basis
    }
    /// All declared Evidence predicates, without conferring a readiness level.
    pub fn evidence_part(&self) -> EvidencePart {
        if self
            .findings
            .iter()
            .all(|f| f.disposition() == SupportDisposition::SupportedWithinDeclaredContract)
        {
            EvidencePart::AllRequiredPredicatesSupported
        } else {
            EvidencePart::Incomplete
        }
    }
    /// Ordered required-predicate findings.
    pub fn findings(&self) -> &[PredicateFinding] {
        &self.findings
    }
    /// Participating originals only, in record-pool order; unrelated records are not inspected.
    pub fn inspected_records(&self) -> &[InspectedRecord<'_>] {
        &self.records
    }
    /// Logical admission accounting, not actual allocation or CPU cost.
    pub fn input_usage(&self) -> SupportUsage {
        self.usage
    }
    /// Always unavailable; support is never operational permission or dispatch.
    pub fn authority(&self) -> ExecutionAuthority {
        ExecutionAuthority::Unavailable
    }
}
redacted!(
    EdgeScopeSnapshot,
    PredicateCorrespondence,
    EdgeEvidenceContract,
    EvidenceReference,
    WitnessBinding,
    RecordExamination<'_>,
    SupportRequest<'_>,
    ExaminationBasis,
    SupportBasis,
    PredicateFinding,
    InspectedRecord<'_>,
    SupportReport<'_>
);

/// Inspect the Evidence portion of one edge, without modifying the G1 projection.
///
/// All request/identity/budget errors leave every supplied record unchanged.
/// Valid admitted inspection publishes legitimate sticky invalidations even when
/// the finding is negative. Reports and terminal judgments are never replaced.
/// Missing source context is not filled from the record. Every distinct assigned
/// witness is required; no favorable-record selection or quorum is inferred.
///
/// # Errors
/// Returns a typed malformed-reference, alias, scope or logical-resource refusal.
/// Ordinary allocation panic/abort is outside this Result atomicity guarantee.
/// Source capture truth, independently qualified oracle coverage, coordinator
/// acceptance and operational permission are outside this advisory function.
pub fn inspect_support<'e>(
    projection: &ReadinessProjection,
    contract: &EdgeEvidenceContract,
    records: &'e mut [EvidenceRecord],
    request: &SupportRequest<'_>,
    limits: SupportLimits,
) -> Result<SupportReport<'e>, SupportInputError> {
    let usage = measure(contract, records, request, limits)?;
    let current = EdgeScopeSnapshot::capture(projection, &contract.scope.edge)?;
    if !contract.scope.same_scope(&current) {
        return Err(SupportInputError::ContextChanged);
    }
    for binding in request.bindings {
        if !contract
            .scope
            .requirement
            .predicates()
            .contains(&binding.predicate)
        {
            return Err(SupportInputError::UnknownPredicate);
        }
    }
    validate_identities(contract, records, request)?;
    let producer = context_finding(&contract.scope.producer, request.producer)?;
    let consumer = context_finding(&contract.scope.consumer, request.consumer)?;
    let mut drafts = reserved(records.len())?;
    for (index, original) in records.iter().enumerate() {
        if !request
            .bindings
            .iter()
            .any(|b| b.evidence.observation == *original.observation().id())
        {
            continue;
        }
        let mut draft = original.clone();
        let inspection = request
            .examinations
            .iter()
            .find(|e| &e.observation == original.observation().id())
            .map(|e| {
                let view =
                    draft.inspect(&ValidationContext::new(e.contract, e.candidate, e.profile));
                Inspection {
                    validity: view.validity(),
                    issue: view.validity_issue().cloned(),
                    compared_to: view.compared_to().cloned(),
                    current_pass: view.current_pass(),
                }
            });
        drafts.push((index, draft, inspection));
    }
    let mut findings = reserved(contract.scope.requirement.predicates().len())?;
    for predicate in contract.scope.requirement.predicates() {
        let mut reasons = reserved(5)?;
        let mut witnesses = reserved(request.bindings.len())?;
        if let Some(c) = &producer {
            reasons.push(SupportReason::ProducerContext(c.clone()));
        }
        if let Some(c) = &consumer {
            reasons.push(SupportReason::ConsumerContext(c.clone()));
        }
        let correspondence = contract
            .correspondences
            .iter()
            .find(|c| &c.predicate == predicate);
        match correspondence {
            None => reasons.push(SupportReason::MissingCorrespondence),
            Some(c) if c.coverage == CriterionCoverage::Unestablished => {
                reasons.push(SupportReason::CoverageUnestablished)
            }
            _ => {}
        }
        // Each witness can contribute at most eight static reasons. Reserve before commit.
        reasons
            .try_reserve(
                request
                    .bindings
                    .len()
                    .checked_mul(8)
                    .ok_or(SupportInputError::Limit(SupportLimitKind::Bindings))?,
            )
            .map_err(|_| SupportInputError::Allocation)?;
        for (i, b) in request.bindings.iter().enumerate() {
            if &b.predicate != predicate || request.bindings[..i].contains(b) {
                continue;
            }
            witnesses.push(b.evidence.clone());
            let mut issue = |issue| reasons.push(SupportReason::Witness { binding: i, issue });
            if b.artifact != *contract.scope.requirement.artifact() {
                issue(WitnessIssue::ArtifactMismatch);
            }
            let Some((_, record, inspection)) = drafts
                .iter()
                .find(|(_, r, _)| r.observation().id() == &b.evidence.observation)
            else {
                issue(WitnessIssue::MissingRecord);
                continue;
            };
            match &b.evidence.assessment {
                None => issue(WitnessIssue::MissingAssessmentReference),
                Some(id) if record.assessment().is_none_or(|a| a.id() != id) => {
                    issue(WitnessIssue::AssessmentMismatch)
                }
                _ => {}
            }
            if inspection.is_none() {
                issue(WitnessIssue::MissingExamination);
            }
            if let Some(c) = correspondence {
                if record.observation().subject().contract() != &c.verification {
                    issue(WitnessIssue::CriterionMismatch);
                }
                if record.observation().profile() != &c.profile {
                    issue(WitnessIssue::ProfileMismatch);
                }
            }
            // Unified assessment construction already guards completed execution,
            // checks and matching conclusive oracle. Never duplicate that verdict rule.
            if !inspection.as_ref().is_some_and(|v| v.current_pass) {
                issue(WitnessIssue::NotCurrentPass);
            }
        }
        if witnesses.is_empty() {
            reasons.push(SupportReason::MissingWitness);
        }
        findings.push(PredicateFinding {
            predicate: predicate.clone(),
            reasons,
            witnesses,
        });
    }
    let mut examinations = reserved(request.examinations.len())?;
    for e in request.examinations {
        examinations.push(ExaminationBasis {
            observation: e.observation.clone(),
            contract: e.contract.clone(),
            candidate: e.candidate.cloned(),
            profile: e.profile.cloned(),
        });
    }
    let basis = SupportBasis {
        contract: contract.clone(),
        producer: request.producer.cloned(),
        consumer: request.consumer.cloned(),
        examinations,
        bindings: request.bindings.to_vec(),
    };
    let mut inspected = reserved(drafts.len())?;
    let mut dimensions = reserved(drafts.len())?;
    // Last fallible operation is above. No callbacks, validation, cloning or
    // reservations after this publication boundary. All assignments are infallible.
    for (index, draft, inspection) in drafts {
        records[index] = draft;
        dimensions.push((index, inspection));
    }
    for (index, inspection) in dimensions {
        inspected.push(InspectedRecord {
            record: &records[index],
            inspection,
        });
    }
    Ok(SupportReport {
        basis,
        findings,
        records: inspected,
        usage,
    })
}

fn reserved<T>(count: usize) -> Result<Vec<T>, SupportInputError> {
    let mut value = Vec::new();
    value
        .try_reserve(count)
        .map_err(|_| SupportInputError::Allocation)?;
    Ok(value)
}
fn check_count(
    actual: usize,
    limit: usize,
    kind: SupportLimitKind,
) -> Result<(), SupportInputError> {
    if actual > limit {
        Err(SupportInputError::Limit(kind))
    } else {
        Ok(())
    }
}
fn context_finding(
    scope: &MissionScope,
    current: Option<&CandidateObservation>,
) -> Result<Option<ContextFinding>, SupportInputError> {
    Ok(
        match bind_candidate(
            scope.context().contract(),
            scope.context().capture(),
            current,
        )
        .map_err(SupportInputError::Binding)?
        {
            BindingDecision::Compatible(_) => None,
            BindingDecision::Incompatible(c) => Some(ContextFinding::Incompatible(c)),
            BindingDecision::Stale(c) => Some(ContextFinding::Stale(c)),
            BindingDecision::Insufficient(c) => Some(ContextFinding::Insufficient(c)),
        },
    )
}

fn validate_identities(
    contract: &EdgeEvidenceContract,
    records: &[EvidenceRecord],
    request: &SupportRequest<'_>,
) -> Result<(), SupportInputError> {
    for (i, r) in records.iter().enumerate() {
        if records[..i]
            .iter()
            .any(|p| p.observation().id() == r.observation().id())
        {
            return Err(SupportInputError::DuplicateRecordIdentity);
        }
    }
    for (i, e) in request.examinations.iter().enumerate() {
        if request.examinations[..i]
            .iter()
            .any(|p| p.observation == e.observation && p != e)
        {
            return Err(SupportInputError::ConflictingExamination);
        }
        if records
            .iter()
            .find(|r| r.observation().id() == &e.observation)
            .is_some_and(|r| {
                r.observation().subject().contract().task().id() != e.contract.task().id()
            })
        {
            return Err(SupportInputError::IncompatibleReference);
        }
    }
    let count = records
        .len()
        .checked_mul(2)
        .and_then(|n| n.checked_add(request.examinations.len()))
        .and_then(|n| n.checked_add(4))
        .ok_or(SupportInputError::Limit(SupportLimitKind::Records))?;
    let mut captures = reserved(count)?;
    for scope in [&contract.scope.producer, &contract.scope.consumer] {
        if let Some(c) = scope.context().capture() {
            captures.push(c);
        }
    }
    captures.extend(request.producer);
    captures.extend(request.consumer);
    for r in records {
        captures.push(r.observation().subject());
        if let Some(c) = r.assessment().and_then(|a| a.candidate_at_assessment()) {
            captures.push(c);
        }
    }
    for e in request.examinations {
        captures.extend(e.candidate);
    }
    for (i, c) in captures.iter().enumerate() {
        if captures[..i].iter().any(|p| p.id() == c.id() && p != c) {
            return Err(SupportInputError::IdentityConflict);
        }
    }
    let count = captures
        .len()
        .checked_add(contract.correspondences.len())
        .and_then(|n| n.checked_add(request.examinations.len()))
        .and_then(|n| n.checked_add(2))
        .ok_or(SupportInputError::Limit(SupportLimitKind::Records))?;
    let mut contracts = reserved(count)?;
    contracts.push(contract.scope.producer.context().contract());
    contracts.push(contract.scope.consumer.context().contract());
    contracts.extend(captures.iter().map(|c| c.contract()));
    contracts.extend(contract.correspondences.iter().map(|c| &c.verification));
    contracts.extend(request.examinations.iter().map(|e| e.contract));
    for (i, c) in contracts.iter().enumerate() {
        if contracts[..i]
            .iter()
            .any(|p| p.task().id() == c.task().id() && p.revision() == c.revision() && p != c)
        {
            return Err(SupportInputError::ContractAlias);
        }
    }
    Ok(())
}

fn measure(
    contract: &EdgeEvidenceContract,
    records: &[EvidenceRecord],
    request: &SupportRequest<'_>,
    limits: SupportLimits,
) -> Result<SupportUsage, SupportInputError> {
    let record_count = records.len().max(request.examinations.len());
    let predicates = contract
        .scope
        .requirement
        .predicates()
        .len()
        .max(contract.correspondences.len());
    check_count(record_count, limits.records, SupportLimitKind::Records)?;
    check_count(
        request.bindings.len(),
        limits.bindings,
        SupportLimitKind::Bindings,
    )?;
    check_count(predicates, limits.predicates, SupportLimitKind::Predicates)?;
    let mut m = Material::new(limits.material);
    m.contract(contract)?;
    for c in [request.producer, request.consumer].into_iter().flatten() {
        m.capture(c)?;
    }
    for r in records {
        m.record(r)?;
    }
    for e in request.examinations {
        m.observation(&e.observation)?;
        m.binding(e.contract)?;
        if let Some(c) = e.candidate {
            m.capture(c)?;
        }
        if let Some(p) = e.profile {
            m.profile(p)?;
        }
    }
    for b in request.bindings {
        m.add(b.predicate.as_str().len())?;
        m.observation(&b.evidence.observation)?;
        if let Some(a) = &b.evidence.assessment {
            m.add(a.as_str().len())?;
        }
        m.artifact(&b.artifact)?;
    }
    Ok(SupportUsage {
        records: record_count,
        bindings: request.bindings.len(),
        predicates,
        material: m.bytes,
    })
}

// Charges occurrences before staging copies, including nested retained contexts.
// This is neither a serialization scheme nor total allocated-memory accounting.
struct Material {
    bytes: usize,
    limit: usize,
}
impl Material {
    fn new(limit: usize) -> Self {
        Self { bytes: 0, limit }
    }
    fn add(&mut self, count: usize) -> Result<(), SupportInputError> {
        self.bytes = self
            .bytes
            .checked_add(count)
            .filter(|n| *n <= self.limit)
            .ok_or(SupportInputError::Limit(SupportLimitKind::MaterialBytes))?;
        Ok(())
    }
    fn observation(&mut self, id: &ObservationId) -> Result<(), SupportInputError> {
        self.add(id.scope().len())
    }
    fn requirement(&mut self, c: &CandidateRequirement) -> Result<(), SupportInputError> {
        for p in [
            c.repository().root(),
            c.repository().git_dir(),
            c.repository().common_dir(),
        ] {
            self.add(p.as_os_str().as_encoded_bytes().len())?;
        }
        self.add(c.commit().as_str().len())
    }
    fn binding(&mut self, c: &CandidateBindingContract) -> Result<(), SupportInputError> {
        self.add(c.task().id().as_str().len())?;
        self.add(c.task().objective().len())?;
        self.add(c.task().acceptance().len())?;
        self.requirement(c.candidate())
    }
    fn capture(&mut self, c: &CandidateObservation) -> Result<(), SupportInputError> {
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
    fn profile(&mut self, p: &VerificationProfile) -> Result<(), SupportInputError> {
        for s in [
            p.provenance().mechanism(),
            p.provenance().inputs(),
            p.provenance().configuration(),
            p.version(),
            p.environment(),
        ] {
            self.add(s.len())?;
        }
        for (n, v) in p.components() {
            self.add(n.len())?;
            self.add(v.len())?;
        }
        Ok(())
    }
    fn artifact(&mut self, a: &ArtifactReference) -> Result<(), SupportInputError> {
        self.add(a.id().as_str().len())?;
        self.add(a.candidate().as_str().len())
    }
    fn contract(&mut self, c: &EdgeEvidenceContract) -> Result<(), SupportInputError> {
        self.add(c.scope.graph.id().as_str().len())?;
        self.add(c.scope.edge.as_str().len())?;
        for s in [&c.scope.producer, &c.scope.consumer] {
            self.binding(s.context().contract())?;
            if let Some(capture) = s.context().capture() {
                self.capture(capture)?;
            }
        }
        self.artifact(c.scope.requirement.artifact())?;
        for p in c.scope.requirement.predicates() {
            self.add(p.as_str().len())?;
        }
        self.requirement(&c.artifact)?;
        for mapping in &c.correspondences {
            self.add(mapping.predicate.as_str().len())?;
            self.binding(&mapping.verification)?;
            self.profile(&mapping.profile)?;
        }
        self.add(c.basis.source().len())?;
        self.add(c.basis.version().len())?;
        match c.basis.origin() {
            SourceOrigin::OperatorReported { author } => self.add(author.len())?,
            SourceOrigin::ObservationReference(id) => self.observation(id)?,
        }
        Ok(())
    }
    fn record(&mut self, r: &EvidenceRecord) -> Result<(), SupportInputError> {
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn checked_material_overflow_refuses() {
        let mut m = Material {
            bytes: usize::MAX,
            limit: usize::MAX,
        };
        assert_eq!(
            m.add(1),
            Err(SupportInputError::Limit(SupportLimitKind::MaterialBytes))
        );
    }
    #[test]
    fn limits_have_positive_representable_neighbors() {
        assert!(SupportLimits::new(1, 1, 1, 1).is_ok());
        for values in [
            (0, 1, 1, 1),
            (1, 0, 1, 1),
            (1, 1, 0, 1),
            (1, 1, 1, 0),
            (usize::MAX, 1, 1, 1),
        ] {
            assert_eq!(
                SupportLimits::new(values.0, values.1, values.2, values.3),
                Err(SupportInputError::InvalidLimits)
            );
        }
    }
}
