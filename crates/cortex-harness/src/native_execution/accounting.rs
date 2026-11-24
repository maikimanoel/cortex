//! Cgroup CPU samples and read-only corroboration of an independent closure claim.
use cortex_core::program::domain::{
    ClosureReceipt, Consumption, DomainClosureTreeObservation, DomainRecord,
};

mod lifetime;
pub use lifetime::{ManagedCpuCut, ManagedCpuObservations, ManagedCpuReadback};
pub(in crate::native_execution) use lifetime::{clock_namespace, monotonic_ns};
mod settlement;
#[cfg(test)]
pub(crate) use settlement::tests::source_cpu_attempt;
pub use settlement::{
    CollectedCpuClosure, CollectedCpuClosureAction, CollectedCpuClosureCut,
    CollectedCpuClosureError,
};
pub(in crate::native_execution) use settlement::{challenge, submit};

const CPU_STAT_LIMIT: usize = 16_384;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CounterError {
    Missing,
    Malformed,
    Duplicate,
    Range,
    Incomplete,
}

/// Read one complete kernel flat-keyed counter. Other keys may be added by the
/// kernel. A malformed occurrence of the selected key still counts as a row.
pub(crate) fn parse_cpu_usage(input: &str) -> Result<u64, CounterError> {
    if input.len() > CPU_STAT_LIMIT || !input.ends_with('\n') {
        return Err(CounterError::Incomplete);
    }
    let mut rows = input.lines().filter_map(|line| {
        let mut fields = line.split_ascii_whitespace();
        (fields.next() == Some("usage_usec")).then_some(fields)
    });
    let mut entry = rows.next().ok_or(CounterError::Missing)?;
    if rows.next().is_some() {
        return Err(CounterError::Duplicate);
    }
    let value = entry.next().ok_or(CounterError::Malformed)?;
    if entry.next().is_some() || !value.bytes().all(|b| b.is_ascii_digit()) {
        return Err(CounterError::Malformed);
    }
    value.parse().map_err(|_| CounterError::Range)
}

/// Bounded cumulative CPU readback for one already verified cgroup epoch.
/// None of these states establishes complete attributable terminal consumption.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuCounterObservation {
    /// At least one bounded read was absent, unreadable, or malformed.
    Unavailable,
    /// The counter regressed, or changed across an apparently empty tree read.
    Contradictory,
    /// Cumulative usage in microseconds, usable only as a lower bound.
    LowerBoundMicros(u64),
}

impl CpuCounterObservation {
    pub(crate) fn lower_bound_micros(self) -> Option<u64> {
        match self {
            Self::LowerBoundMicros(value) => Some(value),
            Self::Unavailable | Self::Contradictory => None,
        }
    }
}

/// Compare samples of one already verified counter epoch. Equality is not a fence.
pub(crate) fn bracket_cpu_observation(
    before: Option<u64>,
    after: Option<u64>,
    populated: bool,
) -> CpuCounterObservation {
    match (before, after) {
        (Some(before), Some(after)) if after >= before && (populated || before == after) => {
            CpuCounterObservation::LowerBoundMicros(after)
        }
        (Some(_), Some(_)) => CpuCounterObservation::Contradictory,
        _ => CpuCounterObservation::Unavailable,
    }
}

/// Why a candidate closure debit could not be compared with an exact sample.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuClosureCheckError {
    /// No retained sample at the requested tick, or its numeric value is absent.
    MissingSample,
    /// A newer retained sample or a decreasing observation invalidates the comparison.
    StaleSample,
    /// Domain, incarnation, configuration, scope, or binding does not match.
    Correspondence,
    /// The observation or asserted closure lacks the applicable coverage.
    Incomplete,
    /// The candidate repeats, omits, or leaves the CPU dimension unresolved.
    Debit,
    /// The claimed final amount is below the observed cumulative sample.
    Undercount,
}

/// Compare a proposed existing closure receipt against a retained native tree
/// observation from the same epoch. Return the sample rounded upward in
/// cumulative scheduled CPU milliseconds. This screens for undercounting.
/// The caller must separately establish finality, authenticity, freshness and
/// authorization before submitting the receipt through the canonical journal.
/// No usage is accepted or released by this check.
pub fn check_cpu_closure_against_sample(
    record: &DomainRecord,
    sampled_at: u64,
    receipt: &ClosureReceipt,
) -> Result<u64, CpuClosureCheckError> {
    let observation = record
        .closure_tree_observations
        .last()
        .ok_or(CpuClosureCheckError::MissingSample)?;
    if observation.sample.observed_at != sampled_at
        || record
            .closure_observations
            .last()
            .is_some_and(|entry| entry.observed_at > sampled_at)
    {
        return Err(CpuClosureCheckError::StaleSample);
    }
    let amount = check_sample(record, observation, receipt)?;
    if !monotonic_cpu_history(record) {
        return Err(CpuClosureCheckError::StaleSample);
    }
    Ok(amount)
}

