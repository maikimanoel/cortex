//! Typed reference-snapshot changes for PP03 / CAP-KNW-001 / 003-B.
//!
//! [`compare`](crate::knowledge::delta::compare) borrows two immutable sets from the same exact declared repository
//! and worktree. `before` and `after` are caller-selected roles, not authenticated
//! chronology, ancestry or live captures. Both complete contexts, observation IDs,
//! provenance, bytes and unresolved coverage remain available through the report.
//!
//! Collection mechanisms/profiles, common sources' mechanisms/profiles and capture
//! comparison profiles must match exactly. Profile strings include any declared
//! parser/rule dependencies and are compared opaquely, without interpreting them.
//! Matching assertions permit comparison of reported structure only; they do not
//! authenticate or qualify producer behavior. New/removed selections retain their
//! own profiles and mean selected scope changed, not repository creation/deletion.
//!
//! Changes are ordered by exact source-path bytes, then lexical locator bytes.
//! Nodes match by path plus locator; occurrences match by referring path plus
//! locator. Resolved targets match by endpoint identity, independently of candidate,
//! extraction identity, source bytes or span. All changed dimensions are retained.
//! Source bytes are compared exactly even when nodes and references are unchanged.
//!
//! An empty reference-change iterator establishes neither program equivalence,
//! absence of semantic impact, permanent freshness, verified correctness, CURRENT,
//! PASS nor execution permission. Unchanged unresolved/unsupported occurrences
//! remain on both snapshots; missing coverage is never proof of absence. Relation
//! origins and dispositions retain their original Extracted/Unverified meanings.
//!
//! Comparison and iteration allocate nothing, copy no graph and mutate no inputs.
//! Each iterator merges the existing sorted bounded collections. Source, node and
//! occurrence change counts are at most twice their existing aggregate limits
//! (`MAX_SOURCES`, `MAX_SET_NODES`, `MAX_SET_REFERENCES`); no new unbounded input
//! language, allocation failure path, persistence or invalidation authority exists.
//! Default diagnostics omit source/candidate payloads; explicit getters expose
//! potentially private retained values and do not authorize disclosure.

use super::{KnowledgeNode, ReferenceIndex, source_set::*};
use crate::{binding::RepositoryIdentity, temporal::SourceReference};
use std::{cmp::Ordering, error::Error, fmt};

/// A known incompatibility in declared inputs, without private context payloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeltaError {
    /// Exact declared root, per-worktree Git directory or common directory differs.
    RepositoryMismatch,
    /// Candidate observation comparison mechanism/profile differs.
    CaptureProfileMismatch,
    /// Collection extraction mechanism differs.
    CollectionMechanismMismatch,
    /// Collection profile, including declared rule dependencies, differs.
    CollectionProfileMismatch,
    /// A source selected in both sets declares a different extraction mechanism.
    SourceMechanismMismatch,
    /// A source selected in both sets declares a different exact profile.
    SourceProfileMismatch,
}
impl fmt::Display for DeltaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "knowledge delta: {self:?}")
    }
}
impl Error for DeltaError {}

/// Selected scope or exact source-byte change. Source indexes are borrowed intact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceChange<'a> {
    /// A path was added to the selected scope, with no repository-creation claim.
    Added(&'a ReferenceIndex),
    /// A path was removed from the selected scope, with no repository-deletion claim.
    Removed(&'a ReferenceIndex),
    /// Same selected path, different exact bytes, irrespective of graph structure.
    BytesChanged {
        /// Original source bytes and context.
        before: &'a ReferenceIndex,
        /// Replacement source bytes and context.
        after: &'a ReferenceIndex,
    },
}

/// A node addition/removal or span change, borrowing original source/node values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeChange<'a> {
    /// A path/locator exists only in the second selected snapshot.
    Added {
        /// Original containing index, with candidate/provenance and exact path.
        source: &'a ReferenceIndex,
        /// Original node.
        node: &'a KnowledgeNode,
    },
    /// A path/locator exists only in the first selected snapshot.
    Removed {
        /// Original containing index, with candidate/provenance and exact path.
        source: &'a ReferenceIndex,
        /// Original node.
        node: &'a KnowledgeNode,
    },
    /// A matched node has different exact half-open byte spans.
    SpanChanged {
        /// Original containing index.
        before_source: &'a ReferenceIndex,
        /// Original node and span.
        before: &'a KnowledgeNode,
        /// Replacement containing index.
        after_source: &'a ReferenceIndex,
        /// Replacement node and span.
        after: &'a KnowledgeNode,
    },
}

