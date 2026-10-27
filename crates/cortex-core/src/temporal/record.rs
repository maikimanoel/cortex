use super::{MAX_SOURCE_REFERENCES, TemporalError};
use crate::{binding::ObservationId, validate_identifier, validate_text};
use std::{fmt, time::SystemTime};

/// A logical canonical entity, using the 1..=128-byte ASCII [`crate::TaskId`] syntax.
///
/// This is not a task, permission, version or globally registered identity.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct RecordId(String);

impl RecordId {
    /// Validate a logical entity ID before allocating its representation.
    pub fn new(value: &str) -> Result<Self, TemporalError> {
        validate_identifier("record_id", value)?;
        Ok(Self(value.to_owned()))
    }

    /// Read the caller-supplied ID without normalization.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for RecordId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RecordId([redacted])")
    }
}

/// An immutable version ID, distinct from its logical [`RecordId`].
///
/// Uses the same syntax as `RecordId`; uniqueness is checked within a collection.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct RecordVersionId(String);

impl RecordVersionId {
    /// Validate a version ID before allocating its representation.
    pub fn new(value: &str) -> Result<Self, TemporalError> {
        validate_identifier("record_version_id", value)?;
        Ok(Self(value.to_owned()))
    }

    /// Read the caller-supplied version ID.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for RecordVersionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RecordVersionId([redacted])")
    }
}

/// The canonical categories admitted by this slice; neither grants authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordKind {
    /// A stated requirement, not evidence that it was satisfied.
    Requirement,
    /// A historical decision, not permission for a new action.
    Decision,
}

/// Caller-reported origin of a source reference, without authentication.
#[derive(Clone, PartialEq, Eq)]
pub enum SourceOrigin {
    /// An operator assertion; the author string is not an authenticated principal.
    OperatorReported {
        /// Caller-supplied author description.
        author: String,
    },
    /// A reference to an observation; Core does not resolve or authenticate it.
    ObservationReference(ObservationId),
}

impl fmt::Debug for SourceOrigin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::OperatorReported { .. } => "OperatorReported([redacted])",
            Self::ObservationReference(_) => "ObservationReference([redacted])",
        })
    }
}

/// A bounded source/version reference, not Evidence verification provenance.
#[derive(Clone, PartialEq, Eq)]
pub struct SourceReference {
    origin: SourceOrigin,
    source: String,
    version: String,
}

impl SourceReference {
    /// Record an explicit operator assertion, without inventing an observation.
    ///
    /// Each string follows the nonblank, 4096-byte contract text rules.
    pub fn operator_reported(
        author: &str,
        source: &str,
        version: &str,
    ) -> Result<Self, TemporalError> {
        validate_text("source_author", author)?;
        validate_text("source", source)?;
        validate_text("source_version", version)?;
        Ok(Self {
            origin: SourceOrigin::OperatorReported {
                author: author.to_owned(),
            },
            source: source.to_owned(),
            version: version.to_owned(),
        })
    }

    /// Retain an observation reference and its declared source/version.
    ///
    /// The referenced observation need not be the collection's candidate capture;
    /// existence and authenticity remain caller obligations.
    pub fn observation_reference(
        observation: ObservationId,
        source: &str,
        version: &str,
    ) -> Result<Self, TemporalError> {
        validate_text("source", source)?;
        validate_text("source_version", version)?;
        Ok(Self {
            origin: SourceOrigin::ObservationReference(observation),
            source: source.to_owned(),
            version: version.to_owned(),
        })
    }

    /// Inspect the declared origin without upgrading its trust.
    pub fn origin(&self) -> &SourceOrigin {
        &self.origin
    }

    /// Read the source description explicitly.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Read the source version explicitly; this is not a storage format version.
    pub fn version(&self) -> &str {
        &self.version
    }

    fn retained_bytes(&self) -> usize {
        self.source.len()
            + self.version.len()
            + match &self.origin {
                SourceOrigin::OperatorReported { author } => author.len(),
                SourceOrigin::ObservationReference(id) => observation_bytes(id),
            }
    }
}

impl fmt::Debug for SourceReference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SourceReference([redacted])")
    }
}

/// Validated body and 1..=32 explicit source references, retained without rewriting.
#[derive(Clone, PartialEq, Eq)]
pub struct RecordContent {
    body: String,
    sources: Vec<SourceReference>,
}

impl RecordContent {
    /// Reject blank/control-containing/overlong bodies and missing/excess sources.
    pub fn new(body: &str, sources: Vec<SourceReference>) -> Result<Self, TemporalError> {
        validate_text("record_body", body)?;
        if sources.is_empty() {
            return Err(TemporalError::MissingSource);
        }
        if sources.len() > MAX_SOURCE_REFERENCES {
            return Err(TemporalError::SourceLimit);
        }
        Ok(Self {
            body: body.to_owned(),
            sources,
        })
    }

    /// Read the supplied body explicitly; this does not disclose it by default.
    pub fn body(&self) -> &str {
        &self.body
    }

    /// Inspect source references in their original input order.
    pub fn sources(&self) -> &[SourceReference] {
        &self.sources
    }
}

