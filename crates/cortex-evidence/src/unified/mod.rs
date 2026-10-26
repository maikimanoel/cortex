//! Candidate-bound evidence with independent result, applicability and contextual validity.
//!
//! Foundation values remain available without reinterpretation or inferred metadata:
//! ```compile_fail
//! use cortex_evidence::{Verdict, unified::AssessmentResult};
//! let converted: AssessmentResult = Verdict::Inconclusive.into();
//! ```
//! All observations and oracle reasoning are caller reports. Validation establishes
//! structural/semantic consistency, not authenticity, software correctness or authority.

mod report;
pub use report::{
    ObservationKind, ObservationTimes, OracleConclusion, OracleEvaluation, ReportedObservation,
    ScopeFinding, VerificationOutcome, VerificationProfile,
};

use crate::{EvidenceError, ExecutionObservation, validate_text};
use cortex_core::{
    ContractError, TaskId,
    binding::{
        BindingDecision, BindingError, BindingInsufficiency, BindingMismatch, BindingStaleness,
        CandidateBindingContract, CandidateObservation, ObservationId, bind_candidate,
    },
};
use std::{
    error::Error,
    fmt,
    io::{self, Write},
    time::{SystemTime, UNIX_EPOCH},
};

/// Rejected unified evidence input or unsupported assessment transition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SemanticError {
    /// Bounded evidence text failed existing validation.
    Text(EvidenceError),
    /// Assessment identity failed foundation identifier syntax validation.
    Identity(ContractError),
    /// Contract context failed existing binding validation.
    Binding(BindingError),
    /// Too many participating components were declared.
    ComponentLimit,
    /// A component name was declared more than once.
    DuplicateComponent,
    /// Times were before the epoch or ordered inconsistently.
    InvalidChronology,
    /// Verification and candidate observation identities were conflated.
    IdentityCollision,
    /// Execution/check/oracle assertions contradict one another.
    ContradictoryObservation,
    /// Result is incompatible with the applicability determination.
    InvalidApplicability,
    /// Result lacks its required observation kind or execution state.
    UnsupportedResult,
    /// A conclusive assessment lacks a matching explicit oracle and relevant checks.
    MissingOracle,
    /// PASS requires completed zero-exit execution.
    NonzeroExitCannotPass,
    /// PASS/FAIL/N/A cannot be established against stale or unresolved candidate context.
    CurrentContextRequired,
    /// A terminal assessment cannot be replaced.
    AlreadyAssessed,
    /// First invalidation reason cannot be overwritten.
    AlreadyInvalidated,
}
impl From<EvidenceError> for SemanticError {
    fn from(error: EvidenceError) -> Self {
        Self::Text(error)
    }
}
impl fmt::Display for SemanticError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::Text(e) => return e.fmt(f),
            Self::Identity(_) => "invalid assessment identity",
            Self::Binding(e) => return e.fmt(f),
            Self::ComponentLimit => "verification profile exceeds 32 components",
            Self::DuplicateComponent => "verification profile has duplicate component names",
            Self::InvalidChronology => "evidence timestamps are inconsistent",
            Self::IdentityCollision => {
                "candidate and verification observations need distinct identities"
            }
            Self::ContradictoryObservation => {
                "execution, relevant checks and oracle reports contradict one another"
            }
            Self::InvalidApplicability => "assessment result contradicts applicability",
            Self::UnsupportedResult => {
                "assessment result is unsupported by this observation kind or execution state"
            }
            Self::MissingOracle => {
                "conclusive assessment requires relevant checks and an explicit matching oracle"
            }
            Self::NonzeroExitCannotPass => "nonzero execution exit cannot support PASS",
            Self::CurrentContextRequired => {
                "conclusive assessment requires current candidate and verification context"
            }
            Self::AlreadyAssessed => "evidence already has a terminal assessment",
            Self::AlreadyInvalidated => "evidence already retains its first invalidation",
        };
        f.write_str(message)
    }
}
impl Error for SemanticError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Text(e) => Some(e),
            Self::Identity(e) => Some(e),
            Self::Binding(e) => Some(e),
            _ => None,
        }
    }
}

