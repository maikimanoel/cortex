//! Producer-owned JSON refresh (PP02 / CAP-KNW-001 / 003-B).
//!
//! An opaque [`RefreshCache`](crate::knowledge::refresh::RefreshCache) retains one producer-created generation. Every
//! refresh captures the real repository before/after reading **all** selected
//! committed blobs. Only exact path/byte/parser dependency equality permits
//! copying parsed nodes and lexical references. The complete selected set is
//! always resolved anew. A dropped selection is not a repository deletion claim.
//!
//! Publication follows successful acquisition, parsing, bounds, binding and Core
//! construction. Failure leaves the retained generation unchanged; external
//! snapshots remain historical and need [`reinspect`](crate::knowledge::refresh::reinspect) for contextual validity.
//! No supplied parsed index can populate the cache. Cloning a snapshot grants no
//! cache admission, verification judgment or operational permission.
//!
//! Limits and Git budgets are those of the existing source-set operation: eight
//! sources, 1 MiB bytes, 8192 nodes, 2048 references, every per-source bound, two
//! captures and at most 24 additional Git queries. The previous generation and
//! bounded construction coexist; this is not a hard RSS/latency guarantee. Cache
//! hits avoid parsing only, not acquisition or binding. There is no persistence,
//! discovery, transitive invalidation or claim of an atomic repository snapshot.
//!
//! ```no_run
//! use cortex_cli::knowledge::refresh::{RefreshCache, refresh_json_source_set, reinspect};
//! use cortex_core::{binding::{CandidateBindingContract, ObservationId}, knowledge::SourcePath};
//! fn refresh(repo: &std::path::Path, contract: &CandidateBindingContract)
//!     -> Result<(), Box<dyn std::error::Error>> {
//!     let mut cache = RefreshCache::new();
//!     let generation = refresh_json_source_set(repo, contract,
//!         ObservationId::new("refresh", 1)?, vec![SourcePath::new(b"schema.json")?],
//!         std::time::UNIX_EPOCH, &mut cache)?;
//!     let view = reinspect(repo, generation, ObservationId::new("refresh", 5)?)?;
//!     let _validity = view.validity(); // Scoped correspondence, never an assessment.
//!     Ok(())
//! }
//! ```

use super::{ExtractionError, json, source_set::SourceSetExtractionError};
use cortex_core::{
    CandidateId,
    binding::*,
    knowledge::{source_set::*, *},
};
use cortex_evidence::unified::Validity;
use std::{fmt, path::Path, time::SystemTime};

/// Refresh collection mechanism, distinct from both legacy extraction mechanisms.
pub const ADAPTER: &str = "cortex.json-reference-refresh";
/// Refresh protocol; full provenance additionally includes the exact resolver profile.
pub const PROFILE: &str = "v1;producer-owned;fresh-all-blobs;exact-path-bytes;exact-source-bytes;full-selected-resolution";
/// Per-source mechanism: fresh acquisition/binding with explicit parse execution or reuse.
pub const SOURCE_ADAPTER: &str = "cortex.json-refreshed-source";

fn collection_profile() -> String {
    format!(
        "{PROFILE};resolver={};resolver-profile={}",
        super::source_set::ADAPTER,
        super::source_set::PROFILE
    )
}

