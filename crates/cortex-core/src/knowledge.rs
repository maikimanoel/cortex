//! Immutable extracted reference knowledge (first CAP-KNW-001 slice).
//! Construction validates structure, not the authenticity/truth of producer reports.
//! Queries retain the original candidate; no value grants permission or assessment.

/// Additive, candidate-bound projection over an explicit collection of source indexes.
pub mod source_set;

use crate::{binding::*, temporal::SourceReference};
use std::{
    error::Error,
    fmt,
    time::{SystemTime, UNIX_EPOCH},
};

/// Maximum captured source length, independently of Git capture limits.
pub const MAX_SOURCE_BYTES: usize = 256 * 1024;
/// Maximum nodes in one source index.
pub const MAX_NODES: usize = 4096;
/// Maximum extracted references in one source index.
pub const MAX_REFERENCES: usize = 1024;
/// Maximum path, locator or reference spelling length.
pub const MAX_LOCATION_BYTES: usize = 4096;

/// Non-disclosing knowledge construction/query failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KnowledgeError {
    /// An explicit source path is not a supported root-relative Git path.
    InvalidPath,
    /// A byte span is empty or outside the captured source.
    InvalidSpan,
    /// A locator/reference contains unsupported text.
    InvalidLocator,
    /// Input exceeds a finite byte/count bound.
    Limit,
    /// Two nodes reuse one location identity.
    DuplicateNode,
    /// A local relationship names no captured node.
    MissingNode,
    /// Extraction identity/provenance/time does not meet the contract.
    InvalidProvenance,
}
impl fmt::Display for KnowledgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "knowledge: {self:?}")
    }
}
impl Error for KnowledgeError {}

/// Root-relative Git bytes; no filesystem access or portability assertion.
#[derive(Clone, PartialEq, Eq)]
pub struct SourcePath(Vec<u8>);
impl SourcePath {
    /// Validate before allocating. Paths are literal, not Git pathspecs.
    pub fn new(path: &[u8]) -> Result<Self, KnowledgeError> {
        if path.len() > MAX_LOCATION_BYTES {
            return Err(KnowledgeError::Limit);
        }
        if path.is_empty()
            || path.iter().any(|b| b.is_ascii_control())
            || path
                .split(|b| *b == b'/')
                .any(|p| p.is_empty() || p == b"." || p == b"..")
        {
            return Err(KnowledgeError::InvalidPath);
        }
        Ok(Self(path.to_vec()))
    }
    /// Explicitly inspect potentially private source-path bytes.
    pub fn bytes(&self) -> &[u8] {
        &self.0
    }
}
impl fmt::Debug for SourcePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SourcePath([private])")
    }
}

