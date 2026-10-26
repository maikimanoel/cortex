//! Validated reports supplied by callers; no observation authentication or execution.

use super::SemanticError;
use crate::{ExecutionObservation, Provenance, validate_text};
use cortex_core::binding::{CandidateObservation, ObservationId};
use std::{
    fmt,
    time::{SystemTime, UNIX_EPOCH},
};

/// Versioned mechanism, input/configuration descriptions and environment supplied by a caller.
#[derive(Clone, PartialEq, Eq)]
pub struct VerificationProfile {
    provenance: Provenance,
    version: String,
    environment: String,
    components: Vec<(String, String)>,
}
impl VerificationProfile {
    /// Validate required metadata and up to 32 participating components with unique names.
    /// Empty components mean none were declared; no agent/harness versions are invented.
    pub fn new(
        provenance: Provenance,
        version: &str,
        environment: &str,
        components: &[(&str, &str)],
    ) -> Result<Self, SemanticError> {
        validate_text("mechanism_version", version)?;
        validate_text("environment", environment)?;
        if components.len() > 32 {
            return Err(SemanticError::ComponentLimit);
        }
        let mut owned = Vec::with_capacity(components.len());
        for (name, version) in components {
            validate_text("component_name", name)?;
            validate_text("component_version", version)?;
            owned.push(((*name).to_owned(), (*version).to_owned()));
        }
        owned.sort();
        if owned.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            return Err(SemanticError::DuplicateComponent);
        }
        Ok(Self {
            provenance,
            version: version.to_owned(),
            environment: environment.to_owned(),
            components: owned,
        })
    }
    /// Inspect the original, potentially private, mechanism/input/configuration descriptions.
    pub fn provenance(&self) -> &Provenance {
        &self.provenance
    }
    /// Inspect the declared mechanism version.
    pub fn version(&self) -> &str {
        &self.version
    }
    /// Inspect the declared environment.
    pub fn environment(&self) -> &str {
        &self.environment
    }
    /// Inspect sorted participating component names and exact versions.
    pub fn components(&self) -> &[(String, String)] {
        &self.components
    }
}
impl fmt::Debug for VerificationProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VerificationProfile")
            .field("metadata", &"<private>")
            .field("component_count", &self.components.len())
            .finish()
    }
}

/// Reported observation and recording times, not authenticated execution timestamps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObservationTimes {
    observed: SystemTime,
    recorded: SystemTime,
}
impl ObservationTimes {
    /// Require times at/after the Unix epoch and recording no earlier than observation.
    pub fn new(observed: SystemTime, recorded: SystemTime) -> Result<Self, SemanticError> {
        if observed < UNIX_EPOCH || recorded < observed {
            return Err(SemanticError::InvalidChronology);
        }
        Ok(Self { observed, recorded })
    }
    /// Time the caller reports observing the outcome or unperformed/blocked status.
    pub fn observed(&self) -> SystemTime {
        self.observed
    }
    /// Time the caller reports recording that observation.
    pub fn recorded(&self) -> SystemTime {
        self.recorded
    }
}

/// Explicit oracle conclusion supplied separately from process exit status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OracleConclusion {
    /// Relevant checks support the criterion.
    Satisfied,
    /// Relevant checks demonstrate a criterion violation.
    Violated,
    /// Checks do not establish satisfaction or violation.
    Insufficient,
}
/// A reported interpretation of observations against the contract's criterion.
#[derive(Clone, PartialEq, Eq)]
pub struct OracleEvaluation {
    conclusion: OracleConclusion,
    reasoning: String,
}
impl OracleEvaluation {
    /// Retain explicit bounded reasoning; construction does not establish its truth.
    pub fn new(conclusion: OracleConclusion, reasoning: &str) -> Result<Self, SemanticError> {
        validate_text("oracle_reasoning", reasoning)?;
        Ok(Self {
            conclusion,
            reasoning: reasoning.to_owned(),
        })
    }
    /// Inspect the caller's oracle conclusion.
    pub fn conclusion(&self) -> OracleConclusion {
        self.conclusion
    }
    /// Inspect the potentially private oracle reasoning.
    pub fn reasoning(&self) -> &str {
        &self.reasoning
    }
}
impl fmt::Debug for OracleEvaluation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OracleEvaluation")
            .field("conclusion", &self.conclusion)
            .field("reasoning", &"<private>")
            .finish()
    }
}