/// What the explicit assessment concluded, independent of applicability and validity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssessmentResult {
    /// Applicable criterion supported by completed relevant checks and an explicit oracle.
    Pass,
    /// Applicable criterion violated according to completed checks and an explicit oracle.
    Fail,
    /// Examination occurred but a supported conclusion is unavailable.
    Unknown,
    /// Verification was explicitly not performed.
    NotRun,
    /// A reported unavailable prerequisite prevented verification.
    Blocked,
    /// Examination demonstrated that the criterion's subject/trigger is absent.
    NotApplicable,
}
impl AssessmentResult {
    fn label(self) -> &'static str {
        match self {
            Self::Pass => "PASS",
            Self::Fail => "FAIL",
            Self::Unknown => "UNKNOWN",
            Self::NotRun => "NOT_RUN",
            Self::Blocked => "BLOCKED",
            Self::NotApplicable => "N/A",
        }
    }
}
/// Whether the single retained contract criterion applies to its evaluated subject.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Applicability {
    /// Criterion applies.
    Applicable,
    /// Examination established absence of the governed subject/trigger.
    NotApplicable,
    /// Applicability remains uncertain.
    Uncertain,
}
impl Applicability {
    fn label(self) -> &'static str {
        match self {
            Self::Applicable => "APPLICABLE",
            Self::NotApplicable => "NOT_APPLICABLE",
            Self::Uncertain => "UNCERTAIN",
        }
    }
}
/// Correspondence to an explicitly supplied candidate/profile context, never permanent freshness.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Validity {
    /// Relevant supplied contexts still correspond within inherited coverage limits.
    Current,
    /// A known relevant mismatch or explicit invalidation prevents current reuse.
    Stale,
}

/// Caller-assigned assessment identity, distinct from task, commit and observation identities.
#[derive(Clone, PartialEq, Eq)]
pub struct AssessmentId(TaskId);
impl AssessmentId {
    /// Reuse foundation identifier syntax without implying uniqueness or authentication.
    pub fn new(value: &str) -> Result<Self, SemanticError> {
        TaskId::new(value)
            .map(Self)
            .map_err(SemanticError::Identity)
    }
    /// Inspect the caller-assigned identity.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}
impl fmt::Debug for AssessmentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AssessmentId(<private>)")
    }
}