/// Exact half-open byte range in a captured source, not a character offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceSpan {
    start: usize,
    end: usize,
}
impl SourceSpan {
    /// Validate ordering; index construction also checks the source extent.
    pub fn new(start: usize, end: usize) -> Result<Self, KnowledgeError> {
        if start >= end {
            return Err(KnowledgeError::InvalidSpan);
        }
        Ok(Self { start, end })
    }
    /// Inclusive byte offset.
    pub fn start(self) -> usize {
        self.start
    }
    /// Exclusive byte offset.
    pub fn end(self) -> usize {
        self.end
    }
}
fn location(s: &str) -> Result<(), KnowledgeError> {
    if s.len() > MAX_LOCATION_BYTES {
        return Err(KnowledgeError::Limit);
    }
    if s.chars().any(char::is_control) {
        return Err(KnowledgeError::InvalidLocator);
    }
    Ok(())
}
/// An adapter-defined location within one source. Empty locator denotes its root.
#[derive(Clone, PartialEq, Eq)]
pub struct KnowledgeNode {
    locator: String,
    span: SourceSpan,
}
impl KnowledgeNode {
    /// Validate bounded location metadata, without parsing the source.
    pub fn new(locator: &str, span: SourceSpan) -> Result<Self, KnowledgeError> {
        location(locator)?;
        Ok(Self {
            locator: locator.to_owned(),
            span,
        })
    }
    /// Explicit source location; interpretation belongs to the named adapter.
    pub fn locator(&self) -> &str {
        &self.locator
    }
    /// Captured source range.
    pub fn span(&self) -> SourceSpan {
        self.span
    }
}
impl fmt::Debug for KnowledgeNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("KnowledgeNode([private])")
    }
}
/// Why a spelling cannot be resolved within this source's declared coverage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnresolvedReference {
    /// Spelling is outside the adapter's supported local-reference forms.
    OutsideCoverage,
    /// Fragment syntax/encoding is outside the adapter's supported subset.
    UnsupportedFragment,
    /// A well-formed local location has no captured target.
    MissingTarget,
}
/// A source relationship target, separate from validation or authority.
#[derive(Clone, PartialEq, Eq)]
pub enum ReferenceTarget {
    /// A location that must exist in this index.
    Local(String),
    /// Unresolved spelling is retained on the reference, never an invented node.
    Unresolved(UnresolvedReference),
}
impl fmt::Debug for ReferenceTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Local(_) => f.write_str("Local([private])"),
            Self::Unresolved(r) => r.fmt(f),
        }
    }
}
/// Only deterministic extraction is implemented by this slice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelationOrigin {
    /// Observed source syntax, not inferred semantics.
    Extracted,
}
/// No evidence-validation/promotion operation is implemented by this slice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationDisposition {
    /// Extraction alone does not validate a relation.
    Unverified,
}
/// Immutable extracted occurrence. Source syntax is not proof of semantic truth.
#[derive(Clone, PartialEq, Eq)]
pub struct ExtractedReference {
    from: String,
    spelling: String,
    target: ReferenceTarget,
}
impl ExtractedReference {
    /// Validate bounded producer-supplied metadata; index checks all endpoints.
    pub fn new(
        from: &str,
        spelling: &str,
        target: ReferenceTarget,
    ) -> Result<Self, KnowledgeError> {
        location(from)?;
        location(spelling)?;
        if let ReferenceTarget::Local(t) = &target {
            location(t)?;
        }
        Ok(Self {
            from: from.to_owned(),
            spelling: spelling.to_owned(),
            target,
        })
    }
    /// Location of the source occurrence (whose span is available through node()).
    pub fn from(&self) -> &str {
        &self.from
    }
    /// Explicit, potentially private reference spelling.
    pub fn spelling(&self) -> &str {
        &self.spelling
    }
    /// Resolved or explicitly unresolved target.
    pub fn target(&self) -> &ReferenceTarget {
        &self.target
    }
    /// Extraction origin cannot be promoted through this API.
    pub fn origin(&self) -> RelationOrigin {
        RelationOrigin::Extracted
    }
    /// There is no evidence input or validation promotion in this slice.
    pub fn disposition(&self) -> ValidationDisposition {
        ValidationDisposition::Unverified
    }
}
impl fmt::Debug for ExtractedReference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ExtractedReference([private])")
    }
}
/// A bounded immutable source index, retaining the exact binding and provenance.
#[derive(Clone, PartialEq, Eq)]
pub struct ReferenceIndex {
    candidate: BoundCandidate,
    path: SourcePath,
    bytes: Vec<u8>,
    id: ObservationId,
    provenance: SourceReference,
    recorded_at: SystemTime,
    nodes: Vec<KnowledgeNode>,
    references: Vec<ExtractedReference>,
}
/// Separate extraction identity/profile and caller-reported recording instant.
pub struct ExtractionRecord {
    id: ObservationId,
    provenance: SourceReference,
    recorded_at: SystemTime,
}
impl ExtractionRecord {
    /// Reuse Core's observation-reference provenance; constructor authenticates nothing.
    pub fn new(
        id: ObservationId,
        adapter: &str,
        version: &str,
        recorded_at: SystemTime,
    ) -> Result<Self, KnowledgeError> {
        if recorded_at < UNIX_EPOCH {
            return Err(KnowledgeError::InvalidProvenance);
        }
        let provenance = SourceReference::observation_reference(id.clone(), adapter, version)
            .map_err(|_| KnowledgeError::InvalidProvenance)?;
        Ok(Self {
            id,
            provenance,
            recorded_at,
        })
    }
}
impl ReferenceIndex {
    /// Admit one complete bounded batch. Failure publishes no partial index.
    pub fn new(
        candidate: BoundCandidate,
        path: SourcePath,
        bytes: Vec<u8>,
        record: ExtractionRecord,
        mut nodes: Vec<KnowledgeNode>,
        mut references: Vec<ExtractedReference>,
    ) -> Result<Self, KnowledgeError> {
        if bytes.len() > MAX_SOURCE_BYTES
            || nodes.len() > MAX_NODES
            || references.len() > MAX_REFERENCES
        {
            return Err(KnowledgeError::Limit);
        }
        if record.id == *candidate.observation().id() || record.id == *candidate.baseline() {
            return Err(KnowledgeError::InvalidProvenance);
        }
        let mut size = 0usize;
        for node in &nodes {
            if node.span.end > bytes.len() {
                return Err(KnowledgeError::InvalidSpan);
            }
            size = size
                .checked_add(node.locator.len())
                .ok_or(KnowledgeError::Limit)?;
            if size > MAX_SOURCE_BYTES {
                return Err(KnowledgeError::Limit);
            }
        }
        nodes.sort_by(|a, b| a.locator.cmp(&b.locator));
        if nodes.windows(2).any(|w| w[0].locator == w[1].locator) {
            return Err(KnowledgeError::DuplicateNode);
        }
        let exists = |key: &str| {
            nodes
                .binary_search_by(|n| n.locator.as_str().cmp(key))
                .is_ok()
        };
        if !exists("") {
            return Err(KnowledgeError::MissingNode);
        }
        for r in &references {
            if !exists(&r.from) || matches!(&r.target,ReferenceTarget::Local(t) if !exists(t)) {
                return Err(KnowledgeError::MissingNode);
            }
        }
        references.sort_by(|a, b| a.from.cmp(&b.from));
        if references.windows(2).any(|w| w[0].from == w[1].from) {
            return Err(KnowledgeError::DuplicateNode);
        }
        Ok(Self {
            candidate,
            path,
            bytes,
            id: record.id,
            provenance: record.provenance,
            recorded_at: record.recorded_at,
            nodes,
            references,
        })
    }
    /// Actual capture/binding reported by the producer; grants no permission.
    pub fn candidate(&self) -> &BoundCandidate {
        &self.candidate
    }
    /// Separate extraction observation identity.
    pub fn id(&self) -> &ObservationId {
        &self.id
    }
    /// Exact adapter/parser/rules reference, not authenticated provenance.
    pub fn provenance(&self) -> &SourceReference {
        &self.provenance
    }
    /// Caller-reported instant; never determines freshness by itself.
    pub fn recorded_at(&self) -> SystemTime {
        self.recorded_at
    }
    /// Source identity relative to the retained candidate.
    pub fn path(&self) -> &SourcePath {
        &self.path
    }
    /// Explicitly inspect the captured source; privacy policy still applies.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    /// Find a typed node without reparsing or performing I/O.
    pub fn node(&self, key: &str) -> Result<&KnowledgeNode, KnowledgeError> {
        self.nodes
            .binary_search_by(|n| n.locator.as_str().cmp(key))
            .map(|i| &self.nodes[i])
            .map_err(|_| KnowledgeError::MissingNode)
    }
    /// All locations in deterministic lexical order.
    pub fn nodes(&self) -> &[KnowledgeNode] {
        &self.nodes
    }
    /// All occurrences in deterministic source-locator order.
    pub fn references(&self) -> &[ExtractedReference] {
        &self.references
    }
    /// Incoming resolved relationships to a known location; zero differs from missing node.
    pub fn incoming<'a>(
        &'a self,
        target: &'a str,
    ) -> Result<impl Iterator<Item = &'a ExtractedReference>, KnowledgeError> {
        self.node(target)?;
        Ok(self
            .references
            .iter()
            .filter(move |r| matches!(&r.target,ReferenceTarget::Local(t) if t==target)))
    }
    /// Whether every extracted spelling resolved within this source's declared subset.
    pub fn complete(&self) -> bool {
        self.references
            .iter()
            .all(|r| matches!(r.target, ReferenceTarget::Local(_)))
    }
    /// Reuse existing candidate correspondence without revalidating a live repository.
    pub fn correspondence(
        &self,
        latest: Option<&CandidateObservation>,
    ) -> Result<BindingDecision, BindingError> {
        if latest.is_some_and(|observed| observed.id() == &self.id) {
            return Ok(BindingDecision::Stale(BindingStaleness::ObservationOrder));
        }
        let captured = self.candidate.observation();
        bind_candidate(captured.contract(), Some(captured), latest)
    }
}
impl fmt::Debug for ReferenceIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReferenceIndex")
            .field("nodes", &self.nodes.len())
            .field("references", &self.references.len())
            .finish()
    }
}