/// A reported process outcome and relevant criterion-check count, never an assessment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationOutcome {
    execution: ExecutionObservation,
    checks: u64,
    oracle: Option<OracleEvaluation>,
}
impl VerificationOutcome {
    /// An incomplete/unperformed execution cannot claim completed checks or an oracle.
    /// A conclusive oracle requires at least one relevant check. Zero-test completion
    /// remains representable, but cannot support PASS or FAIL.
    pub fn new(
        execution: ExecutionObservation,
        checks: u64,
        oracle: Option<OracleEvaluation>,
    ) -> Result<Self, SemanticError> {
        if !matches!(execution, ExecutionObservation::Exited { .. })
            && (checks != 0 || oracle.is_some())
        {
            return Err(SemanticError::ContradictoryObservation);
        }
        if checks == 0
            && oracle
                .as_ref()
                .is_some_and(|o| o.conclusion != OracleConclusion::Insufficient)
        {
            return Err(SemanticError::ContradictoryObservation);
        }
        Ok(Self {
            execution,
            checks,
            oracle,
        })
    }
    /// Inspect the original foundation execution status.
    pub fn execution(&self) -> ExecutionObservation {
        self.execution
    }
    /// Inspect the reported count of relevant checks, not all discovered tests.
    pub fn checks(&self) -> u64 {
        self.checks
    }
    /// Inspect the explicit oracle, if one was supplied.
    pub fn oracle(&self) -> Option<&OracleEvaluation> {
        self.oracle.as_ref()
    }
}

/// Finding from a caller-reported examination of whether the criterion's subject exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeFinding {
    /// The governed subject/trigger was found.
    Present,
    /// Examination demonstrated absence; the report detail must explain the basis.
    Absent,
    /// Examination did not establish applicability.
    Uncertain,
}
/// Distinct kinds of caller reports; none carries operational approval.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObservationKind {
    /// Reported process execution and oracle, if available.
    Execution(VerificationOutcome),
    /// A reported inference from the retained candidate observation; detail is its derivation.
    /// Inferences cannot independently support PASS or FAIL in this bounded model.
    Inference,
    /// Reported examination of applicability; distinct from a missing tool or skipped check.
    ScopeInspection(ScopeFinding),
}

/// One caller report tied to the exact existing candidate observation and task revision.
#[derive(Clone, PartialEq, Eq)]
pub struct ReportedObservation {
    id: ObservationId,
    subject: CandidateObservation,
    profile: VerificationProfile,
    times: ObservationTimes,
    kind: ObservationKind,
    detail: String,
}
impl ReportedObservation {
    /// Retain explicit context and bounded detail (including blockage/absence/derivation reasons).
    /// Verification and repository observations must have distinct identities.
    pub fn new(
        id: ObservationId,
        subject: CandidateObservation,
        profile: VerificationProfile,
        times: ObservationTimes,
        kind: ObservationKind,
        detail: &str,
    ) -> Result<Self, SemanticError> {
        validate_text("observation_detail", detail)?;
        if &id == subject.id() {
            return Err(SemanticError::IdentityCollision);
        }
        Ok(Self {
            id,
            subject,
            profile,
            times,
            kind,
            detail: detail.to_owned(),
        })
    }
    /// Identify the verification, inference or applicability report.
    pub fn id(&self) -> &ObservationId {
        &self.id
    }
    /// Inspect exact task/revision, repository, committed/uncommitted state and coverage.
    pub fn subject(&self) -> &CandidateObservation {
        &self.subject
    }
    /// Inspect versioned, caller-reported mechanism provenance.
    pub fn profile(&self) -> &VerificationProfile {
        &self.profile
    }
    /// Inspect the caller-supplied temporal metadata.
    pub fn times(&self) -> ObservationTimes {
        self.times
    }
    /// Inspect the kind without promoting it to a judgment or authorization.
    pub fn kind(&self) -> &ObservationKind {
        &self.kind
    }
    /// Inspect the potentially private outcome/blockage/absence/derivation detail.
    pub fn detail(&self) -> &str {
        &self.detail
    }
}
impl fmt::Debug for ReportedObservation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReportedObservation")
            .field("context", &"<private>")
            .field("kind", &self.kind)
            .field("times", &self.times)
            .finish()
    }
}