/// Change in reported collection resolution, distinct from original source targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionChange {
    /// Both resolve, to different exact path/locator identities.
    Retargeted,
    /// An unresolved occurrence now resolves within selected coverage.
    BecameResolved,
    /// A resolved occurrence is now explicitly unresolved.
    BecameUnresolved,
    /// Both are unresolved, with different reported reasons.
    UnresolvedReasonChanged,
}

/// Independent changed dimensions for a matched occurrence; no assessment implied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReferenceChanges {
    resolution: Option<ResolutionChange>,
    spelling: bool,
    span: bool,
    original_target: bool,
    origin: bool,
    disposition: bool,
}
impl ReferenceChanges {
    /// Collection target/reason transition; `None` means the reported endpoint agrees.
    pub fn resolution(self) -> Option<ResolutionChange> {
        self.resolution
    }
    /// Original reference spellings differ, including equivalent lexical forms.
    pub fn spelling(self) -> bool {
        self.spelling
    }
    /// Referring token spans differ; also reported in the node-change iterator.
    pub fn span(self) -> bool {
        self.span
    }
    /// Original single-source target representations differ, independently of resolution.
    pub fn original_target(self) -> bool {
        self.original_target
    }
    /// Original relation origins differ; neither is promoted by comparison.
    pub fn origin(self) -> bool {
        self.origin
    }
    /// Original validation dispositions differ; comparison performs no validation.
    pub fn disposition(self) -> bool {
        self.disposition
    }
    fn any(self) -> bool {
        self.resolution.is_some()
            || self.spelling
            || self.span
            || self.original_target
            || self.origin
            || self.disposition
    }
}