/// Expected context for an evidence examination. Producers must actually capture
/// the separate current candidate; labels and increasing IDs do not authenticate it.
pub struct ValidationContext<'a> {
    contract: &'a CandidateBindingContract,
    candidate: Option<&'a CandidateObservation>,
    profile: Option<&'a VerificationProfile>,
}
impl<'a> ValidationContext<'a> {
    /// Missing candidate/profile stays explicit and cannot establish CURRENT.
    pub fn new(
        contract: &'a CandidateBindingContract,
        candidate: Option<&'a CandidateObservation>,
        profile: Option<&'a VerificationProfile>,
    ) -> Self {
        Self {
            contract,
            candidate,
            profile,
        }
    }
}
/// Why evidence is stale or its current validity cannot be established.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidityIssue {
    /// Known candidate or task mismatch.
    CandidateMismatch(BindingMismatch),
    /// Known revision, observation-order or state change.
    CandidateStale(BindingStaleness),
    /// Binding lacks enough information; this does not itself prove staleness.
    CandidateInsufficient(BindingInsufficiency),
    /// Contradictory contract input; not a valid current context.
    InvalidContext(BindingError),
    /// Expected verification mechanism/version/inputs/configuration/environment changed.
    ProfileChanged,
    /// Expected verification profile was not supplied.
    MissingProfile,
    /// Verification and reobservation IDs were conflated.
    ObservationIdentityCollision,
    /// Caller explicitly invalidated evidence; private rationale is retained separately.
    ExplicitInvalidation,
}
impl ValidityIssue {
    fn label(&self) -> &'static str {
        match self {
            Self::CandidateMismatch(_) => "CANDIDATE_MISMATCH",
            Self::CandidateStale(_) => "CANDIDATE_STALE",
            Self::CandidateInsufficient(_) => "CANDIDATE_INSUFFICIENT",
            Self::InvalidContext(_) => "INVALID_CONTEXT",
            Self::ProfileChanged => "PROFILE_CHANGED",
            Self::MissingProfile => "MISSING_PROFILE",
            Self::ObservationIdentityCollision => "OBSERVATION_IDENTITY_COLLISION",
            Self::ExplicitInvalidation => "EXPLICIT_INVALIDATION",
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct ContextCheck {
    validity: Option<Validity>,
    issue: Option<ValidityIssue>,
    compared_to: Option<ObservationId>,
}

/// Terminal historical conclusion; use an EvidenceView before claiming current PASS.
#[derive(Clone, PartialEq, Eq)]
pub struct UnifiedAssessment {
    id: AssessmentId,
    result: AssessmentResult,
    applicability: Applicability,
    rationale: String,
    assessed_at: SystemTime,
    context: ContextCheck,
    candidate: Option<Box<CandidateObservation>>,
    profile: Option<VerificationProfile>,
}
impl UnifiedAssessment {
    /// Inspect assessment identity.
    pub fn id(&self) -> &AssessmentId {
        &self.id
    }
    /// Inspect the historical result, which is not automatically current.
    pub fn result(&self) -> AssessmentResult {
        self.result
    }
    /// Inspect its explicit applicability determination.
    pub fn applicability(&self) -> Applicability {
        self.applicability
    }
    /// Inspect potentially private reasoning, including applicability/blockage rationale.
    pub fn rationale(&self) -> &str {
        &self.rationale
    }
    /// Inspect caller-reported assessment time.
    pub fn assessed_at(&self) -> SystemTime {
        self.assessed_at
    }
    /// Inspect the separate candidate observation supplied at assessment, if any.
    pub fn compared_to(&self) -> Option<&ObservationId> {
        self.context.compared_to.as_ref()
    }
    /// Inspect the supplied reobservation retained for the historical judgment.
    pub fn candidate_at_assessment(&self) -> Option<&CandidateObservation> {
        self.candidate.as_deref()
    }
    /// Inspect the exact expected verification context supplied for the judgment.
    pub fn profile_at_assessment(&self) -> Option<&VerificationProfile> {
        self.profile.as_ref()
    }
    /// Inspect why validity was stale or unestablished at assessment.
    pub fn validity_issue_at_assessment(&self) -> Option<&ValidityIssue> {
        self.context.issue.as_ref()
    }
    /// Inspect validity established at assessment time; None means unestablished.
    pub fn validity_at_assessment(&self) -> Option<Validity> {
        self.context.validity
    }
}
impl fmt::Debug for UnifiedAssessment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UnifiedAssessment")
            .field("result", &self.result)
            .field("applicability", &self.applicability)
            .field("context", &"<private>")
            .finish()
    }
}

/// One immutable reported observation, at most one terminal assessment and sticky invalidation.
/// Construction is unassessed; compatibility and process success do not infer a judgment.
#[derive(Clone, PartialEq, Eq)]
pub struct EvidenceRecord {
    observation: ReportedObservation,
    assessment: Option<UnifiedAssessment>,
    invalidation: Option<ContextCheck>,
    invalidation_reason: Option<String>,
}
impl EvidenceRecord {
    /// Start unassessed, without asserting current candidate validity.
    pub fn new(observation: ReportedObservation) -> Self {
        Self {
            observation,
            assessment: None,
            invalidation: None,
            invalidation_reason: None,
        }
    }
    /// Inspect the original report, full candidate/contract and provenance.
    pub fn observation(&self) -> &ReportedObservation {
        &self.observation
    }
    /// Inspect a historical assessment. It grants no authority and may now be stale.
    pub fn assessment(&self) -> Option<&UnifiedAssessment> {
        self.assessment.as_ref()
    }
    /// Inspect a private explicit invalidation rationale, if one was supplied.
    pub fn invalidation_reason(&self) -> Option<&str> {
        self.invalidation_reason.as_deref()
    }

