//! Read-only assessment of an independently supplied normal-end closure candidate.
use super::accounting::CpuCounterObservation;
use super::managed_backend::{ManagedEndObservation, ManagedEndReadback, successful_managed_end};
use cortex_core::{
    program::{
        domain::{Consumption, DomainEvent, DomainIdentity, DomainIntent, HistoricalOutcome},
        operational::EffectOutcome,
    },
    temporal::{SourceOrigin, SourceReference},
};
use cortex_evidence::program::{Command, CoordinationKernel, KernelError, Operation};
use std::time::Instant;

/// One cumulative kernel counter read from the independently observed incarnation.
pub struct TerminalCpuRead {
    /// Exact boot, manager, invocation and held cgroup counter incarnation.
    pub identity: DomainIdentity,
    /// Cut under the original controller monotonic clock.
    pub at: Instant,
    /// Cgroup `usage_usec`, before converting to cumulative CPU milliseconds.
    pub usage_micros: u64,
}

/// References supplied by the independent observer for the entire cgroup lifetime.
///
/// The caller supplies support for a baseline before the first member, a terminal
/// read after all payload, bootstrap, helper and writer activity, and prevention
/// of reentry, restart and snapshot restoration from the terminal cut. An
/// observation reference alone does not authenticate any of those assertions.
pub struct NormalEndCpuWitness {
    /// Cumulative read before any attributable work, including bootstrap work.
    pub baseline: TerminalCpuRead,
    /// Terminal read before counter loss, after the independently observed fence.
    pub terminal: TerminalCpuRead,
    /// Independent observation of counter continuity and lifetime attribution.
    pub lifetime: SourceReference,
    /// Independent observation of complete process, helper, and stream coverage.
    pub population_and_streams: SourceReference,
    /// Independent observation of no reentry, restart, or snapshot resurrection.
    pub terminal_fence: SourceReference,
    /// Observation binding the underlying proof to the proposed receipt.
    pub receipt: SourceReference,
}

/// Read-only refusal reasons. No result authorizes settlement or releases debt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NormalEndClosureError {
    /// No retained managed end, binding, or matching durable release.
    MissingContext,
    /// Changed domain, release, configuration, scope, or cgroup incarnation.
    Correspondence,
    /// Non-success, cancellation, incomplete streams, or observed population.
    NotNormalEnd,
    /// No exact observed and consistent cumulative lower bound.
    CounterUnavailable,
    /// Independent lifetime, population, stream, or fence proof was absent.
    WitnessRequired,
    /// A witness cut falls outside the original interval or observed chronology.
    WitnessTiming,
    /// The selected fresh-cgroup proof needs an initial zero and monotonic reads.
    CounterContinuity,
    /// The claimed final CPU debit is absent, nonunique, or numerically inconsistent.
    CpuDebit,
    /// The exact operation was already retained and needs no new submission.
    AlreadyRecorded,
    /// The submitted current Closed/Settle operation failed canonical admission.
    Canonical(KernelError),
}

pub(super) struct NormalEndContext<'a> {
    pub(super) intent: &'a DomainIntent,
    pub(super) observation: &'a ManagedEndObservation,
    pub(super) start: Instant,
    pub(super) end: Instant,
}

pub(super) fn check_normal_end_cpu_closure(
    kernel: &CoordinationKernel,
    context: NormalEndContext<'_>,
    witness: Option<&NormalEndCpuWitness>,
    operation: &Operation,
) -> Result<u64, NormalEndClosureError> {
    use NormalEndClosureError::*;
    let end = context.observation;
    let record = kernel
        .domains()
        .and_then(|domains| domains.domain(&end.domain))
        .ok_or(MissingContext)?;
    let binding = record.binding_observation.as_ref().ok_or(MissingContext)?;
    if context.intent != &record.intent
        || record.identity.as_ref() != Some(&end.binding_identity)
        || binding.identity != end.binding_identity
        || binding.configuration != record.intent.configuration
    {
        return Err(Correspondence);
    }
    let release = kernel
        .operations()
        .iter()
        .find(|operation| operation.id == end.release)
        .ok_or(MissingContext)?;
    if !matches!(&release.command, Command::Domain(event) if matches!(
        event.as_ref(),
        DomainEvent::ReleaseRequested { domain, identity, .. }
            if domain == &end.domain && identity == &end.binding_identity
    )) {
        return Err(Correspondence);
    }
    if record.cancelled
        || record.outcome != HistoricalOutcome::Reported(EffectOutcome::Succeeded)
        || !successful_managed_end(&end.completion, end.transport_complete, &end.readback)
    {
        return Err(NotNormalEnd);
    }
    let ManagedEndReadback::Observed {
        cpu: CpuCounterObservation::LowerBoundMicros(lower_bound),
        ..
    } = &end.readback
    else {
        return Err(CounterUnavailable);
    };
    let witness = witness.ok_or(WitnessRequired)?;
    let Command::Domain(event) = &operation.command else {
        return Err(Correspondence);
    };
    let receipt = match event.as_ref() {
        DomainEvent::Closed { receipt, .. } | DomainEvent::Settle { receipt, .. } => receipt,
        _ => return Err(Correspondence),
    };
    if receipt.domain != end.domain
        || receipt.identity != end.binding_identity
        || receipt.configuration != record.intent.configuration
        || receipt.scope != record.intent.effect.scope
        || witness.baseline.identity != end.binding_identity
        || witness.terminal.identity != end.binding_identity
        || receipt.evidence != witness.receipt
    {
        return Err(Correspondence);
    }
    if [
        &witness.lifetime,
        &witness.population_and_streams,
        &witness.terminal_fence,
        &witness.receipt,
    ]
    .into_iter()
    .any(|reference| {
        reference.origin() == binding.basis.origin()
            || !matches!(reference.origin(), SourceOrigin::ObservationReference(_))
    }) {
        return Err(WitnessRequired);
    }
    if witness.baseline.at < context.start
        || witness.baseline.at >= end.observed_at
        || end.observed_at > witness.terminal.at
        || witness.terminal.at >= context.end
    {
        return Err(WitnessTiming);
    }
    // A fresh, dedicated cgroup covers its whole charged lifetime. A nonzero
    // baseline could conceal bootstrap work and is outside this proof profile.
    if witness.baseline.usage_micros != 0 || witness.terminal.usage_micros < *lower_bound {
        return Err(CounterContinuity);
    }
    let amount = witness.terminal.usage_micros.div_ceil(1000);
    let mut cpu_debits = receipt
        .consumption
        .iter()
        .filter(|entry| entry.dimension == Consumption::CpuExecutionMillis);
    if cpu_debits.next().and_then(|entry| entry.measured) != Some(amount)
        || cpu_debits.next().is_some()
    {
        return Err(CpuDebit);
    }
    if kernel
        .preview_admission(operation)
        .map_err(Canonical)?
        .is_duplicate()
    {
        return Err(AlreadyRecorded);
    }
    Ok(amount)
}

#[cfg(test)]
mod tests;