/// Original borrowed occurrences, including spelling, spans, targets and provenance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceChange<'a> {
    /// A referring path/locator exists only in the second snapshot's references.
    Added(ReferenceOccurrence<'a>),
    /// A referring path/locator exists only in the first snapshot's references.
    Removed(ReferenceOccurrence<'a>),
    /// One or more independent dimensions changed at a matched occurrence.
    Changed {
        /// Original occurrence, with both per-source and collection targets.
        before: ReferenceOccurrence<'a>,
        /// Replacement occurrence, with both per-source and collection targets.
        after: ReferenceOccurrence<'a>,
        /// All differing dimensions, not merely the first difference.
        changes: ReferenceChanges,
    },
}

/// Borrowed comparison report. It contains no validity, result or permission state.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ReferenceDelta<'a> {
    before: &'a ReferenceSet,
    after: &'a ReferenceSet,
}
impl fmt::Debug for ReferenceDelta<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ReferenceDelta([private snapshots])")
    }
}
impl<'a> ReferenceDelta<'a> {
    /// Full first snapshot, retaining exact context/provenance and uncovered references.
    pub fn before(self) -> &'a ReferenceSet {
        self.before
    }
    /// Full second snapshot; this role implies no chronological ordering or freshness.
    pub fn after(self) -> &'a ReferenceSet {
        self.after
    }
    /// Selected source additions/removals and byte changes in exact path-byte order.
    pub fn source_changes(self) -> impl Iterator<Item = SourceChange<'a>> {
        source_pairs(self.before, self.after).filter_map(|pair| match pair {
            (Some(before), Some(after)) if before.bytes() != after.bytes() => {
                Some(SourceChange::BytesChanged { before, after })
            }
            (Some(_), Some(_)) => None,
            (Some(source), None) => Some(SourceChange::Removed(source)),
            (None, Some(source)) => Some(SourceChange::Added(source)),
            (None, None) => unreachable!("source merge always selects at least one source"),
        })
    }
    /// Node additions/removals/spans in exact path-byte, then locator-byte order.
    /// Nodes belonging to added/removed selections are included explicitly.
    pub fn node_changes(self) -> impl Iterator<Item = NodeChange<'a>> {
        source_pairs(self.before, self.after).flat_map(|(before_source, after_source)| {
            let mut before = before_source.map_or(&[][..], ReferenceIndex::nodes);
            let mut after = after_source.map_or(&[][..], ReferenceIndex::nodes);
            std::iter::from_fn(move || {
                loop {
                    let order = match (before.first(), after.first()) {
                        (Some(b), Some(a)) => b.locator().cmp(a.locator()),
                        (Some(_), None) => Ordering::Less,
                        (None, Some(_)) => Ordering::Greater,
                        (None, None) => return None,
                    };
                    match order {
                        Ordering::Less => {
                            let node = &before[0];
                            before = &before[1..];
                            return Some(NodeChange::Removed {
                                source: before_source.expect("node came from this source"),
                                node,
                            });
                        }
                        Ordering::Greater => {
                            let node = &after[0];
                            after = &after[1..];
                            return Some(NodeChange::Added {
                                source: after_source.expect("node came from this source"),
                                node,
                            });
                        }
                        Ordering::Equal => {
                            let (b, a) = (&before[0], &after[0]);
                            before = &before[1..];
                            after = &after[1..];
                            if b.span() != a.span() {
                                return Some(NodeChange::SpanChanged {
                                    before_source: before_source
                                        .expect("node came from this source"),
                                    before: b,
                                    after_source: after_source.expect("node came from this source"),
                                    after: a,
                                });
                            }
                        }
                    }
                }
            })
        })
    }
    /// Reference additions/removals/modifications in referring path/locator order.
    /// Unchanged unresolved coverage remains inspectable through both snapshots.
    pub fn reference_changes(self) -> impl Iterator<Item = ReferenceChange<'a>> {
        let mut before = self.before.references().peekable();
        let mut after = self.after.references().peekable();
        std::iter::from_fn(move || {
            loop {
                let order = match (before.peek(), after.peek()) {
                    (Some(b), Some(a)) => b
                        .from()
                        .path()
                        .bytes()
                        .cmp(a.from().path().bytes())
                        .then_with(|| b.from().node().locator().cmp(a.from().node().locator())),
                    (Some(_), None) => Ordering::Less,
                    (None, Some(_)) => Ordering::Greater,
                    (None, None) => return None,
                };
                match order {
                    Ordering::Less => return before.next().map(ReferenceChange::Removed),
                    Ordering::Greater => return after.next().map(ReferenceChange::Added),
                    Ordering::Equal => {
                        let b = before.next().expect("peek found an occurrence");
                        let a = after.next().expect("peek found an occurrence");
                        let changes = ReferenceChanges {
                            resolution: resolution_change(b.target(), a.target()),
                            spelling: b.reference().spelling() != a.reference().spelling(),
                            span: b.from().node().span() != a.from().node().span(),
                            original_target: b.reference().target() != a.reference().target(),
                            origin: b.reference().origin() != a.reference().origin(),
                            disposition: b.reference().disposition() != a.reference().disposition(),
                        };
                        if changes.any() {
                            return Some(ReferenceChange::Changed {
                                before: b,
                                after: a,
                                changes,
                            });
                        }
                    }
                }
            }
        })
    }
}