    /// Explicitly invalidate without replacing the observation, result or first cause.
    /// There is no stale-to-current mutation. Revalidation needs a new record/report/assessment.
    pub fn invalidate(&mut self, reason: &str) -> Result<(), SemanticError> {
        if self.invalidation.is_some() {
            return Err(SemanticError::AlreadyInvalidated);
        }
        validate_text("invalidation_reason", reason)?;
        self.invalidation = Some(ContextCheck {
            validity: Some(Validity::Stale),
            issue: Some(ValidityIssue::ExplicitInvalidation),
            compared_to: None,
        });
        self.invalidation_reason = Some(reason.to_owned());
        Ok(())
    }

    fn check(&self, context: &ValidationContext<'_>) -> ContextCheck {
        if let Some(stale) = &self.invalidation {
            return stale.clone();
        }
        let compared_to = context.candidate.map(|candidate| candidate.id().clone());
        if context
            .candidate
            .is_some_and(|candidate| candidate.id() == self.observation.id())
        {
            return ContextCheck {
                validity: None,
                issue: Some(ValidityIssue::ObservationIdentityCollision),
                compared_to,
            };
        }
        let (validity, issue) = match context.profile {
            None => (None, Some(ValidityIssue::MissingProfile)),
            Some(profile) if profile != self.observation.profile() => {
                (Some(Validity::Stale), Some(ValidityIssue::ProfileChanged))
            }
            Some(_) => match bind_candidate(
                context.contract,
                Some(self.observation.subject()),
                context.candidate,
            ) {
                Ok(BindingDecision::Compatible(_)) => (Some(Validity::Current), None),
                Ok(BindingDecision::Incompatible(reason)) => (
                    Some(Validity::Stale),
                    Some(ValidityIssue::CandidateMismatch(reason)),
                ),
                Ok(BindingDecision::Stale(reason)) => (
                    Some(Validity::Stale),
                    Some(ValidityIssue::CandidateStale(reason)),
                ),
                Ok(BindingDecision::Insufficient(reason)) => {
                    (None, Some(ValidityIssue::CandidateInsufficient(reason)))
                }
                Err(error) => (None, Some(ValidityIssue::InvalidContext(error))),
            },
        };
        ContextCheck {
            validity,
            issue,
            compared_to,
        }
    }

    /// Validate result/applicability/observations and retain exactly one assessment.
    /// Invalid judgment inputs leave the record unchanged. Valid context examination
    /// can record sticky invalidation even when a conclusive judgment is rejected.
    /// Observation and assessment are never replaced. Timestamp is caller-reported.
    pub fn assess(
        &mut self,
        id: AssessmentId,
        assessed_at: SystemTime,
        result: AssessmentResult,
        applicability: Applicability,
        rationale: &str,
        context: &ValidationContext<'_>,
    ) -> Result<(), SemanticError> {
        if self.assessment.is_some() {
            return Err(SemanticError::AlreadyAssessed);
        }
        validate_text("assessment_rationale", rationale)?;
        if assessed_at < UNIX_EPOCH || assessed_at < self.observation.times().recorded() {
            return Err(SemanticError::InvalidChronology);
        }
        validate_result(self.observation.kind(), result, applicability)?;
        let check = self.check(context);
        if let Some(ValidityIssue::InvalidContext(error)) = &check.issue {
            return Err(SemanticError::Binding(error.clone()));
        }
        if check.issue == Some(ValidityIssue::ObservationIdentityCollision) {
            return Err(SemanticError::IdentityCollision);
        }
        // Retain a demonstrated context change even if it prevents this judgment.
        if check.validity == Some(Validity::Stale) && self.invalidation.is_none() {
            self.invalidation = Some(check.clone());
        }
        if matches!(
            result,
            AssessmentResult::Pass | AssessmentResult::Fail | AssessmentResult::NotApplicable
        ) && check.validity != Some(Validity::Current)
        {
            return Err(SemanticError::CurrentContextRequired);
        }
        self.assessment = Some(UnifiedAssessment {
            id,
            result,
            applicability,
            rationale: rationale.to_owned(),
            assessed_at,
            context: check,
            candidate: context
                .candidate
                .map(|candidate| Box::new(candidate.clone())),
            profile: context.profile.cloned(),
        });
        Ok(())
    }

