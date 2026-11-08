//! Independent dimensions of bounded lexical knowledge inspection.
//!
//! These borrowed results describe retained producer assertions. They neither
//! authenticate extraction nor validate semantics, acquire sources or grant authority.

use super::{
    ExtractedReference, KnowledgeError, KnowledgeNode, ReferenceIndex, ReferenceTarget, SourcePath,
    SourceSpan, UnresolvedReference,
    source_set::{ReferenceSet, Resolution},
};
use std::fmt;

/// In-memory inspection API contract version; not a persistent encoding version.
pub const INSPECTION_CONTRACT_VERSION: u32 = 2;

/// The implemented extraction subset, independently of emitted-reference resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtractionProfile {
    /// JSON values and literal `$ref` occurrences in one source.
    JsonReferences,
    /// The same extraction plus lexical resolution across explicitly selected JSON sources.
    JsonSourceSet,
    /// Rust item/import/module/macro occurrences without language name resolution.
    RustLexical,
    /// Native Markdown headings and supported links, excluding code regions.
    MarkdownLexical,
    /// Cargo manifest declarations and explicit feature-reference strings.
    CargoManifestLexical,
}
impl ExtractionProfile {
    /// Deliberate omissions of this exact supported subset, not a measured count.
    /// Encountered input outside the parser's accepted syntax still rejects extraction.
    pub fn omitted_constructs(self) -> &'static [&'static str] {
        match self {
            Self::JsonReferences | Self::JsonSourceSet => &[
                "JSON Schema dialect and dynamic-reference semantics",
                "relationships other than literal $ref members",
                "unselected documents and remote target acquisition",
            ],
            Self::RustLexical => &[
                "fields, variants, parameters, expressions and signature-type references",
                "attribute interpretation and macro expansion/token-tree contents",
                "conditional compilation and cross-file Rust name resolution",
            ],
            Self::MarkdownLexical => &[
                "fenced, indented and inline code-region contents",
                "renderer-specific heading/link semantics",
                "external file and URL target acquisition",
            ],
            Self::CargoManifestLexical => &[
                "effective dependency resolution and feature activation",
                "workspace inheritance and conditional configuration evaluation",
                "relations outside explicit feature and optional-dependency declarations",
            ],
        }
    }
}

/// Coverage describes a bounded subset, never complete extraction from an empty result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtractionCoverage {
    /// This implementation recognizes the retained exact profile and its omissions.
    Bounded(ExtractionProfile),
    /// The retained profile is not supported by the inspecting adapter.
    Unestablished,
}

/// Universe over which the emitted-reference resolution predicate was computed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionScope {
    /// Targets within each original source snapshot.
    SingleSource,
    /// Targets within one explicitly selected, candidate-bound collection.
    SelectedSourceSet,
}

/// Resolution of emitted occurrences only; no inference about unextracted syntax.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReferenceResolution {
    scope: ResolutionScope,
    emitted: usize,
    resolved: usize,
    missing_target: usize,
    outside_coverage: usize,
    unsupported_fragment: usize,
}
impl ReferenceResolution {
    fn new(scope: ResolutionScope) -> Self {
        Self {
            scope,
            emitted: 0,
            resolved: 0,
            missing_target: 0,
            outside_coverage: 0,
            unsupported_fragment: 0,
        }
    }
    fn add(&mut self, unresolved: Option<UnresolvedReference>) {
        // Both constructors consume indexes with fixed finite admission bounds.
        self.emitted += 1;
        match unresolved {
            None => self.resolved += 1,
            Some(UnresolvedReference::MissingTarget) => self.missing_target += 1,
            Some(UnresolvedReference::OutsideCoverage) => self.outside_coverage += 1,
            Some(UnresolvedReference::UnsupportedFragment) => self.unsupported_fragment += 1,
        }
    }
    /// Summarize the original snapshot without changing its targets.
    pub fn for_index(index: &ReferenceIndex) -> Self {
        let mut result = Self::new(ResolutionScope::SingleSource);
        for reference in index.references() {
            result.add(match reference.target() {
                ReferenceTarget::Local(_) => None,
                ReferenceTarget::Unresolved(reason) => Some(*reason),
            });
        }
        result
    }
    /// Summarize collection resolution, which can differ from individual snapshots.
    pub fn for_set(set: &ReferenceSet) -> Self {
        let mut result = Self::new(ResolutionScope::SelectedSourceSet);
        for reference in set.references() {
            result.add(match reference.target() {
                Resolution::Resolved(_) => None,
                Resolution::Unresolved(reason) => Some(reason),
            });
        }
        result
    }
    /// The selected resolution universe.
    pub fn scope(self) -> ResolutionScope {
        self.scope
    }
    /// Actual emitted occurrence count, including unresolved occurrences.
    pub fn emitted(self) -> usize {
        self.emitted
    }
    /// Occurrences with a target in this universe.
    pub fn resolved(self) -> usize {
        self.resolved
    }
    /// Well-formed supported target absent within selected coverage.
    pub fn missing_target(self) -> usize {
        self.missing_target
    }
    /// Occurrences outside selected or supported target coverage.
    pub fn outside_coverage(self) -> usize {
        self.outside_coverage
    }
    /// Occurrences with unsupported fragment syntax.
    pub fn unsupported_fragment(self) -> usize {
        self.unsupported_fragment
    }
    /// Vacuously true for a valid empty index/set; does not establish coverage.
    pub fn all_references_resolved(self) -> bool {
        self.emitted == self.resolved
    }
}

