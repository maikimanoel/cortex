//! CAP-MEM-001: finite domain-time timelines at one inclusive recording cutoff.
//!
//! Selection delegates to [`CanonicalHistory::query`]. A timeline borrows retained
//! records and their original context; it performs no I/O, clock sampling, mutation
//! or live revalidation. Historical selection establishes no CURRENT validity,
//! assessment, source authentication or permission. Absence means only that a known
//! entity has no eligible retained version in that segment.

use super::{
    CanonicalHistory, CanonicalRecord, DomainInterval, RecordId, TemporalError, TemporalQuery,
};
use crate::binding::{CandidateObservation, ObservationId};
use std::{error::Error, fmt, time::SystemTime};

/// A rejected timeline request, without caller payloads in diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimelineError {
    /// An existing time, interval, entity or context validation error.
    Temporal(TemporalError),
    /// Space for bounded endpoints or segments could not be reserved.
    AllocationFailed,
}

impl From<TemporalError> for TimelineError {
    fn from(value: TemporalError) -> Self {
        Self::Temporal(value)
    }
}

impl fmt::Display for TimelineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Temporal(cause) => cause.fmt(f),
            Self::AllocationFailed => f.write_str("timeline could not reserve space for its view"),
        }
    }
}

impl Error for TimelineError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Temporal(cause) => Some(cause),
            Self::AllocationFailed => None,
        }
    }
}

/// One validated finite, nonempty `[from, until)` request in an explicit context.
#[derive(Debug, PartialEq, Eq)]
pub struct TimelineQuery {
    point: TemporalQuery,
    interval: DomainInterval,
}

impl TimelineQuery {
    /// Require all instants at/after the epoch and `from < until`.
    ///
    /// Domain time and the inclusive recording cutoff are independent. Context is
    /// an unauthenticated capture reference, with the same meaning as in a point query.
    pub fn new(
        context: ObservationId,
        from: SystemTime,
        until: SystemTime,
        recorded_by: SystemTime,
    ) -> Result<Self, TimelineError> {
        let interval = DomainInterval::new(from, Some(until))?;
        let point = TemporalQuery::new(context, from, recorded_by)?;
        Ok(Self { point, interval })
    }
}

/// A nonempty half-open segment borrowing a selected version or explicit absence.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct TimelineSegment<'a> {
    from: SystemTime,
    until: SystemTime,
    record: Option<&'a CanonicalRecord>,
}

impl<'a> TimelineSegment<'a> {
    /// Inclusive domain start, with no timestamp conversion or truncation.
    pub fn from(&self) -> SystemTime {
        self.from
    }

    /// Finite exclusive domain end, strictly after the start.
    pub fn until(&self) -> SystemTime {
        self.until
    }

    /// The original selected version; `None` means no eligible retained record.
    ///
    /// This does not establish real-world absence, privacy gaps or non-applicability.
    pub fn record(&self) -> Option<&'a CanonicalRecord> {
        self.record
    }
}

impl fmt::Debug for TimelineSegment<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TimelineSegment")
            .field("from", &self.from)
            .field("until", &self.until)
            .field("matched", &self.record.is_some())
            .finish_non_exhaustive()
    }
}

/// An ordered, disjoint, coalesced partition of exactly the requested interval.
#[derive(PartialEq, Eq)]
pub struct TimelineView<'a> {
    context: &'a CandidateObservation,
    entity: &'a RecordId,
    interval: DomainInterval,
    recorded_by: SystemTime,
    segments: Vec<TimelineSegment<'a>>,
}

impl<'a> TimelineView<'a> {
    /// The unchanged retained candidate, task, coverage and capture context.
    pub fn context(&self) -> &'a CandidateObservation {
        self.context
    }

    /// The retained logical entity identity, distinct from selected version IDs.
    pub fn entity(&self) -> &'a RecordId {
        self.entity
    }

    /// The requested nonempty domain interval; its end is always finite.
    pub fn interval(&self) -> DomainInterval {
        self.interval
    }

    /// Inclusive recording cutoff used for every segment, without sampling a clock.
    pub fn recorded_by(&self) -> SystemTime {
        self.recorded_by
    }

    /// Inspect the complete partition, including uncovered segments.
    pub fn segments(&self) -> &[TimelineSegment<'a>] {
        &self.segments
    }
}