/// Original parse context and this generation's reuse decision, not authentication.
/// Exact source bytes/path are retained by the corresponding index in the set.
#[derive(Clone, PartialEq, Eq)]
pub struct ParseObservation {
    path: SourcePath,
    origin: ObservationId,
    candidate: CandidateId,
    recorded_at: SystemTime,
    reused_from: Option<ObservationId>,
    parser: &'static str,
    parser_profile: &'static str,
}
impl ParseObservation {
    /// Exact path of the corresponding source, in the same sorted order as the set.
    pub fn path(&self) -> &SourcePath {
        &self.path
    }
    /// Extraction observation at which the parser actually executed.
    pub fn origin(&self) -> &ObservationId {
        &self.origin
    }
    /// Actual committed candidate of the original parse; never relabeled on reuse.
    pub fn candidate(&self) -> &CandidateId {
        &self.candidate
    }
    /// Original caller-reported parse recording time, not a freshness clock.
    pub fn recorded_at(&self) -> SystemTime {
        self.recorded_at
    }
    /// Previous source extraction copied in this refresh; None means parsing ran.
    pub fn reused_from(&self) -> Option<&ObservationId> {
        self.reused_from.as_ref()
    }
    /// Actual parser/lexical-rule adapter dependency.
    pub fn parser(&self) -> &str {
        self.parser
    }
    /// Exact parser version, feature, rule and configuration dependency.
    pub fn parser_profile(&self) -> &str {
        self.parser_profile
    }
    fn profile(&self) -> String {
        // Construction accepts only at/after-epoch instants, including origins.
        let at = self
            .recorded_at
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap();
        let reuse = self.reused_from.as_ref().map_or_else(
            || "none".to_owned(),
            |id| format!("{}:{}", id.scope(), id.sequence()),
        );
        format!(
            "v1;parser={};parser-profile={};mode={};origin={}:{};origin-commit={};origin-recorded={}.{:09};reuse-from={reuse}",
            self.parser,
            self.parser_profile,
            if self.reused_from.is_some() {
                "reused"
            } else {
                "parsed"
            },
            self.origin.scope(),
            self.origin.sequence(),
            self.candidate.as_str(),
            at.as_secs(),
            at.subsec_nanos()
        )
    }
}
impl fmt::Debug for ParseObservation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ParseObservation")
            .field("reused", &self.reused_from.is_some())
            .finish_non_exhaustive()
    }
}

/// Immutable producer-created snapshot. It has no public constructor or setter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefreshGeneration {
    set: ReferenceSet,
    parsing: Vec<ParseObservation>,
}
impl RefreshGeneration {
    /// Freshly bound collection with rebuilt cross-document resolution.
    pub fn set(&self) -> &ReferenceSet {
        &self.set
    }
    /// Actual parse/reuse provenance, sorted by exact source path bytes.
    pub fn parsing(&self) -> &[ParseObservation] {
        &self.parsing
    }
    fn supported(&self) -> bool {
        self.set.provenance().source() == ADAPTER
            && self.set.provenance().version() == collection_profile()
            && self.set.sources().len() == self.parsing.len()
            && self.set.sources().iter().zip(&self.parsing).all(|(s, p)| {
                s.path() == p.path()
                    && p.parser == super::ADAPTER
                    && p.parser_profile == super::PROFILE
                    && s.provenance().source() == SOURCE_ADAPTER
                    && s.provenance().version() == p.profile()
            })
    }
}

/// Opaque caller-owned cache. Only empty construction and refresh can populate it.
/// It retains at most one generation; it cannot import caller-built parsed indexes.
///
/// ```compile_fail
/// use cortex_cli::knowledge::refresh::{RefreshCache, RefreshGeneration};
/// use cortex_core::knowledge::source_set::ReferenceSet;
/// fn import_untrusted(set: ReferenceSet) -> RefreshCache {
///     RefreshCache { retained: Some(RefreshGeneration { set, parsing: Vec::new() }) }
/// }
/// ```
#[derive(Debug, Default)]
pub struct RefreshCache {
    retained: Option<RefreshGeneration>,
}
impl RefreshCache {
    /// Construct an empty cache, with no trusted caller-provided parsed contents.
    pub fn new() -> Self {
        Self::default()
    }
    /// Inspect historical retained state without establishing current validity.
    pub fn retained(&self) -> Option<&RefreshGeneration> {
        self.retained.as_ref()
    }
}

/// Refresh explicitly selected committed JSON sources and atomically replace the cache.
///
/// IDs n/n+1 identify real captures, n+2..n+M+1 source refreshes and n+M+2 the
/// collection. Overflow and known sequence reuse/order collisions with retained
/// identities fail before I/O. Scope/time are caller assertions, not authentication;
/// the caller still owns uniqueness outside the retained generation. A different
/// producer scope is not proof of a different principal or permission.
pub fn refresh_json_source_set<'a>(
    repo: &Path,
    contract: &CandidateBindingContract,
    first: ObservationId,
    paths: Vec<SourcePath>,
    recorded_at: SystemTime,
    cache: &'a mut RefreshCache,
) -> Result<&'a RefreshGeneration, SourceSetExtractionError> {
    refresh_between(repo, contract, first, paths, recorded_at, cache, || {})
}

