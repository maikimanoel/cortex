//! CAP-MEM-001: bounded, in-memory canonical history, without I/O or clock sampling.
//!
//! A collection retains one supplied candidate observation. Source assertions and
//! capture IDs are not authenticated. History/query success is neither verification
//! nor permission. No persistence, live freshness, deletion or secure erasure is supplied.
//!
//! Append enforces a linear predecessor chain per entity. Queries apply an inclusive
//! recording cutoff and half-open domain interval, selecting the latest eligible
//! member of that chain. An inapplicable correction does not erase its predecessor.
//!
//! Retained values cannot be edited through the public API:
//! ```compile_fail,E0616
//! use cortex_core::temporal::RecordContent;
//! fn rewrite(content: &mut RecordContent) {
//!     content.body = "silently overwritten".into();
//! }
//! ```

mod record;

pub use record::{
    CanonicalRecord, DomainInterval, RecordContent, RecordId, RecordKind, RecordTime,
    RecordVersionId, SourceOrigin, SourceReference,
};

use crate::{
    ContractError,
    binding::{CandidateObservation, ObservationId},
};
use std::{error::Error, fmt, time::SystemTime};

/// Maximum source references per immutable version; at least one is required.
pub const MAX_SOURCE_REFERENCES: usize = 32;
/// Maximum retained versions, including superseded entries across all entities.
pub const MAX_RECORD_VERSIONS: usize = 1024;
/// Maximum retained record text/reference bytes; context and allocation overhead are separate.
pub const MAX_RETAINED_RECORD_BYTES: usize = 4 * 1024 * 1024;

/// A rejected operation, with no caller payloads in diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemporalError {
    /// Invalid bounded text or identifier; preserves the original field-level cause.
    Contract(ContractError),
    /// No declared source for a canonical assertion.
    MissingSource,
    /// More than 32 source references in a version.
    SourceLimit,
    /// An instant precedes the Unix epoch.
    InvalidTime,
    /// An empty or inverted finite domain interval.
    InvalidInterval,
    /// The operation names a different candidate capture.
    ScopeMismatch,
    /// A version ID is already retained, even for an identical submission.
    DuplicateVersion,
    /// An initial version already exists for this logical entity.
    EntityExists,
    /// A named entity is absent from this collection.
    MissingEntity,
    /// A named version is absent from this collection.
    MissingVersion,
    /// A correction names an unavailable predecessor.
    MissingPredecessor,
    /// The predecessor belongs to a different logical entity.
    EntityMismatch,
    /// A correction attempts to change a requirement into a decision or vice versa.
    CategoryMismatch,
    /// A correction names an older chain member, creating a fork.
    StalePredecessor,
    /// A correction's recording instant precedes its predecessor's.
    RecordingOrder,
    /// The retained version count would exceed its finite bound.
    VersionLimit,
    /// Retained record text/reference bytes would exceed their finite bound.
    ByteLimit,
    /// The collection could not reserve space before appending.
    AllocationFailed,
}

impl From<ContractError> for TemporalError {
    fn from(value: ContractError) -> Self {
        Self::Contract(value)
    }
}

impl fmt::Display for TemporalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Contract(cause) => return cause.fmt(f),
            Self::MissingSource => "a canonical record requires a declared source",
            Self::SourceLimit => "record exceeds 32 source references",
            Self::InvalidTime => "temporal instants must be at or after the Unix epoch",
            Self::InvalidInterval => "domain interval end must follow its start",
            Self::ScopeMismatch => "operation must name the collection's candidate capture",
            Self::DuplicateVersion => "record version identity is already retained",
            Self::EntityExists => "entity already has an initial version; append a correction",
            Self::MissingEntity => "entity is not recorded in this collection",
            Self::MissingVersion => "version is not recorded in this collection",
            Self::MissingPredecessor => "correction predecessor is not recorded in this collection",
            Self::EntityMismatch => "correction predecessor belongs to another entity",
            Self::CategoryMismatch => "correction must preserve its canonical category",
            Self::StalePredecessor => "correction must name the latest version of its entity",
            Self::RecordingOrder => "correction cannot be recorded before its predecessor",
            Self::VersionLimit => "history exceeds 1024 retained versions",
            Self::ByteLimit => "history exceeds 4 MiB of record text/reference bytes",
            Self::AllocationFailed => "history could not reserve space for the record",
        })
    }
}

