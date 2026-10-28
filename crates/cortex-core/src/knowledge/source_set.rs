//! Explicit JSON source-set projection for CAP-KNW-001 / 003-B.
//!
//! A collection owns the original [`ReferenceIndex`] values, never a second copy
//! of their source bytes or node indexes. Relationships add a source path to node
//! identity and borrow their occurrence, span and provenance from those indexes.
//! Construction validates producer assertions, not their authenticity or semantic
//! truth. No operation creates an assessment, permission or permanent freshness.
//!
//! Only lexical relative document paths and local pointer fragments are supported.
//! No URI decoding, normalization, `$id`, discovery or recursive expansion occurs.
//! Existing per-source targets and completeness retain their original meanings.
//!
//! `#` and `#/Id` select the same document. `common.json`, `common.json#` and
//! `models/user.json#/Id` select a document relative to the referring path's raw
//! byte parent. Document components must be nonempty and cannot be `.` or `..`;
//! `:`, `\\`, `?` and `%` are outside coverage in a document spelling. Thus schemes,
//! hosts, absolute paths, queries and encoded paths are never followed. There is
//! no filename-extension inference. An unselected document is `OutsideCoverage`.
//!
//! A single optional fragment uses the existing lexical pointer rules: empty or
//! leading `/`, with only `~0`/`~1` escapes; `%`, extra `#` separators and named
//! anchors are `UnsupportedFragment`. Pointer keys retain their exact spelling,
//! including empty keys. A missing locator in a selected document is
//! `MissingTarget`. Empty reference spelling is outside coverage. Cycles are
//! finite relationships, with no traversal or recursive evaluation.

use super::*;

/// Maximum explicitly selected sources in one collection.
pub const MAX_SOURCES: usize = 8;
/// Maximum aggregate captured bytes, in addition to every per-source limit.
pub const MAX_SET_BYTES: usize = 1024 * 1024;
/// Maximum aggregate nodes.
pub const MAX_SET_NODES: usize = 8192;
/// Maximum aggregate reference occurrences.
pub const MAX_SET_REFERENCES: usize = 2048;

/// Non-disclosing collection admission or endpoint lookup failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceSetError {
    /// At least one explicit source is required.
    Empty,
    /// Source count or aggregate resource bound exceeded.
    Limit,
    /// Two entries name the same exact source-path bytes.
    DuplicateSource,
    /// Sources do not retain exactly the same candidate, contract and captures.
    MixedContext,
    /// Capture/extraction IDs or recording times violate collection ordering.
    InvalidProvenance,
    /// A per-source target contradicts its lexical spelling or captured nodes.
    ConflictingReference,
    /// The requested source was not selected.
    MissingSource,
    /// The requested locator does not exist within the selected source.
    MissingNode,
}
impl fmt::Display for SourceSetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "knowledge source set: {self:?}")
    }
}
impl Error for SourceSetError {}

/// Borrowed endpoint: source path and locator together identify a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Endpoint<'a> {
    source: &'a ReferenceIndex,
    node: &'a KnowledgeNode,
}
impl<'a> Endpoint<'a> {
    /// Original source index, including exact candidate, bytes and provenance.
    pub fn source(self) -> &'a ReferenceIndex {
        self.source
    }
    /// Exact source identity; no replacement-character coercion.
    pub fn path(self) -> &'a SourcePath {
        self.source.path()
    }
    /// Original typed node and its byte span.
    pub fn node(self) -> &'a KnowledgeNode {
        self.node
    }
}

/// Resolution within this declared source set, separate from the per-source target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resolution<'a> {
    /// Target exists in the same exact captured context.
    Resolved(Endpoint<'a>),
    /// Unsupported, outside selected coverage or absent within a selected document.
    Unresolved(UnresolvedReference),
}

/// Borrowed original occurrence and its additive collection resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReferenceOccurrence<'a> {
    from: Endpoint<'a>,
    reference: &'a ExtractedReference,
    target: Resolution<'a>,
}
impl<'a> ReferenceOccurrence<'a> {
    /// Original referring endpoint; its node spans the original quoted JSON token.
    pub fn from(self) -> Endpoint<'a> {
        self.from
    }
    /// Original spelling, single-source target, origin and disposition.
    pub fn reference(self) -> &'a ExtractedReference {
        self.reference
    }
    /// Resolved or explicitly unresolved collection target.
    pub fn target(self) -> Resolution<'a> {
        self.target
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Target {
    Node { source: usize, node: usize },
    Unresolved(UnresolvedReference),
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct Projection {
    source: usize,
    reference: usize,
    from: usize,
    target: Target,
}

/// Immutable collection; the only source representation is its retained indexes.
#[derive(Clone, PartialEq, Eq)]
pub struct ReferenceSet {
    sources: Vec<ReferenceIndex>,
    projections: Vec<Projection>,
    id: ObservationId,
    provenance: SourceReference,
    recorded_at: SystemTime,
}
impl fmt::Debug for ReferenceSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReferenceSet")
            .field("sources", &self.sources.len())
            .field("references", &self.projections.len())
            .finish()
    }
}