    /// Re-examine contextual validity; known mismatches invalidate permanently in this record.
    /// Missing/uncertain context returns unestablished validity and cannot support current PASS.
    pub fn inspect<'a>(&'a mut self, context: &ValidationContext<'_>) -> EvidenceView<'a> {
        let check = self.check(context);
        if check.validity == Some(Validity::Stale) && self.invalidation.is_none() {
            self.invalidation = Some(check.clone());
        }
        EvidenceView {
            record: self,
            check,
        }
    }
}
impl fmt::Debug for EvidenceRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EvidenceRecord")
            .field("observation", &self.observation)
            .field("assessment", &self.assessment)
            .field("invalidated", &self.invalidation.is_some())
            .finish()
    }
}

fn validate_result(
    kind: &ObservationKind,
    result: AssessmentResult,
    applicability: Applicability,
) -> Result<(), SemanticError> {
    let allowed = match applicability {
        Applicability::Applicable => result != AssessmentResult::NotApplicable,
        Applicability::NotApplicable => result == AssessmentResult::NotApplicable,
        Applicability::Uncertain => matches!(
            result,
            AssessmentResult::Unknown | AssessmentResult::NotRun | AssessmentResult::Blocked
        ),
    };
    if !allowed {
        return Err(SemanticError::InvalidApplicability);
    }
    let execution = match kind {
        ObservationKind::Execution(outcome) => Some(outcome.execution()),
        _ => None,
    };
    match result {
        AssessmentResult::Pass | AssessmentResult::Fail => {
            let ObservationKind::Execution(outcome) = kind else {
                return Err(SemanticError::UnsupportedResult);
            };
            let ExecutionObservation::Exited { code } = outcome.execution() else {
                return Err(SemanticError::UnsupportedResult);
            };
            if result == AssessmentResult::Pass && code != 0 {
                return Err(SemanticError::NonzeroExitCannotPass);
            }
            let expected = if result == AssessmentResult::Pass {
                OracleConclusion::Satisfied
            } else {
                OracleConclusion::Violated
            };
            if outcome.checks() == 0
                || outcome
                    .oracle()
                    .is_none_or(|oracle| oracle.conclusion() != expected)
            {
                return Err(SemanticError::MissingOracle);
            }
        }
        AssessmentResult::NotRun if execution != Some(ExecutionObservation::NotRun) => {
            return Err(SemanticError::UnsupportedResult);
        }
        AssessmentResult::Blocked if execution != Some(ExecutionObservation::Unavailable) => {
            return Err(SemanticError::UnsupportedResult);
        }
        AssessmentResult::NotApplicable
            if !matches!(kind, ObservationKind::ScopeInspection(ScopeFinding::Absent)) =>
        {
            return Err(SemanticError::UnsupportedResult);
        }
        AssessmentResult::Unknown
            if matches!(
                execution,
                Some(ExecutionObservation::NotRun | ExecutionObservation::Unavailable)
            ) =>
        {
            return Err(SemanticError::UnsupportedResult);
        }
        _ => {}
    }
    Ok(())
}

