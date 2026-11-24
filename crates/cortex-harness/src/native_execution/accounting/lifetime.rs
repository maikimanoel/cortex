//! Bounded source observations through the held managed cgroup incarnation.

use super::{CpuCounterObservation, bracket_cpu_observation};
use cortex_core::program::domain::{DomainId, DomainIdentity};
use cortex_storage::history_store::StoreId;
use std::time::Instant;

pub(in crate::native_execution) fn monotonic_ns() -> Option<u64> {
    let time = rustix::time::clock_gettime(rustix::time::ClockId::Monotonic);
    u64::try_from(time.tv_sec)
        .ok()?
        .checked_mul(1_000_000_000)?
        .checked_add(u64::try_from(time.tv_nsec).ok()?)
}

/// Which managed lifecycle boundary selected a counter attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManagedCpuCut {
    /// First verified held group, while the bootstrap awaits its setup.
    FirstHeld,
    /// Complete or interrupted transport return and exact tree readback.
    EndOfTransport,
    /// Exact tree readback after canonical cancellation.
    AfterCancellation,
}

/// Counter evidence for one bounded lifecycle cut.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ManagedCpuReadback {
    /// Selected cut.
    pub cut: ManagedCpuCut,
    /// Monotonic time before the first read.
    pub began_at: Instant,
    /// Monotonic time when the attempt returned.
    pub observed_at: Instant,
    /// Linux monotonic start and end in nanoseconds for cross-process matching
    /// within the same recorded boot.
    pub monotonic_ns: Option<(u64, u64)>,
    /// Exact recursive population readback when available.
    pub populated: Option<bool>,
    /// Cumulative measurement quality. No variant implies finality.
    pub counter: CpuCounterObservation,
}

/// A live bounded set of observations from one managed cgroup and store.
///
/// The first sample already includes some bootstrap activity. It is a
/// cumulative counter checkpoint. It cannot prove an empty initial domain,
/// full lifetime attribution, absence of re-entry, or terminal consumption.
pub struct ManagedCpuObservations {
    store: StoreId,
    domain: DomainId,
    identity: DomainIdentity,
    started_at: Instant,
    started_ns: Option<u64>,
    deadline: Instant,
    first_held: ManagedCpuReadback,
    end_of_transport: Option<ManagedCpuReadback>,
    after_cancellation: Option<ManagedCpuReadback>,
    last_cut: Instant,
    last_numeric: Option<u64>,
    contradictory: bool,
    incomplete: bool,
}

impl ManagedCpuObservations {
    pub(in crate::native_execution) fn new(
        store: StoreId,
        domain: DomainId,
        identity: DomainIdentity,
        started_at: Instant,
        started_ns: Option<u64>,
        deadline: Instant,
        first_held: ManagedCpuReadback,
    ) -> Self {
        let incomplete = first_held.began_at < started_at
            || first_held.observed_at < first_held.began_at
            || first_held.observed_at >= deadline
            || first_held.counter == CpuCounterObservation::Unavailable;
        Self {
            store,
            domain,
            identity,
            started_at,
            started_ns,
            deadline,
            first_held,
            end_of_transport: None,
            after_cancellation: None,
            last_cut: first_held.observed_at,
            last_numeric: first_held.counter.lower_bound_micros(),
            contradictory: first_held.counter == CpuCounterObservation::Contradictory,
            incomplete,
        }
    }

    /// Exact observed encrypted-store incarnation.
    pub fn store(&self) -> StoreId {
        self.store
    }

    /// Exact canonical attempt's domain.
    pub fn domain(&self) -> &DomainId {
        &self.domain
    }

    /// Exact held cgroup, manager invocation, pool and boot.
    pub fn identity(&self) -> &DomainIdentity {
        &self.identity
    }

    /// Original controller interval, shared by all cuts.
    pub fn interval(&self) -> (Instant, Instant) {
        (self.started_at, self.deadline)
    }

    /// Start on the Linux monotonic clock, in the recorded boot.
    pub fn started_ns(&self) -> Option<u64> {
        self.started_ns
    }

    /// The earliest held counter, potentially after bootstrap CPU was charged.
    pub fn first_held(&self) -> ManagedCpuReadback {
        self.first_held
    }

    /// Latest retained normal transport-end cut.
    pub fn end_of_transport(&self) -> Option<ManagedCpuReadback> {
        self.end_of_transport
    }

    /// Latest retained post-cancel cut. Re-observation cannot erase contradictions.
    pub fn after_cancellation(&self) -> Option<ManagedCpuReadback> {
        self.after_cancellation
    }

    /// True once overlapping or regressing readings contradict continuity.
    pub fn contradictory(&self) -> bool {
        self.contradictory
    }

    /// True when a required attempted reading was lost or outside the interval.
    pub fn incomplete(&self) -> bool {
        self.incomplete
    }

    pub(in crate::native_execution) fn record_bracket(
        &mut self,
        identity: &DomainIdentity,
        cut: ManagedCpuCut,
        mut sample: ManagedCpuReadback,
        reads: (Option<u64>, Option<u64>),
    ) -> CpuCounterObservation {
        let ManagedCpuReadback {
            began_at,
            observed_at,
            populated,
            ..
        } = sample;
        let (before, after) = reads;
        let raw = populated.map_or(CpuCounterObservation::Unavailable, |populated| {
            bracket_cpu_observation(before, after, populated)
        });
        if identity != &self.identity
            || began_at < self.last_cut
            || observed_at < began_at
            || cut == ManagedCpuCut::FirstHeld
            || (cut == ManagedCpuCut::EndOfTransport
                && (self.end_of_transport.is_some() || self.after_cancellation.is_some()))
            || raw == CpuCounterObservation::Contradictory
            || before
                .zip(self.last_numeric)
                .is_some_and(|(next, last)| next < last)
            || after
                .zip(self.last_numeric)
                .is_some_and(|(next, last)| next < last)
        {
            self.contradictory = true;
        }
        if observed_at >= self.deadline || raw == CpuCounterObservation::Unavailable {
            self.incomplete = true;
        }
        let counter = if self.contradictory {
            CpuCounterObservation::Contradictory
        } else if observed_at >= self.deadline {
            CpuCounterObservation::Unavailable
        } else {
            raw
        };
        if let Some(next) = counter.lower_bound_micros() {
            self.last_numeric = Some(next);
        }
        self.last_cut = self.last_cut.max(observed_at);
        sample.counter = counter;
        match cut {
            ManagedCpuCut::FirstHeld => (),
            ManagedCpuCut::EndOfTransport => self.end_of_transport = Some(sample),
            ManagedCpuCut::AfterCancellation => self.after_cancellation = Some(sample),
        }
        counter
    }
}