impl fmt::Debug for RecordContent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RecordContent")
            .field("source_count", &self.sources.len())
            .finish_non_exhaustive()
    }
}

/// A nonempty half-open domain interval `[from, until)`, optionally without an end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DomainInterval {
    from: SystemTime,
    until: Option<SystemTime>,
}

impl DomainInterval {
    /// Require instants at/after the Unix epoch and a finite end strictly after start.
    pub fn new(from: SystemTime, until: Option<SystemTime>) -> Result<Self, TemporalError> {
        validate_instant(from)?;
        if let Some(end) = until {
            validate_instant(end)?;
            if end <= from {
                return Err(TemporalError::InvalidInterval);
            }
        }
        Ok(Self { from, until })
    }

    /// Inclusive domain start.
    pub fn from(&self) -> SystemTime {
        self.from
    }

    /// Exclusive domain end; `None` denotes an open end.
    pub fn until(&self) -> Option<SystemTime> {
        self.until
    }

    /// Test interval membership, without sampling a clock.
    pub fn contains(&self, instant: SystemTime) -> bool {
        instant >= self.from && self.until.is_none_or(|end| instant < end)
    }
}

/// Independent domain validity and caller-supplied recording time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordTime {
    valid: DomainInterval,
    recorded_at: SystemTime,
}

impl RecordTime {
    /// Require a recording instant at/after the epoch, unrelated to domain validity.
    pub fn new(valid: DomainInterval, recorded_at: SystemTime) -> Result<Self, TemporalError> {
        validate_instant(recorded_at)?;
        Ok(Self { valid, recorded_at })
    }

    /// Inspect the domain interval.
    pub fn valid(&self) -> DomainInterval {
        self.valid
    }

    /// Inspect the caller-supplied recording instant.
    pub fn recorded_at(&self) -> SystemTime {
        self.recorded_at
    }
}

/// One immutable canonical version, admitted only through a history's validation.
///
/// Construction validates local values; [`super::CanonicalHistory::append`] checks
/// context, uniqueness, predecessor, category, ordering and collection bounds.
#[derive(Clone, PartialEq, Eq)]
pub struct CanonicalRecord {
    entity: RecordId,
    id: RecordVersionId,
    kind: RecordKind,
    context: ObservationId,
    content: RecordContent,
    time: RecordTime,
    predecessor: Option<RecordVersionId>,
}

impl CanonicalRecord {
    /// Describe a new logical entity; append rejects a second initial version.
    pub fn initial(
        entity: RecordId,
        id: RecordVersionId,
        kind: RecordKind,
        context: ObservationId,
        content: RecordContent,
        time: RecordTime,
    ) -> Self {
        Self {
            entity,
            id,
            kind,
            context,
            content,
            time,
            predecessor: None,
        }
    }

    /// Describe an appended correction, preserving its predecessor for validation.
    pub fn correction(
        entity: RecordId,
        id: RecordVersionId,
        kind: RecordKind,
        context: ObservationId,
        predecessor: RecordVersionId,
        content: RecordContent,
        time: RecordTime,
    ) -> Self {
        Self {
            entity,
            id,
            kind,
            context,
            content,
            time,
            predecessor: Some(predecessor),
        }
    }

    /// Logical entity identity.
    pub fn entity(&self) -> &RecordId {
        &self.entity
    }

    /// Immutable version identity.
    pub fn id(&self) -> &RecordVersionId {
        &self.id
    }

    /// Canonical category, fixed throughout an entity's chain.
    pub fn kind(&self) -> RecordKind {
        self.kind
    }

    /// Exact collection capture identity; not a complete dirty-tree identity.
    pub fn context(&self) -> &ObservationId {
        &self.context
    }

    /// Explicit access to the body and provenance references.
    pub fn content(&self) -> &RecordContent {
        &self.content
    }

    /// Domain and recording times.
    pub fn time(&self) -> RecordTime {
        self.time
    }

    /// Previous version, or `None` for an initial version.
    pub fn predecessor(&self) -> Option<&RecordVersionId> {
        self.predecessor.as_ref()
    }

    pub(super) fn retained_bytes(&self) -> usize {
        // Every component has a finite constructor bound; this sum is below
        // 400 KiB and cannot overflow usize on supported Rust targets.
        self.entity.0.len()
            + self.id.0.len()
            + observation_bytes(&self.context)
            + self.predecessor.as_ref().map_or(0, |id| id.0.len())
            + self.content.body.len()
            + self
                .content
                .sources
                .iter()
                .map(SourceReference::retained_bytes)
                .sum::<usize>()
    }
}

impl fmt::Debug for CanonicalRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CanonicalRecord")
            .field("kind", &self.kind)
            .field("correction", &self.predecessor.is_some())
            .finish_non_exhaustive()
    }
}

fn observation_bytes(id: &ObservationId) -> usize {
    id.scope().len() + size_of::<u64>()
}

pub(super) fn validate_instant(instant: SystemTime) -> Result<(), TemporalError> {
    if instant < SystemTime::UNIX_EPOCH {
        return Err(TemporalError::InvalidTime);
    }
    Ok(())
}
