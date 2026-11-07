//! Read-only explanation of the existing supervised reconciliation guards.
use super::*;

/// Why retained observations cannot currently supply a reconciliation candidate.
/// These are missing proof categories, never instructions to adopt or signal a PID.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryBlocker {
    /// There is no durable dispatch to reconcile.
    NoDispatch,
    /// A supervised process specification is absent.
    MissingSpecification,
    /// Dispatch committed but no process receipt was retained.
    MissingReceipt,
    /// The latest capture is not a terminal outcome or proven pre-spawn refusal.
    NonterminalReceipt,
    /// No exact launcher incarnation was captured.
    MissingLauncherIdentity,
    /// No scope incarnation was captured.
    MissingScopeIdentity,
    /// Exact scope controls, membership, lifecycle or helper correspondence is missing.
    IncompleteScopeCorrespondence,
    /// An auxiliary helper remains unresolved.
    UnresolvedHelper,
    /// The exact process set has not been observed quiescent.
    UnprovenQuiescence,
    /// Required resource quantities cannot be constructed from the capture.
    MissingUsage,
    /// The candidate does not satisfy the canonical process reconciliation guard.
    ReconciliationGuard,
}

/// Inspection of retained facts only. A candidate is not an admitted operation,
/// an authenticated proof, current host observation or permission to retry work.
#[derive(Clone, PartialEq, Eq)]
pub struct ProcessRecoveryInspection {
    /// Resources/ownership remain held until the canonical transition actually commits.
    pub reservation_held: bool,
    /// Exact process specification is retained.
    pub specification_recorded: bool,
    /// Positive launcher incarnation occurs in retained observations.
    pub launcher_recorded: bool,
    /// Exact scope incarnation occurs in retained observations.
    pub scope_recorded: bool,
    /// A checkpoint was retained; this does not imply current operational authority.
    pub identity_checkpoint_recorded: bool,
    /// Decisive missing-proof categories, not an exhaustive inventory. Usage
    /// construction is not attempted until terminal identity/quiescence holds.
    pub blockers: Vec<RecoveryBlocker>,
    /// Existing structurally applicable reconciliation, checked by the same guard
    /// as admission. Human authority, current state and durable admission remain required.
    pub candidate: Option<Reconciliation>,
}

impl OperationalProjection {
    /// Inspect one exact retained supervised effect without I/O, mutation,
    /// dispatch, adoption, reservation release or creation of new recovery rules.
    /// Settled records return no further candidate. Historical missing facts stay missing.
    pub fn inspect_process_recovery(
        &self,
        effect: &EffectId,
        scope: &EffectScope,
    ) -> Result<ProcessRecoveryInspection, OperationalError> {
        if self.mode != EffectMode::SupervisedFixture {
            return Err(OperationalError::ExternalUnavailable);
        }
        let record = &self.effects[self.effect_index(effect, scope)?];
        let mut view = ProcessRecoveryInspection {
            reservation_held: record.pending(),
            specification_recorded: record.process_spec.is_some(),
            launcher_recorded: record
                .process_observations
                .iter()
                .any(|o| o.process.is_some()),
            scope_recorded: record
                .process_observations
                .iter()
                .any(|o| o.containment.is_some()),
            identity_checkpoint_recorded: record
                .process_observations
                .iter()
                .any(|o| o.completion == ProcessCompletion::IdentityVerified),
            blockers: vec![],
            candidate: None,
        };
        if !record.pending() {
            return Ok(view);
        }
        if record.dispatch_at.is_none() {
            view.blockers.push(RecoveryBlocker::NoDispatch);
            return Ok(view);
        }
        let Some(spec) = &record.process_spec else {
            view.blockers.push(RecoveryBlocker::MissingSpecification);
            return Ok(view);
        };
        let Some(observation) = record.process_observations.last() else {
            view.blockers.push(RecoveryBlocker::MissingReceipt);
            return Ok(view);
        };
        // Only the existing pre-spawn proof can propose Absent. A launcher that
        // later vanished cannot be reconstructed into NoChildStarted here.
        if observation.completion == ProcessCompletion::StartFailed {
            let candidate = Reconciliation::Absent;
            if self
                .check_process_reconciliation(record, &candidate)
                .is_ok()
            {
                view.candidate = Some(candidate);
            } else {
                view.blockers.push(RecoveryBlocker::ReconciliationGuard);
            }
            return Ok(view);
        }
        if matches!(
            observation.completion,
            ProcessCompletion::Spawned
                | ProcessCompletion::IdentityVerified
                | ProcessCompletion::Unknown
        ) {
            view.blockers.push(RecoveryBlocker::NonterminalReceipt);
        }
        if observation.process.is_none() {
            view.blockers.push(RecoveryBlocker::MissingLauncherIdentity);
        }
        if !observation.unresolved_scope_helpers.is_empty() {
            view.blockers.push(RecoveryBlocker::UnresolvedHelper);
        }
        if let Some(expected) = &spec.containment {
            match &observation.containment {
                None => view.blockers.push(RecoveryBlocker::MissingScopeIdentity),
                Some(actual) if !actual.corresponds(expected) => {
                    view.blockers
                        .push(RecoveryBlocker::IncompleteScopeCorrespondence);
                }
                _ => {}
            }
        }
        if !observation.quiescent(spec) {
            view.blockers.push(RecoveryBlocker::UnprovenQuiescence);
        }
        if !view.blockers.is_empty() {
            return Ok(view);
        }
        let Some(outcome) = observation.outcome(spec) else {
            view.blockers.push(RecoveryBlocker::NonterminalReceipt);
            return Ok(view);
        };
        let mut usage = Vec::with_capacity(record.intent.demand.len());
        for demand in &record.intent.demand {
            let amount = match demand.resource {
                Resource::WorkerSlots => Some(1),
                Resource::ElapsedMillis => Some(observation.elapsed_ms),
                Resource::OutputBytes => observation
                    .stdout_bytes
                    .checked_add(observation.stderr_bytes),
                Resource::CpuMillicores => spec
                    .containment
                    .as_ref()
                    .and_then(|s| s.cpu_capacity_millicores()),
                Resource::RamBytes => observation
                    .containment
                    .as_ref()
                    .and_then(|s| s.memory_peak_bytes),
                _ => None,
            };
            let Some(amount) = amount else {
                view.blockers.push(RecoveryBlocker::MissingUsage);
                return Ok(view);
            };
            usage.push(Demand {
                resource: demand.resource,
                amount: Some(amount),
            });
        }
        let candidate = Reconciliation::Observed { outcome, usage };
        if self
            .check_process_reconciliation(record, &candidate)
            .is_ok()
        {
            view.candidate = Some(candidate);
        } else {
            view.blockers.push(RecoveryBlocker::ReconciliationGuard);
        }
        Ok(view)
    }
}