/// Compare reported structure without I/O, allocation, mutation or qualification.
///
/// Rejects mismatched exact repository/worktree identities, capture profiles,
/// collection mechanisms/profiles and common sources' mechanisms/profiles, in that
/// priority order (sources in raw-byte order). Profile equality ignores observation
/// IDs, which remain distinct and retained. All other candidate context, including
/// commits, tasks, captures, comparison material and recording times, is retained
/// without inferring cross-snapshot correspondence, ancestry or chronology.
pub fn compare<'a>(
    before: &'a ReferenceSet,
    after: &'a ReferenceSet,
) -> Result<ReferenceDelta<'a>, DeltaError> {
    let b = before.candidate().observation();
    let a = after.candidate().observation();
    if !same_repository(
        b.contract().candidate().repository(),
        a.contract().candidate().repository(),
    ) {
        return Err(DeltaError::RepositoryMismatch);
    }
    if !matches!((b.state().repository(), a.state().repository()),
        (Some(b), Some(a)) if same_repository(b, a))
    {
        return Err(DeltaError::RepositoryMismatch);
    }
    if b.basis().map(|p| p.profile()) != a.basis().map(|p| p.profile()) {
        return Err(DeltaError::CaptureProfileMismatch);
    }
    compatible_profiles(before.provenance(), after.provenance(), false)?;
    for (b, a) in source_pairs(before, after) {
        if let (Some(b), Some(a)) = (b, a) {
            compatible_profiles(b.provenance(), a.provenance(), true)?;
        }
    }
    Ok(ReferenceDelta { before, after })
}

fn same_repository(before: &RepositoryIdentity, after: &RepositoryIdentity) -> bool {
    // Path equality can normalize separators/dot components; declarations are exact.
    before.root().as_os_str().as_encoded_bytes() == after.root().as_os_str().as_encoded_bytes()
        && before.git_dir().as_os_str().as_encoded_bytes()
            == after.git_dir().as_os_str().as_encoded_bytes()
        && before.common_dir().as_os_str().as_encoded_bytes()
            == after.common_dir().as_os_str().as_encoded_bytes()
}

fn compatible_profiles(
    before: &SourceReference,
    after: &SourceReference,
    source: bool,
) -> Result<(), DeltaError> {
    if before.source() != after.source() {
        return Err(if source {
            DeltaError::SourceMechanismMismatch
        } else {
            DeltaError::CollectionMechanismMismatch
        });
    }
    if before.version() != after.version() {
        return Err(if source {
            DeltaError::SourceProfileMismatch
        } else {
            DeltaError::CollectionProfileMismatch
        });
    }
    Ok(())
}

fn resolution_change(before: Resolution<'_>, after: Resolution<'_>) -> Option<ResolutionChange> {
    match (before, after) {
        (Resolution::Resolved(b), Resolution::Resolved(a)) => {
            (b.path().bytes() != a.path().bytes() || b.node().locator() != a.node().locator())
                .then_some(ResolutionChange::Retargeted)
        }
        (Resolution::Unresolved(_), Resolution::Resolved(_)) => {
            Some(ResolutionChange::BecameResolved)
        }
        (Resolution::Resolved(_), Resolution::Unresolved(_)) => {
            Some(ResolutionChange::BecameUnresolved)
        }
        (Resolution::Unresolved(b), Resolution::Unresolved(a)) => {
            (b != a).then_some(ResolutionChange::UnresolvedReasonChanged)
        }
    }
}

fn source_pairs<'a>(
    before: &'a ReferenceSet,
    after: &'a ReferenceSet,
) -> impl Iterator<Item = (Option<&'a ReferenceIndex>, Option<&'a ReferenceIndex>)> {
    let mut before = before.sources();
    let mut after = after.sources();
    std::iter::from_fn(move || {
        let order = match (before.first(), after.first()) {
            (Some(b), Some(a)) => b.path().bytes().cmp(a.path().bytes()),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => return None,
        };
        Some(match order {
            Ordering::Less => {
                let source = &before[0];
                before = &before[1..];
                (Some(source), None)
            }
            Ordering::Greater => {
                let source = &after[0];
                after = &after[1..];
                (None, Some(source))
            }
            Ordering::Equal => {
                let pair = (Some(&before[0]), Some(&after[0]));
                before = &before[1..];
                after = &after[1..];
                pair
            }
        })
    })
}