impl Error for TemporalError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Contract(cause) => Some(cause),
            _ => None,
        }
    }
}

/// A scoped query with independent domain and inclusive recording-cutoff instants.
#[derive(Clone, PartialEq, Eq)]
pub struct TemporalQuery {
    context: ObservationId,
    domain_at: SystemTime,
    recorded_by: SystemTime,
}

impl TemporalQuery {
    /// Reject pre-epoch instants; domain and recording times need not be ordered.
    pub fn new(
        context: ObservationId,
        domain_at: SystemTime,
        recorded_by: SystemTime,
    ) -> Result<Self, TemporalError> {
        record::validate_instant(domain_at)?;
        record::validate_instant(recorded_by)?;
        Ok(Self {
            context,
            domain_at,
            recorded_by,
        })
    }
}

impl fmt::Debug for TemporalQuery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TemporalQuery")
            .field("domain_at", &self.domain_at)
            .field("recorded_by", &self.recorded_by)
            .finish_non_exhaustive()
    }
}

/// A borrowed selection and its exact candidate/coverage context; not live evidence.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct TemporalView<'a> {
    context: &'a CandidateObservation,
    record: Option<&'a CanonicalRecord>,
}

impl<'a> TemporalView<'a> {
    /// Inspect the original candidate, task revision, capture, coverage and uncertainty.
    pub fn context(&self) -> &'a CandidateObservation {
        self.context
    }

    /// The selected version, or no eligible version of a known entity.
    ///
    /// `None` establishes neither real-world absence nor non-applicability.
    pub fn record(&self) -> Option<&'a CanonicalRecord> {
        self.record
    }
}

impl fmt::Debug for TemporalView<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TemporalView")
            .field("matched", &self.record.is_some())
            .finish_non_exhaustive()
    }
}

/// Caller-owned canonical history, bounded across all retained versions.
///
/// Context is retained once. All accepted versions are immutable; no eviction or
/// deletion occurs. `&mut self` serializes appends without introducing synchronization.
#[derive(Clone, PartialEq, Eq)]
pub struct CanonicalHistory {
    context: CandidateObservation,
    records: Vec<CanonicalRecord>,
    retained_bytes: usize,
}

impl CanonicalHistory {
    /// Retain one supplied observation without relabeling it or asserting freshness.
    ///
    /// Dirty, detached, unborn and incomplete captures remain representable.
    pub fn new(context: CandidateObservation) -> Self {
        Self {
            context,
            records: Vec::new(),
            retained_bytes: 0,
        }
    }

    /// Inspect the retained capture explicitly; no repository revalidation occurs.
    pub fn context(&self) -> &CandidateObservation {
        &self.context
    }

    /// Inspect every retained version in accepted append order.
    pub fn records(&self) -> &[CanonicalRecord] {
        &self.records
    }