fn pointer(s: &str) -> bool {
    if !s.is_empty() && !s.starts_with('/') {
        return false;
    }
    let mut bytes = s.bytes();
    while let Some(b) = bytes.next() {
        if b == b'~' && !matches!(bytes.next(), Some(b'0' | b'1')) {
            return false;
        }
    }
    true
}
fn legacy_target(source: &ReferenceIndex, spelling: &str) -> ReferenceTarget {
    match spelling.strip_prefix('#') {
        None => ReferenceTarget::Unresolved(UnresolvedReference::OutsideCoverage),
        Some(p) if p.contains('%') || !pointer(p) => {
            ReferenceTarget::Unresolved(UnresolvedReference::UnsupportedFragment)
        }
        Some(p) if source.node(p).is_err() => {
            ReferenceTarget::Unresolved(UnresolvedReference::MissingTarget)
        }
        Some(p) => ReferenceTarget::Local(p.to_owned()),
    }
}
fn node_index(source: &ReferenceIndex, locator: &str) -> Result<usize, SourceSetError> {
    source
        .nodes()
        .binary_search_by(|n| n.locator().cmp(locator))
        .map_err(|_| SourceSetError::MissingNode)
}

impl ReferenceSet {
    /// Admit a complete collection and derive all targets; no caller-provided edges.
    ///
    /// Sources are sorted by exact path bytes before validation/projection. Each
    /// must retain the identical bound context: captures n/n+1, sorted source
    /// extraction IDs n+2..n+M+1 and collection ID n+M+2, in one producer scope.
    /// Recording times must equal the collection's caller-reported time. Every
    /// per-source limit has already been enforced by [`ReferenceIndex::new`].
    ///
    /// Returns typed errors for empty/duplicate/excessive sources, mixed contexts,
    /// conflicting lexical targets and invalid provenance. No partial collection
    /// escapes. The adapter must actually acquire these sources between captures;
    /// structural admission cannot authenticate that claim or make it atomic.
    pub fn new(
        mut sources: Vec<ReferenceIndex>,
        record: ExtractionRecord,
    ) -> Result<Self, SourceSetError> {
        if sources.is_empty() {
            return Err(SourceSetError::Empty);
        }
        if sources.len() > MAX_SOURCES {
            return Err(SourceSetError::Limit);
        }
        sources.sort_by(|a, b| a.path().bytes().cmp(b.path().bytes()));
        if sources.windows(2).any(|w| w[0].path() == w[1].path()) {
            return Err(SourceSetError::DuplicateSource);
        }
        let candidate = sources[0].candidate();
        let first = candidate.baseline();
        let last = first
            .sequence()
            .checked_add(sources.len() as u64 + 2)
            .ok_or(SourceSetError::InvalidProvenance)?;
        if record.id.scope() != first.scope()
            || record.id.sequence() != last
            || candidate.observation().id().scope() != first.scope()
            || candidate.observation().id().sequence() != first.sequence() + 1
        {
            return Err(SourceSetError::InvalidProvenance);
        }
        let (mut bytes, mut nodes, mut refs) = (0usize, 0usize, 0usize);
        for (i, source) in sources.iter().enumerate() {
            if source.candidate() != candidate {
                return Err(SourceSetError::MixedContext);
            }
            if source.id().scope() != first.scope()
                || source.id().sequence() != first.sequence() + i as u64 + 2
                || source.recorded_at() != record.recorded_at
            {
                return Err(SourceSetError::InvalidProvenance);
            }
            bytes += source.bytes().len();
            nodes += source.nodes().len();
            refs += source.references().len();
        }
        // At most eight individually bounded indexes: sums cannot overflow usize.
        if bytes > MAX_SET_BYTES || nodes > MAX_SET_NODES || refs > MAX_SET_REFERENCES {
            return Err(SourceSetError::Limit);
        }
        let mut value = Self {
            sources,
            projections: Vec::with_capacity(refs),
            id: record.id,
            provenance: record.provenance,
            recorded_at: record.recorded_at,
        };
        for (i, source) in value.sources.iter().enumerate() {
            for (r, reference) in source.references().iter().enumerate() {
                if reference.target() != &legacy_target(source, reference.spelling()) {
                    return Err(SourceSetError::ConflictingReference);
                }
                value.projections.push(Projection {
                    source: i,
                    reference: r,
                    from: node_index(source, reference.from())?,
                    target: value.resolve(i, reference.spelling()),
                });
            }
        }
        Ok(value)
    }

