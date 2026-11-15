//! Advisory whole-program correspondence for the existing evaluation ledger.
//!
//! Declare the complete population before canonical attempts. Register each actual
//! attempt after its explicit reservation and before dispatch. Neither operation
//! grants authority, writes a workflow journal, retries, settles or refunds work.
use super::*;
use crate::program::{Command, CoordinationKernel, Operation};
use cortex_core::{
    TaskId,
    program::{
        AttemptId as ProgramAttemptId, AttemptStatus, OperationId, ProgramDefinition,
        operational::{
            EffectEvent, EffectId, EffectOutcome, EffectRecord, EffectScope, EffectStatus,
            Reconciliation,
        },
        supervision::{InterruptionReason, ProcessCompletion},
    },
};
use std::collections::BTreeSet;

/// One declared trial's exact canonical task correspondence.
#[derive(Clone, PartialEq, Eq)]
pub struct ProgramTrial {
    /// Existing declared trial, including planned or unperformed arms.
    pub trial: TrialId,
    /// Exact canonical task; labels from another program are insufficient.
    pub task: TaskId,
    /// Declared prepared requirement profile, separate from the run configuration.
    /// Required when an actual supervised process specification is present.
    pub process_profile: Option<SourceReference>,
}
/// Explicit evaluation-to-canonical attempt correspondence, never execution authority.
#[derive(Clone, PartialEq, Eq)]
pub struct ProgramAttempt {
    /// Existing evaluation identity, trial and explicit retry predecessor.
    pub definition: AttemptDefinition,
    /// Exact canonical program/task/candidate/attempt/package scope.
    pub scope: EffectScope,
    /// Admitted Claim witness.
    pub claim: OperationId,
    /// Admitted Package witness; an exact package may serve a later attempt.
    pub package: OperationId,
    /// Admitted reservation witness, before dispatch.
    pub reservation: OperationId,
    /// Exact reserved effect identity.
    pub effect: EffectId,
}
/// Non-disclosing refusal without a new authority or accounting mechanism.
#[derive(Debug)]
pub enum ProgramEvaluationError {
    /// Existing ledger guard and its original typed cause.
    Evaluation(EvaluationInputError),
    /// Foreign, duplicate or missing canonical identity.
    Identity,
    /// Declared task/trial population is incomplete or inconsistent.
    Population,
    /// Exact source, scope or operation correspondence failed.
    Correspondence,
    /// Declaration or registration followed the applicable effect boundary.
    AlreadyStarted,
    /// Claimed result exceeds the retained outcome or development scope.
    Outcome,
    /// Existing evaluation or program collection bound was exceeded.
    Limit,
}
impl fmt::Display for ProgramEvaluationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Evaluation(error) => write!(f, "program evaluation: {error}"),
            other => write!(f, "program evaluation: {other:?}"),
        }
    }
}
impl Error for ProgramEvaluationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Evaluation(error) => Some(error),
            _ => None,
        }
    }
}
impl From<EvaluationInputError> for ProgramEvaluationError {
    fn from(error: EvaluationInputError) -> Self {
        Self::Evaluation(error)
    }
}
/// Coverage limitations supplement the complete existing evaluation report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProgramCoverageIssue {
    /// Current accepted program definition differs from the declared baseline.
    ProgramChanged,
    /// A canonical attempt was not registered in the declared evaluation.
    UnmappedAttempt(ProgramAttemptId),
    /// A canonical effect lacks its exact registered mapping.
    UnmappedEffect(EffectId),
    /// Historical observations remain retained but current generation differs.
    StaleGeneration(ProgramAttemptId),
    /// The supplied kernel no longer retains the exact original operation witnesses.
    WitnessChanged(ProgramAttemptId),
    /// A known canonical result has no faithful execution representation here.
    /// The registered attempt remains incomplete; this never settles uncertainty.
    UnrepresentedOutcome {
        /// Exact retained canonical attempt.
        attempt: ProgramAttemptId,
        /// Actual retained mechanism outcome, not an engineering assessment.
        outcome: EffectOutcome,
    },
}
/// Inspection is advisory; only the existing ledger may retain in-memory invalidation.
#[derive(Debug, Clone)]
pub struct ProgramEvaluationReport {
    /// Every declared arm/trial and every registered attempt remains visible.
    pub evaluation: EvaluationReport,
    /// Exact unavailable coverage; an omitted producer cannot disappear.
    pub coverage: Vec<ProgramCoverageIssue>,
}
#[derive(Clone)]
struct RetainedAttempt {
    mapping: ProgramAttempt,
    witnesses: [Operation; 3],
}
/// Bounded joins around one existing ledger, not another observation ledger.
pub struct ProgramEvaluation {
    program: ProgramDefinition,
    ledger: EvaluationLedger,
    trials: Vec<ProgramTrial>,
    attempts: Vec<RetainedAttempt>,
}
impl fmt::Debug for ProgramEvaluation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProgramEvaluation")
            .field("trials", &self.trials.len())
            .field("attempts", &self.attempts.len())
            .finish()
    }
}
impl ProgramEvaluation {
    /// Freeze complete program/task/trial correspondence before any actual attempt.
    pub fn new(
        kernel: &CoordinationKernel,
        ledger: EvaluationLedger,
        trials: Vec<ProgramTrial>,
    ) -> Result<Self, ProgramEvaluationError> {
        let definition = kernel.state().definition();
        let spec = ledger.specification();
        if trials.len() > spec.limits().trials || trials.len() > definition.limits().entries {
            return Err(ProgramEvaluationError::Limit);
        }
        if !ledger.attempts().is_empty()
            || !ledger.releases.is_empty()
            || ledger.first_invalidation().is_some()
            || !kernel.state().attempts().is_empty()
            || !kernel.fixture_effects().effects().is_empty()
        {
            return Err(ProgramEvaluationError::AlreadyStarted);
        }
        let mut identities = BTreeSet::new();
        let mut tasks = BTreeSet::new();
        if trials.len() != spec.trials().len() {
            return Err(ProgramEvaluationError::Population);
        }
        for mapping in &trials {
            if !identities.insert(mapping.trial.as_str()) {
                return Err(ProgramEvaluationError::Population);
            }
            let trial = spec
                .trials()
                .iter()
                .find(|trial| trial.id == mapping.trial)
                .ok_or(ProgramEvaluationError::Population)?;
            let case = spec
                .cases()
                .iter()
                .find(|case| case.id == trial.case)
                .ok_or(ProgramEvaluationError::Population)?;
            let task = definition
                .graph()
                .tasks()
                .iter()
                .find(|task| task.id() == &mapping.task)
                .ok_or(ProgramEvaluationError::Population)?;
            if case.contract != *task.context().contract()
                || case.source.candidate() != task.context().contract().candidate()
            {
                return Err(ProgramEvaluationError::Correspondence);
            }
            tasks.insert(mapping.task.as_str());
        }
        if tasks.len() != definition.graph().tasks().len()
            || spec
                .cases()
                .iter()
                .any(|case| !trials.iter().any(|m| &m.task == case.contract.task().id()))
        {
            return Err(ProgramEvaluationError::Population);
        }
        Ok(Self {
            program: definition.clone(),
            ledger,
            trials,
            attempts: vec![],
        })
    }
    /// Inspect the single existing immutable ledger; mutation remains controlled here.
    pub fn ledger(&self) -> &EvaluationLedger {
        &self.ledger
    }
    /// Record an actual release reference without creating authorization.
    pub fn release_trial(
        &mut self,
        trial: &TrialId,
        release: TrialRelease,
    ) -> Result<(), ProgramEvaluationError> {
        self.ledger.release_trial(trial.clone(), release)?;
        Ok(())
    }
    /// Register one actual reserved attempt before dispatch, never a hypothetical retry.
    pub fn register_attempt(
        &mut self,
        kernel: &CoordinationKernel,
        attempt: ProgramAttempt,
    ) -> Result<(), ProgramEvaluationError> {
        if kernel.state().definition() != &self.program {
            return Err(ProgramEvaluationError::Correspondence);
        }
        if self.attempts.len() >= self.ledger.specification().limits().attempts
            || self.attempts.len() >= self.program.limits().entries
        {
            return Err(ProgramEvaluationError::Limit);
        }
        for prior in &self.attempts {
            self.check_retained(kernel, prior)?;
        }
        if self.attempts.iter().any(|old| {
            old.mapping.scope.attempt == attempt.scope.attempt
                || old.mapping.effect == attempt.effect
                || old.mapping.claim == attempt.claim
                || old.mapping.reservation == attempt.reservation
        }) {
            return Err(ProgramEvaluationError::Identity);
        }
        let trial = self
            .trials
            .iter()
            .find(|trial| trial.trial == attempt.definition.trial)
            .ok_or(ProgramEvaluationError::Identity)?;
        if trial.task != attempt.scope.task || attempt.scope.program != *self.program.id() {
            return Err(ProgramEvaluationError::Identity);
        }
        let scope = kernel
            .state()
            .readiness()
            .scope(&trial.task)
            .map_err(|_| ProgramEvaluationError::Correspondence)?;
        if attempt.scope.baseline != self.program.revision()
            || attempt.scope.generation != scope.generation()
            || attempt.scope.candidate != *scope.context().contract().candidate()
        {
            return Err(ProgramEvaluationError::Correspondence);
        }
        let witnesses = self.witnesses(kernel, &attempt)?;
        let effect = self.effect(kernel, &attempt)?;
        self.check_profile(&attempt, effect)?;
        if effect.status != EffectStatus::Reserved
            || effect.dispatch_at.is_some()
            || !effect.process_observations.is_empty()
            || kernel
                .state()
                .attempts()
                .iter()
                .find(|a| a.id == attempt.scope.attempt)
                .is_none_or(|a| a.status != AttemptStatus::Claimed)
        {
            return Err(ProgramEvaluationError::AlreadyStarted);
        }
        self.ledger.register_attempt(attempt.definition.clone())?;
        self.attempts.push(RetainedAttempt {
            mapping: attempt,
            witnesses,
        });
        Ok(())
    }
    /// Retain the actual historical outcome even if later invalidation changed generation.
    pub fn record(
        &mut self,
        kernel: &CoordinationKernel,
        observation: AttemptObservation,
    ) -> Result<(), ProgramEvaluationError> {
        let retained = self
            .attempts
            .iter()
            .find(|attempt| attempt.mapping.definition.id == observation.attempt)
            .ok_or(ProgramEvaluationError::Identity)?;
        self.check_retained(kernel, retained)?;
        let effect = self.effect(kernel, &retained.mapping)?;
        self.check_profile(&retained.mapping, effect)?;
        if observation
            .endpoint
            .as_ref()
            .and_then(|endpoint| endpoint.acceptance.as_ref())
            .is_some_and(|claim| claim.scope == ResultScope::Integrated)
        {
            return Err(ProgramEvaluationError::Outcome);
        }
        if !outcome_matches(effect, &observation.execution) {
            return Err(ProgramEvaluationError::Outcome);
        }
        // Available capture detail constrains even a synthetic evaluation report.
        // A mechanism failure after exit zero cannot become a successful execution
        // population entry: it remains explicit UnrepresentedOutcome in inspection.
        if has_terminal_capture(effect) {
            if captured_execution(effect)
                .map(AttemptExecution::Observed)
                .as_ref()
                != Some(&observation.execution)
            {
                return Err(ProgramEvaluationError::Correspondence);
            }
        } else if observation.origin == ObservationOrigin::DevelopmentFixture
            && matches!(
                observation.execution,
                AttemptExecution::Observed(
                    ExecutionObservation::Exited { .. }
                        | ExecutionObservation::TimedOut
                        | ExecutionObservation::Cancelled
                )
            )
        {
            return Err(ProgramEvaluationError::Correspondence);
        }
        self.ledger.record(observation)?;
        Ok(())
    }
    /// Inspect complete populations without modifying the canonical kernel or journal.
    pub fn inspect(
        &mut self,
        kernel: &CoordinationKernel,
        current: &EvaluationSpecification,
        oracles: &[OracleContract],
    ) -> Result<ProgramEvaluationReport, ProgramEvaluationError> {
        let mut coverage = Vec::new();
        if kernel.state().definition() != &self.program {
            coverage.push(ProgramCoverageIssue::ProgramChanged);
        }
        for attempt in kernel.state().attempts() {
            if !self
                .attempts
                .iter()
                .any(|mapping| mapping.mapping.scope.attempt == attempt.id)
            {
                coverage.push(ProgramCoverageIssue::UnmappedAttempt(attempt.id.clone()));
            }
        }
        for effect in kernel.fixture_effects().effects() {
            if !self
                .attempts
                .iter()
                .any(|mapping| mapping.mapping.effect == effect.intent.id)
            {
                coverage.push(ProgramCoverageIssue::UnmappedEffect(
                    effect.intent.id.clone(),
                ));
            }
        }
        for retained in &self.attempts {
            if let Ok(effect) = self.effect(kernel, &retained.mapping)
                && matches!(
                    effect.status,
                    EffectStatus::Reported | EffectStatus::ReconciledReported
                )
                && has_terminal_capture(effect)
                && let Some(outcome) = canonical_outcome(effect)
                && captured_execution(effect).is_none_or(|execution| {
                    !outcome_matches(effect, &AttemptExecution::Observed(execution))
                })
            {
                coverage.push(ProgramCoverageIssue::UnrepresentedOutcome {
                    attempt: retained.mapping.scope.attempt.clone(),
                    outcome,
                });
            }
            if self.check_retained(kernel, retained).is_err() {
                coverage.push(ProgramCoverageIssue::WitnessChanged(
                    retained.mapping.scope.attempt.clone(),
                ));
            }
            if kernel
                .state()
                .readiness()
                .scope(&retained.mapping.scope.task)
                .ok()
                .is_none_or(|scope| scope.generation() != retained.mapping.scope.generation)
            {
                coverage.push(ProgramCoverageIssue::StaleGeneration(
                    retained.mapping.scope.attempt.clone(),
                ));
            }
        }
        let mut evaluation = self.ledger.inspect(current, oracles)?;
        if !coverage.is_empty() {
            evaluation.eligibility = ComparisonEligibility::NotComparable;
            evaluation.primary_outcome_available = false;
            if !evaluation
                .issues
                .contains(&ComparisonIssue::PopulationMismatch)
            {
                evaluation.issues.push(ComparisonIssue::PopulationMismatch);
            }
            for arm in &mut evaluation.arms {
                arm.reported_accepted_trials = 0;
                for trial in &mut arm.trials {
                    trial.reported_primary_elapsed_ns = None;
                }
            }
        }
        Ok(ProgramEvaluationReport {
            evaluation,
            coverage,
        })
    }
    // This development adapter declares the run envelope per actual attempt and
    // the harness reference as the captured process-observation mechanism. These
    // conventions do not authenticate host/environment/information assertions.
    fn check_profile(
        &self,
        attempt: &ProgramAttempt,
        effect: &EffectRecord,
    ) -> Result<(), ProgramEvaluationError> {
        let mapping = self
            .trials
            .iter()
            .find(|trial| trial.trial == attempt.definition.trial)
            .ok_or(ProgramEvaluationError::Identity)?;
        let trial = self
            .ledger
            .specification()
            .trials()
            .iter()
            .find(|trial| trial.id == attempt.definition.trial)
            .ok_or(ProgramEvaluationError::Identity)?;
        let configuration = &self
            .ledger
            .specification()
            .arms()
            .iter()
            .find(|arm| arm.strategy == trial.strategy)
            .ok_or(ProgramEvaluationError::Identity)?
            .configuration;
        if configuration.envelope != effect.intent.demand
            || effect
                .process_spec
                .as_ref()
                .is_some_and(|spec| mapping.process_profile.as_ref() != Some(&spec.profile))
            || effect
                .process_observations
                .iter()
                .any(|observation| observation.basis != configuration.harness)
        {
            return Err(ProgramEvaluationError::Correspondence);
        }
        Ok(())
    }
    fn effect<'a>(
        &self,
        kernel: &'a CoordinationKernel,
        attempt: &ProgramAttempt,
    ) -> Result<&'a EffectRecord, ProgramEvaluationError> {
        kernel
            .fixture_effects()
            .effects()
            .iter()
            .find(|record| {
                record.intent.id == attempt.effect && record.intent.scope == attempt.scope
            })
            .ok_or(ProgramEvaluationError::Correspondence)
    }
    fn witnesses(
        &self,
        kernel: &CoordinationKernel,
        attempt: &ProgramAttempt,
    ) -> Result<[Operation; 3], ProgramEvaluationError> {
        if attempt.scope.program != *self.program.id()
            || attempt.scope.baseline != self.program.revision()
        {
            return Err(ProgramEvaluationError::Identity);
        }
        let find = |id: &OperationId| {
            kernel
                .operations()
                .iter()
                .position(|operation| &operation.id == id)
                .ok_or(ProgramEvaluationError::Identity)
        };
        let claim = find(&attempt.claim)?;
        let package = find(&attempt.package)?;
        let reservation = find(&attempt.reservation)?;
        if reservation <= claim || reservation <= package {
            return Err(ProgramEvaluationError::Correspondence);
        }
        let claim_op = &kernel.operations()[claim];
        let package_op = &kernel.operations()[package];
        let reservation_op = &kernel.operations()[reservation];
        let Command::Claim(claim) = &claim_op.command else {
            return Err(ProgramEvaluationError::Correspondence);
        };
        let Command::Package(package) = &package_op.command else {
            return Err(ProgramEvaluationError::Correspondence);
        };
        let (Command::FixtureEffect(event) | Command::SupervisedEffect(event)) =
            &reservation_op.command
        else {
            return Err(ProgramEvaluationError::Correspondence);
        };
        let EffectEvent::Reserve { intent, .. } = event.as_ref() else {
            return Err(ProgramEvaluationError::Correspondence);
        };
        let increment = self
            .program
            .increments()
            .iter()
            .find(|increment| increment.id == claim.increment)
            .ok_or(ProgramEvaluationError::Correspondence)?;
        if claim.id != attempt.scope.attempt
            || claim.baseline != attempt.scope.baseline
            || claim.generation != attempt.scope.generation
            || increment.task != attempt.scope.task
            || package.id != attempt.scope.package
            || package.revision != attempt.scope.package_revision
            || package.task != attempt.scope.task
            || package.baseline != attempt.scope.baseline
            || package.generation != attempt.scope.generation
            || intent.id != attempt.effect
            || intent.scope != attempt.scope
            || self.effect(kernel, attempt)?.intent != *intent
            || !kernel.state().attempts().iter().any(|a| {
                a.id == claim.id
                    && a.increment == claim.increment
                    && a.baseline == claim.baseline
                    && a.generation == claim.generation
                    && a.owner == claim.owner
            })
        {
            return Err(ProgramEvaluationError::Correspondence);
        }
        Ok([claim_op.clone(), package_op.clone(), reservation_op.clone()])
    }
    fn check_retained(
        &self,
        kernel: &CoordinationKernel,
        retained: &RetainedAttempt,
    ) -> Result<(), ProgramEvaluationError> {
        if kernel.state().definition().id() != self.program.id()
            || !kernel.state().definitions().contains(&self.program)
        {
            return Err(ProgramEvaluationError::Identity);
        }
        for operation in &retained.witnesses {
            if !kernel
                .operations()
                .iter()
                .any(|current| current == operation)
            {
                return Err(ProgramEvaluationError::Correspondence);
            }
        }
        let effect = self.effect(kernel, &retained.mapping)?;
        if effect.intent.scope != retained.mapping.scope {
            return Err(ProgramEvaluationError::Correspondence);
        }
        self.check_profile(&retained.mapping, effect)?;
        Ok(())
    }
}
fn outcome_matches(effect: &EffectRecord, outcome: &AttemptExecution) -> bool {
    match effect.status {
        EffectStatus::Unknown
        | EffectStatus::DispatchUncertain
        | EffectStatus::Running
        | EffectStatus::CancellationRequested => matches!(outcome, AttemptExecution::Unknown),
        EffectStatus::Reported | EffectStatus::ReconciledReported => {
            match (canonical_outcome(effect), outcome) {
                (
                    Some(EffectOutcome::Succeeded),
                    AttemptExecution::Observed(ExecutionObservation::Exited { code: 0 }),
                ) => true,
                (
                    Some(EffectOutcome::Failed),
                    AttemptExecution::Observed(ExecutionObservation::Exited { code }),
                ) => *code != 0,
                (
                    Some(EffectOutcome::Interrupted),
                    AttemptExecution::Observed(
                        ExecutionObservation::TimedOut | ExecutionObservation::Cancelled,
                    ),
                ) => true,
                _ => false,
            }
        }
        EffectStatus::CancelledBeforeDispatch | EffectStatus::ReconciledAbsent => matches!(
            outcome,
            AttemptExecution::Observed(
                ExecutionObservation::NotRun | ExecutionObservation::Unavailable
            )
        ),
        EffectStatus::Reserved => false,
    }
}