/// A specific question about a retained reference occurrence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceClaimKind {
    /// This emitted occurrence is retained at the requested source location.
    LexicalOccurrence,
    /// The occurrence has a target within the declared resolution universe.
    LexicalTargetResolution,
    /// The relationship is semantically true; no such mechanism is implemented.
    SemanticRelationship,
    /// Extraction includes every possible relation; no such mechanism is implemented.
    CompleteExtraction,
}

/// Bounded source/locator query. It is not an assertion of authenticity or authority.
#[derive(Clone, Copy)]
pub struct ReferenceClaim<'a> {
    kind: ReferenceClaimKind,
    source: &'a SourcePath,
    locator: &'a str,
}
impl<'a> ReferenceClaim<'a> {
    /// Validate query metadata; existence is checked against the retained projection.
    pub fn new(
        kind: ReferenceClaimKind,
        source: &'a SourcePath,
        locator: &'a str,
    ) -> Result<Self, KnowledgeError> {
        super::location(locator)?;
        Ok(Self {
            kind,
            source,
            locator,
        })
    }
    /// Requested proposition, independent of target resolution and coverage.
    pub fn kind(self) -> ReferenceClaimKind {
        self.kind
    }
    /// Explicit source path of the occurrence.
    pub fn source(self) -> &'a SourcePath {
        self.source
    }
    /// Explicit adapter-owned occurrence locator, not a stable semantic symbol ID.
    pub fn locator(self) -> &'a str {
        self.locator
    }
}
impl fmt::Debug for ReferenceClaim<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReferenceClaim")
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}

/// Why available facts do not establish an otherwise implemented lexical claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimGap {
    /// This adapter does not support the retained producer/profile correspondence.
    ProfileMismatch,
    /// Supplied source correspondence is stale, incompatible or unestablished.
    SourceCorrespondence,
    /// The requested source was not part of the retained projection.
    SourceNotSelected,
    /// No matching occurrence was emitted; absence is not extraction completeness.
    OccurrenceNotRetained,
    /// The occurrence exists, but bounded target resolution is not established.
    UnresolvedTarget(UnresolvedReference),
}

/// A borrowed lexical target within the inspected resolution scope.
#[derive(Debug, Clone, Copy)]
pub struct LexicalTarget<'a> {
    source: &'a ReferenceIndex,
    node: &'a KnowledgeNode,
}
impl<'a> LexicalTarget<'a> {
    /// Original target source, candidate, bytes, profile and extraction identity.
    pub fn source(self) -> &'a ReferenceIndex {
        self.source
    }
    /// Exact captured target node and byte span.
    pub fn node(self) -> &'a KnowledgeNode {
        self.node
    }
}