fn refresh_between<'a>(
    repo: &Path,
    contract: &CandidateBindingContract,
    first: ObservationId,
    mut paths: Vec<SourcePath>,
    recorded_at: SystemTime,
    cache: &'a mut RefreshCache,
    between: impl FnOnce(),
) -> Result<&'a RefreshGeneration, SourceSetExtractionError> {
    if paths.is_empty() {
        return Err(SourceSetError::Empty.into());
    }
    if paths.len() > MAX_SOURCES {
        return Err(SourceSetError::Limit.into());
    }
    paths.sort_by(|a, b| a.bytes().cmp(b.bytes()));
    if paths.windows(2).any(|w| w[0] == w[1]) {
        return Err(SourceSetError::DuplicateSource.into());
    }
    let last = first
        .sequence()
        .checked_add(paths.len() as u64 + 2)
        .ok_or(SourceSetError::InvalidProvenance)?;
    if let Some(old) = cache.retained() {
        let collides =
            |id: &ObservationId| first.scope() == id.scope() && first.sequence() <= id.sequence();
        if collides(old.set.id())
            || old
                .parsing
                .iter()
                .any(|p| collides(&p.origin) || p.reused_from.as_ref().is_some_and(collides))
        {
            return Err(SourceSetError::InvalidProvenance.into());
        }
    }
    let make_id = |n| ObservationId::new(first.scope(), n).map_err(ExtractionError::Binding);
    let collection =
        ExtractionRecord::new(make_id(last)?, ADAPTER, &collection_profile(), recorded_at)
            .map_err(ExtractionError::Domain)?;
    let later = make_id(first.sequence() + 1)?;
    let before = crate::inspect_candidate(repo, contract, first.clone())
        .map_err(ExtractionError::Inspection)?;
    let commit = match before.state().head() {
        Head::Attached { commit, .. } | Head::Detached { commit } => commit,
        _ => return Err(ExtractionError::MissingCommit.into()),
    };
    let repository = before
        .state()
        .repository()
        .ok_or(ExtractionError::Unsupported)?;
    let old = cache.retained().filter(|g| g.supported());
    let mut staged = Vec::with_capacity(paths.len());
    let mut parsing = Vec::with_capacity(paths.len());
    let (mut bytes_total, mut nodes_total, mut refs_total) = (0, 0, 0);
    for (i, path) in paths.into_iter().enumerate() {
        let id = make_id(first.sequence() + i as u64 + 2)?;
        let bytes = crate::inspection::committed_blob(repository, commit, &path)
            .map_err(ExtractionError::Inspection)?;
        bytes_total += bytes.len();
        if bytes_total > MAX_SET_BYTES {
            return Err(SourceSetError::Limit.into());
        }
        let reusable = old.and_then(|g| {
            g.set
                .sources()
                .iter()
                .zip(&g.parsing)
                .find(|(s, _)| s.path() == &path && s.bytes() == bytes)
        });
        let (nodes, references, parse) = if let Some((source, origin)) = reusable {
            let mut parse = origin.clone();
            parse.reused_from = Some(source.id().clone());
            (source.nodes().to_vec(), source.references().to_vec(), parse)
        } else {
            #[cfg(test)]
            PARSER_CALLS.with(|n| n.set(n.get() + 1));
            let (nodes, references) = json::extract(&bytes)?;
            let parse = ParseObservation {
                path: path.clone(),
                origin: id.clone(),
                candidate: commit.clone(),
                recorded_at,
                reused_from: None,
                parser: super::ADAPTER,
                parser_profile: super::PROFILE,
            };
            (nodes, references, parse)
        };
        nodes_total += nodes.len();
        refs_total += references.len();
        if nodes_total > MAX_SET_NODES || refs_total > MAX_SET_REFERENCES {
            return Err(SourceSetError::Limit.into());
        }
        let record = ExtractionRecord::new(id, SOURCE_ADAPTER, &parse.profile(), recorded_at)
            .map_err(ExtractionError::Domain)?;
        staged.push((path, bytes, record, nodes, references));
        parsing.push(parse);
    }
    between();
    let after =
        crate::inspect_candidate(repo, contract, later).map_err(ExtractionError::Inspection)?;
    let bound = match bind_candidate(contract, Some(&before), Some(&after))
        .map_err(ExtractionError::Binding)?
    {
        BindingDecision::Compatible(bound) => *bound,
        other => return Err(ExtractionError::Candidate(Box::new(other)).into()),
    };
    let sources = staged
        .into_iter()
        .map(|(path, bytes, record, nodes, refs)| {
            ReferenceIndex::new(bound.clone(), path, bytes, record, nodes, refs)
                .map_err(ExtractionError::Domain)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let generation = RefreshGeneration {
        set: ReferenceSet::new(sources, collection)?,
        parsing,
    };
    Ok(cache.retained.insert(generation))
}

/// Historical refresh snapshot with contextual validity, never an assessment.
pub struct RefreshView<'a> {
    generation: &'a RefreshGeneration,
    correspondence: BindingDecision,
    validity: Option<Validity>,
    profile_matches: bool,
}
impl<'a> RefreshView<'a> {
    /// Original immutable snapshot, even if stale or unestablished.
    pub fn generation(&self) -> &'a RefreshGeneration {
        self.generation
    }
    /// Existing Core correspondence decision for the supplied capture.
    pub fn correspondence(&self) -> &BindingDecision {
        &self.correspondence
    }
    /// None means unestablished; Current is scoped to captured coverage only.
    pub fn validity(&self) -> Option<Validity> {
        self.validity
    }
    /// Exact refresh, resolver and per-source parse provenance support.
    pub fn profile_matches(&self) -> bool {
        self.profile_matches
    }
}