    /// Record text/reference bytes, including superseded versions.
    ///
    /// Counts UTF-8 bytes of entity/version/context/predecessor IDs, bodies and
    /// source/version/author strings. An observation ID counts scope bytes plus
    /// eight sequence bytes. Times, enum tags, allocator overhead and the separately
    /// bounded candidate observation are excluded; this is not total memory usage.
    pub fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }

    /// Validate and append a version atomically with respect to returned errors.
    ///
    /// Corrections require a fresh version ID, the latest predecessor and unchanged
    /// entity/category/capture. Recording instants cannot move backwards along a
    /// chain; equal instants use predecessor order. Initial entries for unrelated
    /// entities need no shared recording clock. Existing records are never edited.
    pub fn append(&mut self, entry: CanonicalRecord) -> Result<(), TemporalError> {
        if entry.context() != self.context.id() {
            return Err(TemporalError::ScopeMismatch);
        }
        if self.records.iter().any(|r| r.id() == entry.id()) {
            return Err(TemporalError::DuplicateVersion);
        }
        let latest = self
            .records
            .iter()
            .rev()
            .find(|r| r.entity() == entry.entity());
        match entry.predecessor() {
            None if latest.is_some() => return Err(TemporalError::EntityExists),
            None => {}
            Some(id) => {
                let previous = self
                    .records
                    .iter()
                    .find(|r| r.id() == id)
                    .ok_or(TemporalError::MissingPredecessor)?;
                if previous.entity() != entry.entity() {
                    return Err(TemporalError::EntityMismatch);
                }
                if previous.kind() != entry.kind() {
                    return Err(TemporalError::CategoryMismatch);
                }
                if latest.map(CanonicalRecord::id) != Some(previous.id()) {
                    return Err(TemporalError::StalePredecessor);
                }
                if entry.time().recorded_at() < previous.time().recorded_at() {
                    return Err(TemporalError::RecordingOrder);
                }
            }
        }
        // Fresh IDs and an existing latest predecessor also exclude cycles.
        if self.records.len() == MAX_RECORD_VERSIONS {
            return Err(TemporalError::VersionLimit);
        }
        let bytes = self
            .retained_bytes
            .checked_add(entry.retained_bytes())
            .filter(|n| *n <= MAX_RETAINED_RECORD_BYTES)
            .ok_or(TemporalError::ByteLimit)?;
        self.records
            .try_reserve(1)
            .map_err(|_| TemporalError::AllocationFailed)?;
        self.records.push(entry);
        self.retained_bytes = bytes;
        Ok(())
    }

    /// Find an immutable version, distinguishing a missing named version from no match.
    pub fn version(&self, id: &RecordVersionId) -> Result<&CanonicalRecord, TemporalError> {
        self.records
            .iter()
            .find(|r| r.id() == id)
            .ok_or(TemporalError::MissingVersion)
    }

    /// Inspect all versions of one entity in predecessor order, without allocation.
    pub fn history(&self, entity: &RecordId) -> Result<EntityHistory<'_>, TemporalError> {
        let first = self
            .records
            .iter()
            .find(|r| r.entity() == entity)
            .ok_or(TemporalError::MissingEntity)?;
        Ok(EntityHistory {
            history: self,
            entity: first.entity(),
        })
    }

    /// Select the latest eligible chain member within the supplied scope and times.
    ///
    /// A later correction only overrides within its own domain interval. A cutoff
    /// before that correction still selects its eligible predecessor. Unknown entities
    /// are errors; a known entity with no eligible version returns a view without a record.
    pub fn query(
        &self,
        entity: &RecordId,
        query: &TemporalQuery,
    ) -> Result<TemporalView<'_>, TemporalError> {
        if &query.context != self.context.id() {
            return Err(TemporalError::ScopeMismatch);
        }
        let history = self.history(entity)?;
        let record = history.records().rev().find(|r| {
            r.time().recorded_at() <= query.recorded_by
                && r.time().valid().contains(query.domain_at)
        });
        Ok(TemporalView {
            context: &self.context,
            record,
        })
    }
}

impl fmt::Debug for CanonicalHistory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CanonicalHistory")
            .field("versions", &self.records.len())
            .field("retained_bytes", &self.retained_bytes)
            .finish_non_exhaustive()
    }
}

/// A borrowed entity chain and its unchanged collection context.
#[derive(Clone, Copy)]
pub struct EntityHistory<'a> {
    history: &'a CanonicalHistory,
    entity: &'a RecordId,
}

impl<'a> EntityHistory<'a> {
    /// Logical identity of this chain.
    pub fn entity(&self) -> &'a RecordId {
        self.entity
    }

    /// Original candidate/coverage context for every version in this chain.
    pub fn context(&self) -> &'a CandidateObservation {
        self.history.context()
    }

    /// Retained versions in accepted predecessor order, bounded by 1024 entries.
    pub fn records(&self) -> impl DoubleEndedIterator<Item = &'a CanonicalRecord> + use<'a> {
        let entity = self.entity;
        self.history
            .records
            .iter()
            .filter(move |r| r.entity() == entity)
    }
}

impl fmt::Debug for EntityHistory<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EntityHistory").finish_non_exhaustive()
    }
}