    fn source_index(&self, path: &SourcePath) -> Result<usize, SourceSetError> {
        self.sources
            .binary_search_by(|s| s.path().bytes().cmp(path.bytes()))
            .map_err(|_| SourceSetError::MissingSource)
    }
    fn resolve(&self, referring: usize, spelling: &str) -> Target {
        use UnresolvedReference::{MissingTarget, OutsideCoverage, UnsupportedFragment};
        let (document, fragment) = spelling.split_once('#').unwrap_or((spelling, ""));
        if fragment.contains('#') || fragment.contains('%') || !pointer(fragment) {
            return Target::Unresolved(UnsupportedFragment);
        }
        let source = if document.is_empty() {
            // Empty spelling is not one of the declared reference forms.
            if !spelling.starts_with('#') {
                return Target::Unresolved(OutsideCoverage);
            }
            referring
        } else {
            if document
                .bytes()
                .any(|b| matches!(b, b':' | b'\\' | b'?' | b'%'))
                || document
                    .split('/')
                    .any(|p| p.is_empty() || p == "." || p == "..")
            {
                return Target::Unresolved(OutsideCoverage);
            }
            let path = self.sources[referring].path().bytes();
            let parent = path.iter().rposition(|b| *b == b'/').map_or(0, |i| i + 1);
            if parent + document.len() > MAX_LOCATION_BYTES {
                return Target::Unresolved(OutsideCoverage);
            }
            let mut target = path[..parent].to_vec();
            target.extend_from_slice(document.as_bytes());
            let Ok(path) = SourcePath::new(&target) else {
                return Target::Unresolved(OutsideCoverage);
            };
            let Ok(index) = self.source_index(&path) else {
                return Target::Unresolved(OutsideCoverage);
            };
            index
        };
        match node_index(&self.sources[source], fragment) {
            Ok(node) => Target::Node { source, node },
            Err(_) => Target::Unresolved(MissingTarget),
        }
    }
    fn endpoint_at(&self, source: usize, node: usize) -> Endpoint<'_> {
        Endpoint {
            source: &self.sources[source],
            node: &self.sources[source].nodes()[node],
        }
    }
    fn occurrence(&self, p: &Projection) -> ReferenceOccurrence<'_> {
        ReferenceOccurrence {
            from: self.endpoint_at(p.source, p.from),
            reference: &self.sources[p.source].references()[p.reference],
            target: match p.target {
                Target::Node { source, node } => {
                    Resolution::Resolved(self.endpoint_at(source, node))
                }
                Target::Unresolved(reason) => Resolution::Unresolved(reason),
            },
        }
    }
    /// Sources in deterministic exact-byte path order, with unchanged source indexes.
    pub fn sources(&self) -> &[ReferenceIndex] {
        &self.sources
    }
    /// Exact shared bound contract/candidate/capture context; no authority implied.
    pub fn candidate(&self) -> &BoundCandidate {
        self.sources[0].candidate()
    }
    /// Separate collection extraction identity, never a repository observation.
    pub fn id(&self) -> &ObservationId {
        &self.id
    }
    /// Collection mechanism/profile; per-source dependencies remain on each index.
    pub fn provenance(&self) -> &SourceReference {
        &self.provenance
    }
    /// Caller-reported recording instant; Core reads no clock.
    pub fn recorded_at(&self) -> SystemTime {
        self.recorded_at
    }
    /// Known endpoint; absent selected source and absent locator are distinct errors.
    pub fn endpoint(
        &self,
        path: &SourcePath,
        locator: &str,
    ) -> Result<Endpoint<'_>, SourceSetError> {
        let source = self.source_index(path)?;
        Ok(self.endpoint_at(source, node_index(&self.sources[source], locator)?))
    }
    /// All resolved/unresolved occurrences, ordered by referring path bytes then locator.
    pub fn references(&self) -> impl ExactSizeIterator<Item = ReferenceOccurrence<'_>> {
        self.projections.iter().map(|p| self.occurrence(p))
    }
    /// Borrow incoming relationships to an exact endpoint. Zero differs from missing.
    pub fn incoming(
        &self,
        path: &SourcePath,
        locator: &str,
    ) -> Result<impl Iterator<Item = ReferenceOccurrence<'_>>, SourceSetError> {
        let source = self.source_index(path)?;
        let node = node_index(&self.sources[source], locator)?;
        Ok(self
            .projections
            .iter()
            .filter(move |p| p.target == Target::Node { source, node })
            .map(|p| self.occurrence(p)))
    }
    /// Whether every occurrence resolves within the declared source set/subset.
    /// This does not redefine any original per-source completeness value.
    pub fn complete(&self) -> bool {
        self.projections
            .iter()
            .all(|p| matches!(p.target, Target::Node { .. }))
    }
    /// Existing binding semantics plus strictly post-extraction reobservation order.
    /// No supplied ID or saved snapshot establishes actual live verification.
    pub fn correspondence(
        &self,
        latest: Option<&CandidateObservation>,
    ) -> Result<BindingDecision, BindingError> {
        let captured = self.candidate().observation();
        let decision = bind_candidate(captured.contract(), Some(captured), latest)?;
        if matches!(decision, BindingDecision::Compatible(_))
            && latest.is_some_and(|o| o.id().sequence() <= self.id.sequence())
        {
            return Ok(BindingDecision::Stale(BindingStaleness::ObservationOrder));
        }
        Ok(decision)
    }
}