fn canonical_outcome(effect: &EffectRecord) -> Option<EffectOutcome> {
    effect.report.or_else(|| match &effect.reconciliation {
        Some(Reconciliation::Observed { outcome, .. }) => Some(*outcome),
        _ => None,
    })
}
fn has_terminal_capture(effect: &EffectRecord) -> bool {
    effect.process_observations.iter().any(|observation| {
        matches!(
            observation.completion,
            ProcessCompletion::Exited { .. } | ProcessCompletion::Interrupted { .. }
        )
    })
}
fn captured_execution(effect: &EffectRecord) -> Option<ExecutionObservation> {
    effect
        .process_observations
        .iter()
        .find_map(|observation| match observation.completion {
            ProcessCompletion::Exited {
                code: Some(code),
                signal: None,
            } if code >= 0 => Some(ExecutionObservation::Exited { code: code as u32 }),
            ProcessCompletion::Interrupted {
                reason:
                    InterruptionReason::Timeout
                    | InterruptionReason::StartupTimeout
                    | InterruptionReason::IdleTimeout,
            } => Some(ExecutionObservation::TimedOut),
            ProcessCompletion::Interrupted {
                reason: InterruptionReason::Cancelled,
            } => Some(ExecutionObservation::Cancelled),
            _ => None,
        })
}