/// Qualify a snapshot against a caller-supplied later capture. Performs no I/O.
/// Caller assertions cannot substitute for real reinspection or authenticate IDs.
pub fn qualify<'a>(
    generation: &'a RefreshGeneration,
    latest: Option<&CandidateObservation>,
) -> Result<RefreshView<'a>, SourceSetExtractionError> {
    let correspondence = generation
        .set
        .correspondence(latest)
        .map_err(ExtractionError::Binding)?;
    let profile_matches = generation.supported();
    let validity = if !profile_matches {
        Some(Validity::Stale)
    } else {
        match &correspondence {
            BindingDecision::Compatible(_) if generation.set.complete() => Some(Validity::Current),
            BindingDecision::Incompatible(_) | BindingDecision::Stale(_) => Some(Validity::Stale),
            _ => None,
        }
    };
    Ok(RefreshView {
        generation,
        correspondence,
        validity,
        profile_matches,
    })
}

/// Obtain a real bounded later repository capture and qualify the refresh profile.
/// Legacy source-set qualification deliberately continues to reject this profile.
pub fn reinspect<'a>(
    repo: &Path,
    generation: &'a RefreshGeneration,
    id: ObservationId,
) -> Result<RefreshView<'a>, SourceSetExtractionError> {
    let latest = crate::inspect_candidate(
        repo,
        generation.set.candidate().observation().contract(),
        id,
    )
    .map_err(ExtractionError::Inspection)?;
    qualify(generation, Some(&latest))
}

#[cfg(test)]
thread_local! { static PARSER_CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }

#[cfg(test)]
mod tests {
    use super::*;
    use cortex_core::{TaskContract, TaskId};
    use std::time::UNIX_EPOCH;

    fn contract(r: &crate::support::Repo) -> CandidateBindingContract {
        let head = String::from_utf8(r.git(&["rev-parse", "HEAD"]).stdout).unwrap();
        CandidateBindingContract::new(
            TaskContract::new(
                TaskId::new("refresh.unit").unwrap(),
                "Refresh JSON",
                "Publish complete binding",
            )
            .unwrap(),
            ContractRevision::new(1).unwrap(),
            CandidateRequirement::new(
                crate::inspect(&r.path)
                    .unwrap()
                    .repository_identity()
                    .unwrap(),
                CandidateId::from_git_commit(head.trim()).unwrap(),
                WorktreeRequirement::Clean,
            ),
        )
    }
    fn id(n: u64) -> ObservationId {
        ObservationId::new("refresh.unit", n).unwrap()
    }
    fn paths() -> Vec<SourcePath> {
        vec![
            SourcePath::new(b"a").unwrap(),
            SourcePath::new(b"b").unwrap(),
        ]
    }