impl fmt::Debug for TimelineView<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TimelineView")
            .field("interval", &self.interval)
            .field("recorded_by", &self.recorded_by)
            .field("segment_count", &self.segments.len())
            .finish_non_exhaustive()
    }
}

/// Query a known entity over one finite interval at an inclusive recording cutoff.
///
/// Every segment agrees with [`CanonicalHistory::query`] throughout its half-open
/// interval. Equal recording instants follow predecessor order, and an inapplicable
/// correction leaves eligible earlier versions available. Wrong context and unknown
/// entity retain their existing typed errors, wrapped in [`TimelineError::Temporal`].
/// The query is consumed so its validated point request can be reused without
/// allocating copies of the capture reference.
///
/// At most two endpoints per eligible retained version plus the window boundaries
/// are considered: with the existing 1024-version bound, at most 2049 segments
/// precede coalescing. Endpoint sorting is in place; all vector capacity is reserved
/// fallibly before insertion. Point queries bound traversal to O(V²) for V retained
/// versions; these are logical work bounds, not total-RSS or latency guarantees.
pub fn query_timeline<'a>(
    history: &'a CanonicalHistory,
    entity: &RecordId,
    mut query: TimelineQuery,
) -> Result<TimelineView<'a>, TimelineError> {
    // Use the point API's context-before-entity error order, before allocating.
    let mut selected = history.query(entity, &query.point)?.record();
    let chain = history.history(entity)?;
    let recorded_by = query.point.recorded_by;
    let eligible = || {
        chain
            .records()
            .filter(|record| record.time().recorded_at() <= recorded_by)
    };
    // The immutable history's validated bound limits this to 2050 endpoints.
    let capacity = eligible().count() * 2 + 2;
    let mut endpoints = Vec::new();
    endpoints
        .try_reserve_exact(capacity)
        .map_err(|_| TimelineError::AllocationFailed)?;
    let from = query.interval.from();
    // A finite end is guaranteed by TimelineQuery::new. Keep the original end
    // in a local without relying on an unwrap or timestamp arithmetic.
    let until = match query.interval.until() {
        Some(until) => until,
        None => return Err(TemporalError::InvalidInterval.into()),
    };
    endpoints.push(from);
    endpoints.push(until);
    for record in eligible() {
        let valid = record.time().valid();
        if from < valid.from() && valid.from() < until {
            endpoints.push(valid.from());
        }
        if let Some(end) = valid.until()
            && from < end
            && end < until
        {
            endpoints.push(end);
        }
    }
    endpoints.sort_unstable();
    endpoints.dedup();

    let mut segments: Vec<TimelineSegment<'a>> = Vec::new();
    segments
        .try_reserve_exact(endpoints.len() - 1)
        .map_err(|_| TimelineError::AllocationFailed)?;
    for (index, pair) in endpoints.windows(2).enumerate() {
        if index != 0 {
            // Endpoints are already validated domain instants. Only this owned
            // point request changes; the history and retained records are borrowed.
            query.point.domain_at = pair[0];
            selected = history.query(entity, &query.point)?.record();
        }
        if let Some(previous) = segments.last_mut()
            && previous.record.map(CanonicalRecord::id) == selected.map(CanonicalRecord::id)
        {
            previous.until = pair[1];
        } else {
            segments.push(TimelineSegment {
                from: pair[0],
                until: pair[1],
                record: selected,
            });
        }
    }
    Ok(TimelineView {
        context: history.context(),
        entity: chain.entity(),
        interval: query.interval,
        recorded_by,
        segments,
    })
}
