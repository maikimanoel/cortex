//! Bounded evaluation accounting and advisory primary-outcome comparison.
//!
//! This is an in-memory measurement consumer, not a durable workflow journal,
//! runner, permission service, protected-oracle store or statistical claim engine.
//! Reports and acceptance/separation references are unauthenticated caller data.
use crate::{ExecutionObservation, unified::*};
use cortex_core::{
    binding::{CandidateBindingContract, CandidateObservation, ContractRevision, ObservationId},
    evaluation::*,
    program::memory::MemoryObservation,
    temporal::SourceReference,
};
use std::{
    error::Error,
    fmt,
    io::{self, Write},
    time::{SystemTime, UNIX_EPOCH},
};

/// Invalid measurement input. Rejection preserves all previously retained entries.
#[derive(Debug)]
pub enum EvaluationInputError {
    /// Existing contract construction failed.
    Contract(EvaluationError),
    /// Unknown, reused, forked or inconsistent identity.
    Identity,
    /// Logical count limit exceeded.
    Limit,
    /// Missing or inconsistent release/clock/interval correspondence.
    Timing,
    /// A terminal report already exists; retain a distinct new attempt instead.
    Terminal,
    /// Observation/configuration/criterion/candidate/oracle correspondence failed.
    Correspondence,
    /// This development foundation cannot accept performed live-provider work.
    ExecutionProfile,
    /// Known cumulative quantities overflow; no wrapped total is reported.
    Overflow,
}
impl fmt::Display for EvaluationInputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "evaluation input: {self:?}")
    }
}
impl Error for EvaluationInputError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Contract(e) => Some(e),
            _ => None,
        }
    }
}
/// Exact caller-declared oracle/whole-criterion/profile correspondence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OracleContract {
    /// Exact case identity.
    pub case: CaseId,
    /// Exact case revision.
    pub revision: ContractRevision,
    /// Exact external oracle reference; no oracle payload is disclosed.
    pub oracle: SourceReference,
    /// Full existing verification context, not just an equal version label.
    pub profile: VerificationProfile,
}
/// Retained release observation. It references authority but grants none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrialRelease {
    /// Exact release observation identity/version.
    pub reference: SourceReference,
    /// Monotonic clock lifetime/origin; restart must not reuse it.
    pub clock: SourceReference,
    /// Audit timestamp, separate from monotonic elapsed offsets.
    pub at: SystemTime,
    /// Existing authorization reference, asserted by the observing controller.
    pub authorization: SourceReference,
}
/// Measured versus estimated intervals are not interchangeable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimingBasis {
    /// Reported monotonic observation, not authenticated clock capture.
    MonotonicObservation,
    /// Retained estimate; never usable as primary measured latency.
    Estimate,
}
/// Caller-observed interval offsets in nanoseconds from the original trial release.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ElapsedInterval {
    start_ns: u64,
    end_ns: u64,
    basis: TimingBasis,
}
impl ElapsedInterval {
    /// Reject reversed intervals; zero is an actual zero interval, not missing.
    pub fn new(
        start_ns: u64,
        end_ns: u64,
        basis: TimingBasis,
    ) -> Result<Self, EvaluationInputError> {
        if end_ns < start_ns {
            return Err(EvaluationInputError::Timing);
        }
        Ok(Self {
            start_ns,
            end_ns,
            basis,
        })
    }
    /// Observed start offset, including gaps before a retry.
    pub fn start_ns(self) -> u64 {
        self.start_ns
    }
    /// Observed end offset from the original release, not attempt-only duration.
    pub fn end_ns(self) -> u64 {
        self.end_ns
    }
    /// Measurement versus estimation basis.
    pub fn basis(self) -> TimingBasis {
        self.basis
    }
}
/// Explicit phase coverage. Absent phases are not inferred from other logs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineeringPhase {
    /// Source/context and setup preparation.
    Preparation,
    /// Admission, scheduling and coordination.
    Coordination,
    /// Implementation work.
    Implementation,
    /// Verification work.
    Verification,
    /// Integration and integrated checks.
    Integration,
    /// Required independent review.
    Review,
    /// Corrective work.
    Repair,
    /// Verification after repair.
    Requalification,
    /// Required CI observation.
    Ci,
}
/// One explicitly observed phase; intervals may overlap and are never summed to
/// manufacture a whole-workflow duration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseObservation {
    /// Applicable engineering phase.
    pub phase: EngineeringPhase,
    /// Exact interval in the same release clock.
    pub interval: ElapsedInterval,
}
/// Cumulative per-attempt quantities. CPU time is not CPU rate; memory samples
/// retain their existing basis/scope and are deliberately not added here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Metric {
    /// Actual CPU consumption in nanoseconds, not elapsed time or millicores.
    CpuTimeNanos,
    /// Observed bytes written, not scratch occupancy or a disk quota.
    DiskBytesWritten,
    /// Observed model calls, not a configured allowance.
    ModelInvocations,
    /// Observed input tokens.
    InputTokens,
    /// Observed output tokens.
    OutputTokens,
    /// Observed CI consumption in job-nanoseconds.
    CiJobNanos,
    /// Observed operator interventions, not elapsed human effort.
    OperatorInterventions,
    /// Observed review findings; severity/effort remain outside this count.
    ReviewFindings,
}
const METRICS: [Metric; 8] = [
    Metric::CpuTimeNanos,
    Metric::DiskBytesWritten,
    Metric::ModelInvocations,
    Metric::InputTokens,
    Metric::OutputTokens,
    Metric::CiJobNanos,
    Metric::OperatorInterventions,
    Metric::ReviewFindings,
];
/// Missing quantities stay None. These observations never settle a reservation.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MetricObservations {
    values: [Option<u64>; 8],
    memory: Option<MemoryObservation>,
}
impl MetricObservations {
    /// Reject duplicate dimensions. Omitted dimensions remain unmeasured.
    pub fn new(
        values: &[(Metric, Option<u64>)],
        memory: Option<MemoryObservation>,
    ) -> Result<Self, EvaluationInputError> {
        if values.len() > 8 {
            return Err(EvaluationInputError::Limit);
        }
        let mut result = Self {
            memory,
            ..Self::default()
        };
        for (i, (metric, value)) in values.iter().enumerate() {
            if values[..i].iter().any(|(m, _)| m == metric) {
                return Err(EvaluationInputError::Identity);
            }
            result.values[*metric as usize] = *value;
        }
        Ok(result)
    }
    /// A measured zero is Some(0); missing is None.
    pub fn value(&self, metric: Metric) -> Option<u64> {
        self.values[metric as usize]
    }
    /// Typed sampled memory with its own scope, never an allocation total.
    pub fn memory(&self) -> Option<&MemoryObservation> {
        self.memory.as_ref()
    }
}
/// Truthful separation claim, not authenticated reviewer qualification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewerSeparation {
    /// Assessor is also the source author/executor.
    SameAuthor,
    /// Authorship differs; model family/operator/host may be shared.
    SeparateAuthorshipSharedContext,
    /// Caller claims organizational separation; this API cannot authenticate it.
    OrganizationallyIndependent,
}
/// The scope of a claimed accepted engineering result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultScope {
    /// Local branch/fixture completion; integration is still outstanding.
    Local,
    /// Caller explicitly reports appropriately integrated acceptance.
    Integrated,
}
/// Separate explicit acceptance claim. A PASS record alone is insufficient.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptanceClaim {
    /// Exact assessor reference, separate from the attempt's executor.
    pub assessor: SourceReference,
    /// Applicable independent-acceptance authority/policy reference, not a grant.
    pub authority: SourceReference,
    /// Honest review separation scope.
    pub separation: ReviewerSeparation,
    /// Local completion versus claimed integrated result.
    pub scope: ResultScope,
    /// Explicit acceptance-event offset in the observation's original release
    /// clock. Absence cannot be inferred from attempt completion or wall logs.
    pub accepted_elapsed_ns: Option<u64>,
}
/// Existing canonical Evidence for the entire exact case criterion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndpointEvidence {
    /// Result candidate and separate source tree assertion.
    pub source: SourceIdentity,
    /// Original report and terminal assessment; never overwritten by this consumer.
    pub record: EvidenceRecord,
    /// Actual separately ordered current capture supplied by its adapter.
    pub candidate: CandidateObservation,
    /// Separate acceptance claim or explicit absence.
    pub acceptance: Option<AcceptanceClaim>,
}
/// Completion/uncertainty is independent of semantic assessment result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttemptExecution {
    /// Existing execution observation, preserving NOT_RUN and unavailable states.
    Observed(ExecutionObservation),
    /// Outcome unresolved; it is not completed, passed, absent or measured zero.
    Unknown,
    /// Explicit scope examination, requiring canonical N/A Evidence.
    NotApplicable,
}
/// Origin of a performed observation. No live-provider conversion is provided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObservationOrigin {
    /// Programmatically constructed data; never actual runtime measurement.
    Synthetic,
    /// Actual finite provider-free fixture; not a qualified strategy runtime.
    DevelopmentFixture,
    /// Explicit unperformed/blocked/scope-only examination.
    Unperformed,
}
/// Explicit correspondence of one original report to its frozen evaluation input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservationContext {
    /// Exact specification reference; complete current-spec equality is also checked.
    pub specification: SourceReference,
    /// Exact case identity.
    pub case: CaseId,
    /// Exact case revision.
    pub case_revision: ContractRevision,
    /// Exact starting candidate and tree, distinct from a result candidate.
    pub source: SourceIdentity,
    /// Exact oracle version.
    pub oracle: SourceReference,
    /// Actual declared execution configuration; mismatches cannot be hidden.
    pub configuration: RunConfiguration,
}
/// Registered before work, so an unfinished attempt cannot disappear.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttemptDefinition {
    /// Fresh identity; identical repeated input is refused, not deduplicated.
    pub id: AttemptId,
    /// Existing predeclared trial; retries do not create another work item.
    pub trial: TrialId,
    /// Previous attempt of this trial, or none only for its first attempt.
    pub predecessor: Option<AttemptId>,
    /// Observed executor identity; this creates no execution authority.
    pub executor: SourceReference,
    /// Start offset from release, or absent for unperformed work.
    pub start_ns: Option<u64>,
}
/// One terminal immutable report. Public fields are untrusted input; record()
/// validates their correspondence and clones them into privately owned history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttemptObservation {
    /// Unique measurement observation identity.
    pub id: ObservationId,
    /// Exact registered attempt identity.
    pub attempt: AttemptId,
    /// Complete supplied correspondence.
    pub context: ObservationContext,
    /// Scope/provenance of this observation, never a live execution label.
    pub origin: ObservationOrigin,
    /// Actual outcome or explicit uncertainty.
    pub execution: AttemptExecution,
    /// Clock/release reference, required with measured intervals.
    pub release: Option<TrialRelease>,
    /// Actual/estimated interval; absent remains absent.
    pub interval: Option<ElapsedInterval>,
    /// At most 32 explicit subintervals, not inferred missing phases.
    pub phases: Vec<PhaseObservation>,
    /// Actual available quantities, including missing values.
    pub metrics: MetricObservations,
    /// Canonical semantic Evidence, independently optional from execution.
    pub endpoint: Option<EndpointEvidence>,
    /// Report mechanism/version/provenance; no raw sessions or output are retained.
    pub basis: SourceReference,
    /// Audit recording time, not a monotonic clock substitute.
    pub recorded_at: SystemTime,
}
/// One privately retained attempt and its original optional terminal observation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluationAttempt {
    definition: AttemptDefinition,
    observation: Option<AttemptObservation>,
}
impl EvaluationAttempt {
    /// Original attempt; registration is not authorization or dispatch.
    pub fn definition(&self) -> &AttemptDefinition {
        &self.definition
    }
    /// Original terminal observation or explicitly incomplete attempt.
    pub fn observation(&self) -> Option<&AttemptObservation> {
        self.observation.as_ref()
    }
}
/// Exact reason the primary accepted-outcome comparison cannot be established.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComparisonIssue {
    /// First known specification/context change remains sticky even if restored.
    SpecificationChanged,
    /// Current case source or tree differs.
    CandidateChanged,
    /// Current oracle identity differs.
    OracleChanged,
    /// Common model/tool/environment/information/resource controls differ.
    ConfoundedControls,
    /// A required configuration or resource amount remains unknown.
    UnestablishedControls,
    /// Declared case/repetition populations differ across arms or omit a case.
    PopulationMismatch,
    /// No report exists for a declared trial or registered attempt.
    MissingObservation,
    /// An original attempt outcome remains unresolved.
    UnknownOutcome,
    /// A planned runtime is unperformed/blocked or has no qualified implementation.
    UnavailableRuntime,
    /// No exact current independently accepted integrated endpoint is established.
    MissingAcceptedEndpoint,
    /// Estimated or missing timing cannot establish measured primary latency.
    UnmeasuredEndpoint,
}
/// Structural comparison eligibility, separate from primary-measure availability; never an Evidence verdict or permission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComparisonEligibility {
    /// Inputs match within declared controls; scope and endpoint availability remain separate.
    ComparableWithinDeclaredControls,
    /// Missing or incompatible inputs prevent comparison; populations remain visible.
    NotComparable,
}
/// Known subtotal plus missing population; no omitted quantity becomes zero.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetricTotal {
    /// Metric and its explicit units.
    pub metric: Metric,
    /// Sum of available per-attempt observations, including failed attempts.
    pub known_subtotal: u64,
    /// Attempts without a reported quantity; an unstarted trial also prevents total.
    pub missing_attempts: usize,
    /// Complete total only when the whole declared population was measured.
    pub total: Option<u64>,
}
/// One declared trial's retained status and reported endpoint, never a new score.
#[derive(Debug, Clone)]
pub struct TrialInspection {
    /// Original trial identity.
    pub trial: TrialId,
    /// Registered attempt count, including incomplete/failed attempts.
    pub attempts: usize,
    /// Latest retained attempt interval, separately scoped from accepted latency.
    pub latest_interval: Option<ElapsedInterval>,
    /// Distinct retry count, without silently deduplicating any attempt.
    pub retries: usize,
    /// Functional result of its last attempt, if assessed.
    pub functional_result: Option<AssessmentResult>,
    /// Independent applicability of the retained criterion.
    pub functional_applicability: Option<Applicability>,
    /// Validity against the retained current capture/profile, not live freshness.
    pub functional_validity: Option<Validity>,
    /// Reported acceptance separation; none is not independent acceptance.
    pub acceptance_separation: Option<ReviewerSeparation>,
    /// Whole release-to-accepted-end interval; no branch-only/estimated substitute.
    pub reported_primary_elapsed_ns: Option<u64>,
}
/// Counts describe all registered attempts, not just successful observations.
#[derive(Debug, Clone, Default)]
pub struct AttemptPopulation {
    /// All registered attempts.
    pub registered: usize,
    /// Known completed process/fixture observations, regardless of acceptance.
    pub completed: usize,
    /// Nonzero execution or semantic FAIL.
    pub failed: usize,
    /// Cancelled or timed-out execution.
    pub interrupted: usize,
    /// Explicit unresolved outcomes.
    pub unknown: usize,
    /// Explicit NOT_RUN observations.
    pub not_run: usize,
    /// Explicit unavailable/BLOCKED observations.
    pub blocked: usize,
    /// Explicit assessed N/A scope examinations.
    pub not_applicable: usize,
    /// Registered attempts without a terminal report.
    pub incomplete: usize,
}
/// One arm's complete declared population and observations.
#[derive(Debug, Clone)]
pub struct ArmInspection {
    /// Proposed strategy; fixtures do not implement its operational runtime.
    pub strategy: ExecutionStrategy,
    /// Complete selected trials, including absent/skipped cases.
    pub trials: Vec<TrialInspection>,
    /// Full retained attempt denominator and independent outcome categories.
    pub population: AttemptPopulation,
    /// Accepted work items under declared claims; retries count at most once.
    pub reported_accepted_trials: usize,
    /// Conservative cumulative quantities, not physical reservations or savings.
    pub metrics: Vec<MetricTotal>,
}
/// Scope of supplied data; no fixture or synthetic sample qualifies a live arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeasurementScope {
    /// Synthetic contract/model checking, not observed engineering throughput.
    Synthetic,
    /// Only finite development fixture observations, not strategy runtime qualification.
    DevelopmentFixture,
    /// No performed measurements.
    Unperformed,
    /// Incompatible observed scopes remain explicit.
    Mixed,
}
/// Inspectable advisory result. No superiority, acceleration, causal attribution,
/// publication permission or organizational acceptance is produced by this type.
#[derive(Debug, Clone)]
pub struct EvaluationReport {
    /// Primary comparison eligibility only.
    pub eligibility: ComparisonEligibility,
    /// Actual observation scope, independent of proposed strategy names.
    pub scope: MeasurementScope,
    /// Every declared trial has a measured, claimed accepted endpoint. This
    /// still grants no causal or statistical effectiveness conclusion.
    pub primary_outcome_available: bool,
    /// All retained/observed issues, not just a favorable first issue.
    pub issues: Vec<ComparisonIssue>,
    /// Every declared arm and trial remains present.
    pub arms: Vec<ArmInspection>,
    /// First invalidating full specification, if a change was observed.
    pub first_invalidation: Option<SourceReference>,
}
impl EvaluationReport {
    /// Omit caller identifiers, source paths, oracle/configuration contents and raw
    /// Evidence. Numeric observations are not a comparative improvement claim.
    pub fn write_redacted(&self, out: &mut impl Write) -> io::Result<()> {
        writeln!(
            out,
            "Evaluation: {:?}; scope={:?}; primary_outcome_available={}; improvement claim: NOT_ESTABLISHED; authority: NONE",
            self.eligibility, self.scope, self.primary_outcome_available
        )?;
        writeln!(
            out,
            "Limitations: caller assertions; no protected-oracle enforcement; no live strategy qualification; DEC-008 unselected"
        )?;
        writeln!(out, "Issues: {:?}", self.issues)?;
        for arm in &self.arms {
            writeln!(
                out,
                "Arm {:?}: declared_trials={} attempts={} completed={} failed={} interrupted={} unknown={} not_run={} blocked={} not_applicable={} incomplete={} reported_accepted_trials={}",
                arm.strategy,
                arm.trials.len(),
                arm.population.registered,
                arm.population.completed,
                arm.population.failed,
                arm.population.interrupted,
                arm.population.unknown,
                arm.population.not_run,
                arm.population.blocked,
                arm.population.not_applicable,
                arm.population.incomplete,
                arm.reported_accepted_trials
            )?;
            for trial in &arm.trials {
                writeln!(
                    out,
                    "  Trial: attempts={} retries={} functional={:?} applicability={:?} validity={:?} latest_interval={:?} acceptance_separation={:?} reported_primary_elapsed_ns={:?}",
                    trial.attempts,
                    trial.retries,
                    trial.functional_result,
                    trial.functional_applicability,
                    trial.functional_validity,
                    trial.latest_interval,
                    trial.acceptance_separation,
                    trial.reported_primary_elapsed_ns
                )?;
            }
            for total in &arm.metrics {
                writeln!(
                    out,
                    "  {:?}: known_subtotal={} missing_attempts={} total={:?}",
                    total.metric, total.known_subtotal, total.missing_attempts, total.total
                )?;
            }
        }
        Ok(())
    }
}
/// Bounded append-only measurement registry. It performs no effects and persists
/// nothing; process-death retention requires a separately authorized storage design.
#[derive(Debug, Clone)]
pub struct EvaluationLedger {
    specification: EvaluationSpecification,
    oracles: Vec<OracleContract>,
    releases: Vec<(TrialId, TrialRelease)>,
    attempts: Vec<EvaluationAttempt>,
    first_invalidation: Option<(EvaluationSpecification, Vec<OracleContract>)>,
}
impl EvaluationLedger {
    /// Freeze the full case/trial population and exact oracle correspondence before
    /// accepting measurements. No caller-selected successful subset is supported.
    pub fn new(
        specification: EvaluationSpecification,
        oracles: Vec<OracleContract>,
    ) -> Result<Self, EvaluationInputError> {
        if oracles.len() != specification.cases().len()
            || specification.cases().iter().any(|c| {
                oracles
                    .iter()
                    .filter(|o| o.case == c.id && o.revision == c.revision && o.oracle == c.oracle)
                    .count()
                    != 1
            })
        {
            return Err(EvaluationInputError::Correspondence);
        }
        Ok(Self {
            specification,
            oracles,
            releases: vec![],
            attempts: vec![],
            first_invalidation: None,
        })
    }
    /// Immutable original specification, not an approved comparison policy.
    pub fn specification(&self) -> &EvaluationSpecification {
        &self.specification
    }
    /// Frozen exact oracle/profile correspondence. Content, not just version
    /// labels, participates in every current-context examination.
    pub fn oracles(&self) -> &[OracleContract] {
        &self.oracles
    }
    /// Exact first invalidating context, including profile contents. This is
    /// retained evidence, not an assertion that an external source stayed current.
    pub fn first_invalidation(&self) -> Option<(&EvaluationSpecification, &[OracleContract])> {
        self.first_invalidation
            .as_ref()
            .map(|(s, o)| (s, o.as_slice()))
    }
    /// Complete retained attempts and original optional reports.
    pub fn attempts(&self) -> &[EvaluationAttempt] {
        &self.attempts
    }
    /// Retain one actual release before work. Duplicate/replaced release is refused.
    pub fn release_trial(
        &mut self,
        trial: TrialId,
        release: TrialRelease,
    ) -> Result<(), EvaluationInputError> {
        if release.at < UNIX_EPOCH
            || !self.specification.trials().iter().any(|t| t.id == trial)
            || self
                .releases
                .iter()
                .any(|(id, r)| *id == trial || r.reference == release.reference)
            || self.attempts.iter().any(|a| a.definition.trial == trial)
        {
            return Err(EvaluationInputError::Timing);
        }
        self.releases.push((trial, release));
        Ok(())
    }
    /// Register before performing work. This records an asserted attempt; it never
    /// authorizes a retry, including after Unknown or an unavailable runtime.
    pub fn register_attempt(
        &mut self,
        definition: AttemptDefinition,
    ) -> Result<(), EvaluationInputError> {
        if self.attempts.len() >= self.specification.limits().attempts {
            return Err(EvaluationInputError::Limit);
        }
        if self
            .attempts
            .iter()
            .any(|a| a.definition.id == definition.id)
            || !self
                .specification
                .trials()
                .iter()
                .any(|t| t.id == definition.trial)
        {
            return Err(EvaluationInputError::Identity);
        }
        let previous = self
            .attempts
            .iter()
            .rev()
            .find(|a| a.definition.trial == definition.trial);
        if previous.is_some_and(|p| p.observation.is_none())
            || definition.predecessor.as_ref() != previous.map(|a| &a.definition.id)
        {
            return Err(EvaluationInputError::Identity);
        }
        if definition.start_ns.is_some()
            && !self.releases.iter().any(|(id, _)| *id == definition.trial)
        {
            return Err(EvaluationInputError::Timing);
        }
        if let Some(start) = definition.start_ns {
            for earlier in self
                .attempts
                .iter()
                .filter(|a| a.definition.trial == definition.trial)
            {
                let end = earlier
                    .observation
                    .as_ref()
                    .and_then(|o| o.interval)
                    .filter(|i| i.basis == TimingBasis::MonotonicObservation)
                    .map(|i| i.end_ns);
                let accepted = earlier
                    .observation
                    .as_ref()
                    .filter(|o| {
                        o.interval
                            .is_some_and(|i| i.basis == TimingBasis::MonotonicObservation)
                    })
                    .and_then(|o| o.endpoint.as_ref())
                    .and_then(|e| e.acceptance.as_ref())
                    .and_then(|c| c.accepted_elapsed_ns);
                if earlier.definition.start_ns.is_some_and(|n| start < n)
                    || end.is_some_and(|n| start < n)
                    || accepted.is_some_and(|n| start < n)
                {
                    return Err(EvaluationInputError::Timing);
                }
            }
        }
        self.attempts.push(EvaluationAttempt {
            definition,
            observation: None,
        });
        Ok(())
    }
    /// Validate and retain one immutable terminal measurement. Malformed input does
    /// not erase its already registered incomplete attempt or mutate other reports.
    pub fn record(&mut self, observation: AttemptObservation) -> Result<(), EvaluationInputError> {
        let index = self
            .attempts
            .iter()
            .position(|a| a.definition.id == observation.attempt)
            .ok_or(EvaluationInputError::Identity)?;
        if self.attempts[index].observation.is_some() {
            return Err(EvaluationInputError::Terminal);
        }
        if self
            .attempts
            .iter()
            .filter_map(|a| a.observation.as_ref())
            .any(|o| o.id == observation.id)
        {
            return Err(EvaluationInputError::Identity);
        }
        self.validate_observation(&self.attempts[index].definition, &observation)?;
        let strategy = self
            .specification
            .trials()
            .iter()
            .find(|t| t.id == self.attempts[index].definition.trial)
            .expect("registered trial")
            .strategy;
        for metric in METRICS {
            self.attempts
                .iter()
                .filter(|a| {
                    self.specification
                        .trials()
                        .iter()
                        .any(|t| t.id == a.definition.trial && t.strategy == strategy)
                })
                .filter_map(|a| a.observation.as_ref())
                .filter_map(|o| o.metrics.value(metric))
                .chain(observation.metrics.value(metric))
                .try_fold(0u64, |sum, n| sum.checked_add(n))
                .ok_or(EvaluationInputError::Overflow)?;
        }
        self.attempts[index].observation = Some(observation);
        Ok(())
    }
    fn validate_observation(
        &self,
        attempt: &AttemptDefinition,
        o: &AttemptObservation,
    ) -> Result<(), EvaluationInputError> {
        let trial = self
            .specification
            .trials()
            .iter()
            .find(|t| t.id == attempt.trial)
            .ok_or(EvaluationInputError::Identity)?;
        let case = self
            .specification
            .cases()
            .iter()
            .find(|c| c.id == trial.case)
            .ok_or(EvaluationInputError::Identity)?;
        let arm = self
            .specification
            .arms()
            .iter()
            .find(|a| a.strategy == trial.strategy)
            .ok_or(EvaluationInputError::Identity)?;
        if o.context.specification != *self.specification.reference()
            || o.context.case != case.id
            || o.context.case_revision != case.revision
            || o.context.source != case.source
            || o.context.oracle != case.oracle
            || o.context.configuration != arm.configuration
        {
            return Err(EvaluationInputError::Correspondence);
        }
        let unperformed = matches!(
            o.execution,
            AttemptExecution::Observed(
                ExecutionObservation::NotRun | ExecutionObservation::Unavailable
            ) | AttemptExecution::NotApplicable
        );
        if (unperformed && o.origin != ObservationOrigin::Unperformed)
            || (!unperformed
                && match o.origin {
                    ObservationOrigin::Synthetic => {
                        arm.configuration.profile != ExecutionProfile::Synthetic
                    }
                    ObservationOrigin::DevelopmentFixture => {
                        arm.configuration.profile != ExecutionProfile::ProviderFreeFixture
                    }
                    ObservationOrigin::Unperformed => true,
                })
            || (!unperformed && arm.configuration.profile == ExecutionProfile::LiveProvider)
        {
            return Err(EvaluationInputError::ExecutionProfile);
        }
        if (arm.configuration.profile == ExecutionProfile::LiveProvider
            || arm.configuration.model == ModelConfiguration::NotApplicable)
            && [
                Metric::ModelInvocations,
                Metric::InputTokens,
                Metric::OutputTokens,
            ]
            .into_iter()
            .any(|m| o.metrics.value(m).is_some_and(|n| n > 0))
        {
            return Err(EvaluationInputError::ExecutionProfile);
        }
        if o.origin == ObservationOrigin::Synthetic
            && o.interval
                .is_some_and(|i| i.basis == TimingBasis::MonotonicObservation)
        {
            return Err(EvaluationInputError::Timing);
        }
        if o.phases.len() > 32 {
            return Err(EvaluationInputError::Limit);
        }
        if o.recorded_at < UNIX_EPOCH
            || (unperformed
                && (attempt.start_ns.is_some()
                    || o.interval.is_some()
                    || o.release.is_some()
                    || !o.phases.is_empty()))
        {
            return Err(EvaluationInputError::Timing);
        }
        if self
            .releases
            .iter()
            .find(|(id, _)| *id == trial.id)
            .is_some_and(|(_, r)| o.recorded_at < r.at)
        {
            return Err(EvaluationInputError::Timing);
        }
        match (&o.release, o.interval) {
            (Some(release), Some(interval)) => {
                if !self
                    .releases
                    .iter()
                    .any(|(id, r)| *id == trial.id && r == release)
                    || o.recorded_at < release.at
                    || attempt.start_ns != Some(interval.start_ns)
                    || o.phases.iter().any(|p| {
                        p.interval.start_ns < interval.start_ns
                            || p.interval.end_ns > interval.end_ns
                            || p.interval.basis != interval.basis
                    })
                {
                    return Err(EvaluationInputError::Timing);
                }
            }
            (None, None) if o.phases.is_empty() => {}
            _ => return Err(EvaluationInputError::Timing),
        }
        if let Some(endpoint) = &o.endpoint {
            if o.id == *endpoint.record.observation().id() || o.id == *endpoint.candidate.id() {
                return Err(EvaluationInputError::Identity);
            }
            let oracle = self
                .oracles
                .iter()
                .find(|r| r.case == case.id)
                .ok_or(EvaluationInputError::Correspondence)?;
            let contract = CandidateBindingContract::new(
                case.contract.task().clone(),
                case.contract.revision(),
                endpoint.source.candidate().clone(),
            );
            if endpoint.record.observation().subject().contract() != &contract
                || endpoint.record.observation().profile() != &oracle.profile
            {
                return Err(EvaluationInputError::Correspondence);
            }
            if endpoint.record.observation().times().recorded() > o.recorded_at
                || endpoint
                    .record
                    .assessment()
                    .is_some_and(|a| a.assessed_at() > o.recorded_at)
            {
                return Err(EvaluationInputError::Timing);
            }
            if let Some(claim) = &endpoint.acceptance
                && (self
                    .releases
                    .iter()
                    .find(|(id, _)| *id == trial.id)
                    .is_some_and(|(_, r)| {
                        endpoint
                            .record
                            .assessment()
                            .is_some_and(|a| a.assessed_at() < r.at)
                    })
                    || claim.accepted_elapsed_ns.is_some_and(|end| {
                        o.interval.is_none_or(|i| end < i.end_ns) || o.release.is_none()
                    }))
            {
                return Err(EvaluationInputError::Timing);
            }
            let mut record = endpoint.record.clone();
            let view = record.inspect(&ValidationContext::new(
                &contract,
                Some(&endpoint.candidate),
                Some(&oracle.profile),
            ));
            match (&o.execution, endpoint.record.observation().kind()) {
                (AttemptExecution::Observed(actual), ObservationKind::Execution(reported))
                    if *actual == reported.execution() => {}
                (AttemptExecution::NotApplicable, ObservationKind::ScopeInspection(_))
                    if view.result() == Some(AssessmentResult::NotApplicable) => {}
                (AttemptExecution::Unknown, ObservationKind::Inference) => {}
                _ => return Err(EvaluationInputError::Correspondence),
            }
            if endpoint.acceptance.is_some()
                && (!view.current_pass()
                    || !matches!(
                        o.execution,
                        AttemptExecution::Observed(ExecutionObservation::Exited { code: 0 })
                    ))
            {
                return Err(EvaluationInputError::Correspondence);
            }
        } else if matches!(o.execution, AttemptExecution::NotApplicable) {
            return Err(EvaluationInputError::Correspondence);
        }
        Ok(())
    }
    /// Inspect the complete original population. A material current-spec mismatch
    /// retains its first full invalidating context; restoring old inputs cannot
    /// revive this comparison. No report/assessment is overwritten or re-executed.
    pub fn inspect(
        &mut self,
        current: &EvaluationSpecification,
        oracles: &[OracleContract],
    ) -> Result<EvaluationReport, EvaluationInputError> {
        if oracles.len() != current.cases().len()
            || current.cases().iter().any(|c| {
                oracles
                    .iter()
                    .filter(|o| o.case == c.id && o.revision == c.revision && o.oracle == c.oracle)
                    .count()
                    != 1
            })
        {
            return Err(EvaluationInputError::Correspondence);
        }
        if (current != &self.specification || oracles != self.oracles)
            && self.first_invalidation.is_none()
        {
            self.first_invalidation = Some((current.clone(), oracles.to_vec()));
        }
        let mut issues = Vec::new();
        let mut add = |issue| {
            if !issues.contains(&issue) {
                issues.push(issue);
            }
        };
        if self.first_invalidation.is_some() {
            add(ComparisonIssue::SpecificationChanged);
        }
        if oracles != self.oracles
            || self
                .first_invalidation
                .as_ref()
                .is_some_and(|(_, o)| o != &self.oracles)
        {
            add(ComparisonIssue::OracleChanged);
        }
        for changed in
            std::iter::once(current).chain(self.first_invalidation.as_ref().map(|(s, _)| s))
        {
            for old in self.specification.cases() {
                if let Some(new) = changed.cases().iter().find(|c| c.id == old.id) {
                    if new.source != old.source {
                        add(ComparisonIssue::CandidateChanged);
                    }
                    if new.oracle != old.oracle {
                        add(ComparisonIssue::OracleChanged);
                    }
                }
            }
        }
        let first = &self.specification.arms()[0];
        let population = |strategy| {
            self.specification
                .trials()
                .iter()
                .filter(|t| t.strategy == strategy)
                .map(|t| (t.case.clone(), t.repetition))
                .collect::<std::collections::BTreeSet<_>>()
        };
        for arm in self.specification.arms() {
            if !arm.configuration.controls_match(&first.configuration) {
                add(ComparisonIssue::ConfoundedControls);
            }
            if arm.configuration.model == ModelConfiguration::Unestablished
                || arm
                    .configuration
                    .envelope
                    .iter()
                    .any(|d| d.amount.is_none())
            {
                add(ComparisonIssue::UnestablishedControls);
            }
            if population(arm.strategy) != population(first.strategy)
                || self.specification.cases().iter().any(|c| {
                    !self
                        .specification
                        .trials()
                        .iter()
                        .any(|t| t.case == c.id && t.strategy == arm.strategy)
                })
            {
                add(ComparisonIssue::PopulationMismatch);
            }
        }
        if self.specification.arms().len() < 2 {
            add(ComparisonIssue::PopulationMismatch);
        }
        let mut arms = Vec::new();
        for arm in self.specification.arms() {
            let entries: Vec<_> = self
                .attempts
                .iter()
                .filter(|a| {
                    self.specification
                        .trials()
                        .iter()
                        .any(|t| t.id == a.definition.trial && t.strategy == arm.strategy)
                })
                .collect();
            let mut counts = AttemptPopulation {
                registered: entries.len(),
                ..AttemptPopulation::default()
            };
            for entry in &entries {
                match entry.observation.as_ref().map(|o| &o.execution) {
                    None => {
                        counts.incomplete += 1;
                        add(ComparisonIssue::MissingObservation);
                    }
                    Some(AttemptExecution::Unknown) => {
                        counts.unknown += 1;
                        add(ComparisonIssue::UnknownOutcome);
                    }
                    Some(AttemptExecution::NotApplicable) => counts.not_applicable += 1,
                    Some(AttemptExecution::Observed(execution)) => match execution {
                        ExecutionObservation::Exited { code } => {
                            counts.completed += 1;
                            if *code != 0 {
                                counts.failed += 1;
                            }
                        }
                        ExecutionObservation::Cancelled | ExecutionObservation::TimedOut => {
                            counts.interrupted += 1
                        }
                        ExecutionObservation::NotRun => {
                            counts.not_run += 1;
                            add(ComparisonIssue::UnavailableRuntime);
                        }
                        ExecutionObservation::Unavailable => {
                            counts.blocked += 1;
                            add(ComparisonIssue::UnavailableRuntime);
                        }
                    },
                }
                if let Some(o) = &entry.observation
                    && matches!(
                        o.execution,
                        AttemptExecution::Observed(ExecutionObservation::Exited { code: 0 })
                    )
                    && o.endpoint
                        .as_ref()
                        .and_then(|e| e.record.assessment())
                        .is_some_and(|a| a.result() == AssessmentResult::Fail)
                {
                    counts.failed += 1;
                }
            }
            let mut trials = Vec::new();
            let mut accepted = 0;
            for trial in self
                .specification
                .trials()
                .iter()
                .filter(|t| t.strategy == arm.strategy)
            {
                let attempts: Vec<_> = entries
                    .iter()
                    .filter(|a| a.definition.trial == trial.id)
                    .collect();
                let latest = attempts.last().and_then(|a| a.observation.as_ref());
                let endpoint = latest.and_then(|o| o.endpoint.as_ref());
                let claim = endpoint.and_then(|e| e.acceptance.as_ref());
                let separation = claim.map(|c| c.separation);
                let accepted_here = self.first_invalidation.is_none()
                    && !attempts.is_empty()
                    && attempts.iter().all(|a| {
                        a.observation
                            .as_ref()
                            .is_some_and(|o| o.execution != AttemptExecution::Unknown)
                    })
                    && claim.is_some_and(|c| {
                        c.scope == ResultScope::Integrated
                            && c.separation != ReviewerSeparation::SameAuthor
                            && attempts.iter().all(|a| a.definition.executor != c.assessor)
                    });
                let primary = if accepted_here {
                    accepted += 1;
                    latest
                        .filter(|o| {
                            o.origin == ObservationOrigin::DevelopmentFixture
                                && o.interval
                                    .is_some_and(|i| i.basis == TimingBasis::MonotonicObservation)
                        })
                        .and_then(|o| o.endpoint.as_ref())
                        .and_then(|e| e.acceptance.as_ref())
                        .and_then(|c| c.accepted_elapsed_ns)
                } else {
                    None
                };
                if attempts.is_empty() {
                    add(ComparisonIssue::MissingObservation);
                }
                if !accepted_here {
                    add(ComparisonIssue::MissingAcceptedEndpoint);
                }
                if primary.is_none() {
                    add(ComparisonIssue::UnmeasuredEndpoint);
                }
                let validity = endpoint.and_then(|e| {
                    let oracle = self
                        .oracles
                        .iter()
                        .find(|o| o.case == trial.case)
                        .expect("frozen oracle");
                    let contract = e.record.observation().subject().contract();
                    let mut draft = e.record.clone();
                    draft
                        .inspect(&ValidationContext::new(
                            contract,
                            Some(&e.candidate),
                            Some(&oracle.profile),
                        ))
                        .validity()
                });
                trials.push(TrialInspection {
                    trial: trial.id.clone(),
                    attempts: attempts.len(),
                    latest_interval: latest.and_then(|o| o.interval),
                    retries: attempts.len().saturating_sub(1),
                    functional_result: endpoint
                        .and_then(|e| e.record.assessment())
                        .map(|a| a.result()),
                    functional_applicability: endpoint
                        .and_then(|e| e.record.assessment())
                        .map(|a| a.applicability()),
                    functional_validity: validity,
                    acceptance_separation: separation,
                    reported_primary_elapsed_ns: primary,
                });
            }
            let metrics = METRICS
                .into_iter()
                .map(|metric| {
                    let known_subtotal = entries
                        .iter()
                        .filter_map(|a| a.observation.as_ref())
                        .filter_map(|o| o.metrics.value(metric))
                        .sum();
                    let missing_attempts = entries
                        .iter()
                        .filter(|a| {
                            a.observation
                                .as_ref()
                                .and_then(|o| o.metrics.value(metric))
                                .is_none()
                        })
                        .count();
                    let complete_population = trials.iter().all(|t| t.attempts > 0);
                    MetricTotal {
                        metric,
                        known_subtotal,
                        missing_attempts,
                        total: if complete_population
                            && !entries.is_empty()
                            && missing_attempts == 0
                        {
                            Some(known_subtotal)
                        } else {
                            None
                        },
                    }
                })
                .collect();
            arms.push(ArmInspection {
                strategy: arm.strategy,
                trials,
                population: counts,
                reported_accepted_trials: accepted,
                metrics,
            });
        }
        let origins: Vec<_> = self
            .attempts
            .iter()
            .filter_map(|a| a.observation.as_ref())
            .map(|o| o.origin)
            .filter(|o| *o != ObservationOrigin::Unperformed)
            .collect();
        let scope = if origins.is_empty() {
            MeasurementScope::Unperformed
        } else if origins.iter().all(|o| *o == ObservationOrigin::Synthetic) {
            MeasurementScope::Synthetic
        } else if origins
            .iter()
            .all(|o| *o == ObservationOrigin::DevelopmentFixture)
        {
            MeasurementScope::DevelopmentFixture
        } else {
            MeasurementScope::Mixed
        };
        let eligibility = if issues.iter().all(|i| {
            matches!(
                i,
                ComparisonIssue::MissingAcceptedEndpoint | ComparisonIssue::UnmeasuredEndpoint
            )
        }) {
            ComparisonEligibility::ComparableWithinDeclaredControls
        } else {
            ComparisonEligibility::NotComparable
        };
        let primary_outcome_available = eligibility
            == ComparisonEligibility::ComparableWithinDeclaredControls
            && scope == MeasurementScope::DevelopmentFixture
            && arms.iter().all(|a| {
                a.trials
                    .iter()
                    .all(|t| t.reported_primary_elapsed_ns.is_some())
            });
        Ok(EvaluationReport {
            eligibility,
            scope,
            primary_outcome_available,
            issues,
            arms,
            first_invalidation: self
                .first_invalidation
                .as_ref()
                .map(|(s, _)| s.reference().clone()),
        })
    }
}