fn monotonic_cpu_history(record: &DomainRecord) -> bool {
    // Core admits both observation kinds in strictly increasing tick order.
    // Merge the retained sequences without allocation to check the same epoch.
    let mut direct = record.closure_observations.iter().peekable();
    let mut tree = record
        .closure_tree_observations
        .iter()
        .map(|entry| &entry.sample)
        .peekable();
    let mut previous = 0;
    while let Some(entry) = match (direct.peek(), tree.peek()) {
        (Some(left), Some(right)) if left.observed_at < right.observed_at => direct.next(),
        (_, Some(_)) => tree.next(),
        _ => direct.next(),
    } {
        if let Some(usage) = entry.cpu_micros {
            if usage < previous {
                return false;
            }
            previous = usage;
        }
    }
    true
}

fn check_sample(
    record: &DomainRecord,
    observation: &DomainClosureTreeObservation,
    receipt: &ClosureReceipt,
) -> Result<u64, CpuClosureCheckError> {
    let invalid = CpuClosureCheckError::Correspondence;
    observation.validate().map_err(|_| invalid)?;
    let sample = &observation.sample;
    let binding = record.binding_observation.as_ref().ok_or(invalid)?;
    if receipt.domain != record.intent.id
        || record.identity.as_ref() != Some(&sample.identity)
        || binding.identity != sample.identity
        || receipt.identity != sample.identity
        || sample.configuration != record.intent.configuration
        || binding.configuration != sample.configuration
        || receipt.configuration != sample.configuration
        || sample.scope != record.intent.effect.scope
        || receipt.scope != sample.scope
        || sample.binding_main != binding.main
        || record.cancelled_at.is_none_or(|at| sample.observed_at < at)
    {
        return Err(invalid);
    }
    if observation.nodes.iter().any(|node| node.populated)
        || sample.observed_main.is_some()
        || !sample.restart_prevented
        || !receipt.whole_domain
        || !receipt.restart_prevented
        || !receipt.snapshot_safe
        || !receipt.quiescent
        || receipt.evidence == sample.basis
    {
        return Err(CpuClosureCheckError::Incomplete);
    }
    let measured = sample
        .cpu_micros
        .ok_or(CpuClosureCheckError::MissingSample)?;
    let mut debits = receipt
        .consumption
        .iter()
        .filter(|d| d.dimension == Consumption::CpuExecutionMillis);
    let final_ms = debits
        .next()
        .and_then(|d| d.measured)
        .ok_or(CpuClosureCheckError::Debit)?;
    if debits.next().is_some()
        || !record
            .intent
            .consumption
            .iter()
            .any(|value| value.dimension == Consumption::CpuExecutionMillis)
    {
        return Err(CpuClosureCheckError::Debit);
    }
    let sample_ms = measured.div_ceil(1000);
    if final_ms < sample_ms {
        return Err(CpuClosureCheckError::Undercount);
    }
    Ok(sample_ms)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kernel_usage_requires_one_complete_unsigned_counter() {
        assert_eq!(parse_cpu_usage("usage_usec 0\nuser_usec 0\n"), Ok(0));
        assert_eq!(
            parse_cpu_usage("usage_usec 18446744073709551615\n"),
            Ok(u64::MAX)
        );
        for (input, expected) in [
            ("user_usec 100\n", CounterError::Missing),
            ("usage_usec 10", CounterError::Incomplete),
            ("usage_usec\n", CounterError::Malformed),
            ("usage_usec +0\n", CounterError::Malformed),
            ("usage_usec -1\n", CounterError::Malformed),
            ("usage_usec 11 extra\n", CounterError::Malformed),
            ("usage_usec 18446744073709551616\n", CounterError::Range),
            (
                "usage_usec 0\nusage_usec invalid\n",
                CounterError::Duplicate,
            ),
            (
                "usage_usec invalid\n usage_usec 0\n",
                CounterError::Duplicate,
            ),
        ] {
            assert_eq!(parse_cpu_usage(input), Err(expected), "{input:?}");
        }
        assert_eq!(
            parse_cpu_usage(&format!("usage_usec 0\n{}\n", "x".repeat(CPU_STAT_LIMIT))),
            Err(CounterError::Incomplete)
        );
    }

    #[test]
    fn bracketed_reading_does_not_fill_gaps_or_accept_resets() {
        use CpuCounterObservation::*;
        assert_eq!(
            bracket_cpu_observation(Some(0), Some(0), false),
            LowerBoundMicros(0)
        );
        assert_eq!(
            bracket_cpu_observation(Some(9), Some(11), true),
            LowerBoundMicros(11)
        );
        assert_eq!(
            bracket_cpu_observation(Some(9), Some(11), false),
            Contradictory
        );
        assert_eq!(
            bracket_cpu_observation(Some(u64::MAX), Some(0), true),
            Contradictory
        );
        assert_eq!(bracket_cpu_observation(None, Some(0), false), Unavailable);
        assert_eq!(bracket_cpu_observation(Some(0), None, true), Unavailable);
        assert_eq!(Contradictory.lower_bound_micros(), None);
        assert_eq!(Unavailable.lower_bound_micros(), None);
        assert_eq!(
            LowerBoundMicros(u64::MAX).lower_bound_micros(),
            Some(u64::MAX)
        );
    }
}