    #[test]
    fn parser_call_observation_rejects_fake_reuse_and_unknown_dependency_support() {
        let r = crate::support::Repo::new();
        r.commit_file("a", "0");
        r.commit_file("b", "{}");
        let mut cache = RefreshCache::new();
        PARSER_CALLS.with(|n| n.set(0));
        refresh_json_source_set(
            &r.path,
            &contract(&r),
            id(1),
            paths(),
            UNIX_EPOCH,
            &mut cache,
        )
        .unwrap();
        assert_eq!(PARSER_CALLS.with(|n| n.get()), 2);
        r.commit_file("unselected", "new candidate");
        refresh_json_source_set(
            &r.path,
            &contract(&r),
            id(10),
            paths(),
            UNIX_EPOCH,
            &mut cache,
        )
        .unwrap();
        assert_eq!(
            PARSER_CALLS.with(|n| n.get()),
            2,
            "cache hit must skip the parser call site"
        );
        r.commit_file("a", " 0");
        refresh_json_source_set(
            &r.path,
            &contract(&r),
            id(20),
            paths(),
            UNIX_EPOCH,
            &mut cache,
        )
        .unwrap();
        assert_eq!(
            PARSER_CALLS.with(|n| n.get()),
            3,
            "changed bytes must execute parsing"
        );
        // Only this private test can tamper with producer metadata. No public
        // cache import/setter exists, even for a structurally valid Core index.
        cache.retained.as_mut().unwrap().parsing[0].parser_profile = "unknown-rules";
        let stale = reinspect(&r.path, cache.retained().unwrap(), id(30)).unwrap();
        assert!(!stale.profile_matches());
        assert_eq!(stale.validity(), Some(Validity::Stale));
        refresh_json_source_set(
            &r.path,
            &contract(&r),
            id(30),
            paths(),
            UNIX_EPOCH,
            &mut cache,
        )
        .unwrap();
        assert_eq!(
            PARSER_CALLS.with(|n| n.get()),
            5,
            "unsupported provenance must reparse"
        );
        assert!(
            cache
                .retained()
                .unwrap()
                .parsing()
                .iter()
                .all(|p| p.reused_from().is_none())
        );
    }

    #[test]
    fn after_read_dirty_or_committed_race_preserves_old_generation_and_pointer() {
        for commit_change in [false, true] {
            let r = crate::support::Repo::new();
            r.commit_file("a", "0");
            r.commit_file("b", "{}");
            let c = contract(&r);
            let mut cache = RefreshCache::new();
            let old = refresh_json_source_set(&r.path, &c, id(1), paths(), UNIX_EPOCH, &mut cache)
                .unwrap()
                .clone();
            let old_ptr = cache.retained().unwrap() as *const RefreshGeneration;
            let error =
                refresh_between(&r.path, &c, id(10), paths(), UNIX_EPOCH, &mut cache, || {
                    std::fs::write(r.path.join("b"), "[]").unwrap();
                    if commit_change {
                        r.git(&["add", "b"]);
                        r.git(&["commit", "-m", "race"]);
                    }
                })
                .unwrap_err();
            assert!(matches!(
                error,
                SourceSetExtractionError::Extraction(ExtractionError::Candidate(_))
            ));
            assert_eq!(cache.retained(), Some(&old));
            assert_eq!(
                old_ptr,
                cache.retained().unwrap() as *const RefreshGeneration
            );
            assert_eq!(
                reinspect(&r.path, &old, id(20)).unwrap().validity(),
                Some(Validity::Stale)
            );
        }
    }

    #[test]
    fn retained_parse_origin_still_reserves_its_scope_after_producer_scope_change() {
        let r = crate::support::Repo::new();
        r.commit_file("a", "0");
        r.commit_file("b", "{}");
        let c = contract(&r);
        let mut cache = RefreshCache::new();
        refresh_json_source_set(&r.path, &c, id(1), paths(), UNIX_EPOCH, &mut cache).unwrap();
        refresh_json_source_set(
            &r.path,
            &c,
            ObservationId::new("other", 1).unwrap(),
            paths(),
            UNIX_EPOCH,
            &mut cache,
        )
        .unwrap();
        let old = cache.retained().unwrap().clone();
        let error = refresh_json_source_set(
            &r.path.join("absent"),
            &c,
            id(3),
            paths(),
            UNIX_EPOCH,
            &mut cache,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            SourceSetExtractionError::Collection(SourceSetError::InvalidProvenance)
        ));
        assert_eq!(cache.retained(), Some(&old));
    }
}