/// Inspection result that keeps historical assessment dimensions and contextual validity separate.
/// No view or PASS carries an execution grant.
pub struct EvidenceView<'a> {
    record: &'a EvidenceRecord,
    check: ContextCheck,
}
impl EvidenceView<'_> {
    /// Inspect historical result; None is unassessed, not NOT_RUN or PASS.
    pub fn result(&self) -> Option<AssessmentResult> {
        self.record.assessment.as_ref().map(|a| a.result)
    }
    /// Inspect historical applicability independently of validity.
    pub fn applicability(&self) -> Option<Applicability> {
        self.record.assessment.as_ref().map(|a| a.applicability)
    }
    /// Inspect contextual validity; None means missing/insufficient/invalid validation inputs.
    pub fn validity(&self) -> Option<Validity> {
        self.check.validity
    }
    /// Inspect mismatch, insufficient-information or explicit invalidation reason.
    pub fn validity_issue(&self) -> Option<&ValidityIssue> {
        self.check.issue.as_ref()
    }
    /// Inspect the candidate reference used for this check or first sticky invalidation.
    pub fn compared_to(&self) -> Option<&ObservationId> {
        self.check.compared_to.as_ref()
    }
    /// True only for an applicable historical PASS with CURRENT correspondence to supplied context.
    /// This is neither permanent freshness, software certification nor permission.
    pub fn current_pass(&self) -> bool {
        self.result() == Some(AssessmentResult::Pass)
            && self.applicability() == Some(Applicability::Applicable)
            && self.validity() == Some(Validity::Current)
    }
    /// Write a deliberately redacted, non-versioned summary. All caller strings,
    /// identities, paths, raw provenance, oracle text and comparison bytes are omitted.
    /// Full context remains inspectable through explicit typed record getters.
    pub fn write_redacted(&self, output: &mut impl Write) -> io::Result<()> {
        writeln!(
            output,
            "Result: {}",
            self.result().map_or("UNASSESSED", AssessmentResult::label)
        )?;
        writeln!(
            output,
            "Applicability: {}",
            self.applicability()
                .map_or("UNASSESSED", Applicability::label)
        )?;
        writeln!(
            output,
            "Validity: {}",
            match self.validity() {
                Some(Validity::Current) => "CURRENT",
                Some(Validity::Stale) => "STALE",
                None => "UNESTABLISHED",
            }
        )?;
        writeln!(
            output,
            "Validity issue: {}",
            self.validity_issue().map_or("NONE", ValidityIssue::label)
        )?;
        let state = self.record.observation.subject().state();
        writeln!(
            output,
            "Candidate HEAD: {}",
            match state.head() {
                cortex_core::binding::Head::Attached { .. } => "ATTACHED COMMIT",
                cortex_core::binding::Head::Detached { .. } => "DETACHED COMMIT",
                cortex_core::binding::Head::Unborn { .. } => "UNBORN (NO COMMIT)",
                cortex_core::binding::Head::Unknown => "UNKNOWN",
            }
        )?;
        writeln!(
            output,
            "Coverage: {:?}; consistency: {:?}",
            state.coverage(),
            state.consistency()
        )?;
        let staged = state
            .entries()
            .iter()
            .filter(|entry| {
                matches!(
                    entry,
                    cortex_core::binding::Entry::Tracked {
                        staged: Some(_),
                        ..
                    }
                )
            })
            .count();
        let unstaged = state
            .entries()
            .iter()
            .filter(|entry| {
                matches!(
                    entry,
                    cortex_core::binding::Entry::Tracked {
                        unstaged: Some(_),
                        ..
                    }
                )
            })
            .count();
        let untracked = state
            .entries()
            .iter()
            .filter(|entry| matches!(entry, cortex_core::binding::Entry::Untracked { .. }))
            .count();
        writeln!(
            output,
            "Working tree: {}; staged: {staged}; unstaged: {unstaged}; untracked: {untracked}",
            if state.coverage() != cortex_core::binding::ObservationCoverage::GitStatus
                || state.consistency()
                    != cortex_core::binding::ObservationConsistency::TwoMatchingSamples
                || state.repository().is_none()
                || matches!(state.head(), cortex_core::binding::Head::Unknown)
            {
                "INDETERMINATE"
            } else if state.entries().is_empty() {
                "CLEAN (Git status coverage)"
            } else {
                "DIRTY"
            }
        )?;
        match self.record.observation.kind() {
            ObservationKind::Execution(outcome) => writeln!(
                output,
                "Observation: CALLER_REPORTED_EXECUTION; status: {:?}; relevant checks: {}",
                outcome.execution(),
                outcome.checks()
            )?,
            ObservationKind::Inference => {
                writeln!(output, "Observation: CALLER_REPORTED_INFERENCE")?
            }
            ObservationKind::ScopeInspection(finding) => writeln!(
                output,
                "Observation: CALLER_REPORTED_SCOPE_INSPECTION; finding: {finding:?}"
            )?,
        }
        writeln!(output, "Private context/provenance/reasons: REDACTED")?;
        writeln!(output, "Authority: NONE (evidence grants no permission)")
    }
}