/// Exact retained basis for a lexical claim, never semantic verification.
#[derive(Debug, Clone, Copy)]
pub struct LexicalSupport<'a> {
    source: &'a ReferenceIndex,
    occurrence: &'a ExtractedReference,
    span: SourceSpan,
    target: Option<LexicalTarget<'a>>,
    scope: ResolutionScope,
}
impl<'a> LexicalSupport<'a> {
    /// Original source, candidate, bytes, profile and extraction observation identity.
    pub fn source(self) -> &'a ReferenceIndex {
        self.source
    }
    /// Original occurrence, spelling, origin and Unverified disposition.
    pub fn occurrence(self) -> &'a ExtractedReference {
        self.occurrence
    }
    /// Exact half-open span of the occurrence in the retained source bytes.
    pub fn span(self) -> SourceSpan {
        self.span
    }
    /// Resolved target only when the target-resolution proposition was established.
    pub fn target(self) -> Option<LexicalTarget<'a>> {
        self.target
    }
    /// Resolution universe used for this support, separate from extraction coverage.
    pub fn scope(self) -> ResolutionScope {
        self.scope
    }
}

/// Per-claim result. Unsupported means no implemented mechanism, not a false claim.
#[derive(Debug, Clone, Copy)]
pub enum ClaimInspection<'a> {
    /// Retained lexical facts support the specific proposition under its caller context.
    Supported(LexicalSupport<'a>),
    /// The implemented proposition lacks a required fact; no negative claim is inferred.
    NotEstablished(ClaimGap),
    /// No implemented mechanism evaluates this kind of proposition.
    Unsupported(ReferenceClaimKind),
}

fn unsupported(claim: ReferenceClaim<'_>) -> bool {
    matches!(
        claim.kind,
        ReferenceClaimKind::SemanticRelationship | ReferenceClaimKind::CompleteExtraction
    )
}

/// Inspect retained single-source facts only. The consumer must separately establish
/// current candidate/profile correspondence; Core does not acquire or authenticate it.
pub fn inspect_retained_index_claim<'a>(
    index: &'a ReferenceIndex,
    claim: ReferenceClaim<'_>,
) -> ClaimInspection<'a> {
    if unsupported(claim) {
        return ClaimInspection::Unsupported(claim.kind);
    }
    if index.path() != claim.source {
        return ClaimInspection::NotEstablished(ClaimGap::SourceNotSelected);
    }
    let Some(reference) = index
        .references()
        .iter()
        .find(|r| r.from() == claim.locator)
    else {
        return ClaimInspection::NotEstablished(ClaimGap::OccurrenceNotRetained);
    };
    let target = if claim.kind == ReferenceClaimKind::LexicalTargetResolution {
        match reference.target() {
            ReferenceTarget::Unresolved(reason) => {
                return ClaimInspection::NotEstablished(ClaimGap::UnresolvedTarget(*reason));
            }
            ReferenceTarget::Local(locator) => Some(LexicalTarget {
                source: index,
                node: index.node(locator).expect("validated local target"),
            }),
        }
    } else {
        None
    };
    ClaimInspection::Supported(LexicalSupport {
        source: index,
        occurrence: reference,
        span: index
            .node(reference.from())
            .expect("validated occurrence")
            .span(),
        target,
        scope: ResolutionScope::SingleSource,
    })
}

/// Inspect selected-source lexical facts without altering original snapshot targets.
/// This reports retained assertions; freshness/profile checks belong to the consumer.
pub fn inspect_retained_set_claim<'a>(
    set: &'a ReferenceSet,
    claim: ReferenceClaim<'_>,
) -> ClaimInspection<'a> {
    if unsupported(claim) {
        return ClaimInspection::Unsupported(claim.kind);
    }
    if !set.sources().iter().any(|s| s.path() == claim.source) {
        return ClaimInspection::NotEstablished(ClaimGap::SourceNotSelected);
    }
    let Some(reference) = set
        .references()
        .find(|r| r.from().path() == claim.source && r.reference().from() == claim.locator)
    else {
        return ClaimInspection::NotEstablished(ClaimGap::OccurrenceNotRetained);
    };
    let target = if claim.kind == ReferenceClaimKind::LexicalTargetResolution {
        match reference.target() {
            Resolution::Unresolved(reason) => {
                return ClaimInspection::NotEstablished(ClaimGap::UnresolvedTarget(reason));
            }
            Resolution::Resolved(endpoint) => Some(LexicalTarget {
                source: endpoint.source(),
                node: endpoint.node(),
            }),
        }
    } else {
        None
    };
    ClaimInspection::Supported(LexicalSupport {
        source: reference.from().source(),
        occurrence: reference.reference(),
        span: reference.from().node().span(),
        target,
        scope: ResolutionScope::SelectedSourceSet,
    })
}
