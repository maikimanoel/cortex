//! Bounded, explicit committed JSON source sets (CAP-KNW-001 / 003-B).
//!
//! [`extract_json_source_set`](crate::knowledge::source_set::extract_json_source_set)
//! captures Git before/after the entire acquisition,
//! reads only selected regular blobs pinned to the before commit, reuses the
//! unchanged single-source JSON extractor and binds the whole collection once.
//! Failure publishes no collection. Two captures are not an atomic snapshot.
//!
//! The collection profile below declares the exact legacy parser dependency.
//! Every original index and its unresolved external targets remain inspectable;
//! cross-document resolution is an additive Core projection. No source discovery,
//! normalization, remote access, persistent index, assessment or permission exists.
//!
//! ```no_run
//! use cortex_cli::knowledge::source_set::{extract_json_source_set, reinspect};
//! use cortex_core::{binding::{CandidateBindingContract, ObservationId}, knowledge::SourcePath};
//! fn references(repo: &std::path::Path, contract: &CandidateBindingContract)
//!     -> Result<(), Box<dyn std::error::Error>> {
//!     let set = extract_json_source_set(repo, contract, ObservationId::new("sources", 1)?,
//!         vec![SourcePath::new(b"schema.json")?, SourcePath::new(b"common.json")?],
//!         std::time::UNIX_EPOCH)?;
//!     for occurrence in set.incoming(&SourcePath::new(b"common.json")?, "/Id")? {
//!         let source = occurrence.from().source();
//!         let span = occurrence.from().node().span();
//!         let original_token = &source.bytes()[span.start()..span.end()];
//!         assert!(!original_token.is_empty());
//!     }
//!     let view = reinspect(repo, &set, ObservationId::new("sources", 6)?)?;
//!     // Current validity, if established, is scoped; relations stay Unverified.
//!     let _validity = view.validity();
//!     Ok(())
//! }
//! ```

use super::{ExtractionError, json};
use cortex_core::{
    binding::*,
    knowledge::{source_set::*, *},
};
use cortex_evidence::unified::Validity;
use std::{error::Error, fmt, path::Path, time::SystemTime};

/// Separate collection mechanism; the legacy adapter identity is unchanged.
pub const ADAPTER: &str = "cortex.json-source-set-references";
/// Exact rules, bounds and per-source parser/profile dependencies.
pub const PROFILE: &str = "v1;relative-selected-documents;exact-bytes;single-fragment;paths=no-uri-no-dot;limits=8,1048576,8192,2048;source=cortex.json-local-references;source-profile=v1;serde_json=1.0.154;raw_value;local-pointer-fragments;depth=32";

/// Non-disclosing failure; original typed acquisition/parser/binding causes survive.
#[derive(Debug)]
pub enum SourceSetExtractionError {
    /// Existing per-source acquisition, parsing, provenance or binding failed.
    Extraction(ExtractionError),
    /// Collection admission or aggregate bounds failed.
    Collection(SourceSetError),
}
impl fmt::Display for SourceSetExtractionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Extraction(e) => e.fmt(f),
            Self::Collection(e) => e.fmt(f),
        }
    }
}
impl Error for SourceSetExtractionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(match self {
            Self::Extraction(e) => e,
            Self::Collection(e) => e,
        })
    }
}
impl From<ExtractionError> for SourceSetExtractionError {
    fn from(e: ExtractionError) -> Self {
        Self::Extraction(e)
    }
}
impl From<SourceSetError> for SourceSetExtractionError {
    fn from(e: SourceSetError) -> Self {
        Self::Collection(e)
    }
}

/// Capture 1–8 explicitly selected JSON sources from one clean committed candidate.
///
/// Validates count, duplicates, provenance and n+M+2 identity capacity before I/O.
/// IDs n/n+1 are actual captures; sorted extractions use n+2..n+M+1 and the
/// collection uses n+M+2. A later reinspection needs a new capture, conventionally
/// n+M+3. Times/identities are unauthenticated caller reports, not freshness proof.
///
/// At most 24 extra Git queries plus two existing captures are performed. Each
/// blob retains its 15-second budget, each capture its 30-second budget and each
/// subprocess the existing five-second deadline and output/address-space limits.
/// Aggregate/per-source bounds are not a hard whole-operation latency/RSS promise.
/// No successful value escapes an acquisition, parsing, bound or binding failure.
pub fn extract_json_source_set(
    repo: &Path,
    contract: &CandidateBindingContract,
    first: ObservationId,
    paths: Vec<SourcePath>,
    recorded_at: SystemTime,
) -> Result<ReferenceSet, SourceSetExtractionError> {
    extract_between(repo, contract, first, paths, recorded_at, || {})
}

fn extract_between(
    repo: &Path,
    contract: &CandidateBindingContract,
    first: ObservationId,
    mut paths: Vec<SourcePath>,
    recorded_at: SystemTime,
    between: impl FnOnce(),
) -> Result<ReferenceSet, SourceSetExtractionError> {
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
    let make_id =
        |sequence| ObservationId::new(first.scope(), sequence).map_err(ExtractionError::Binding);
    let make_record = |sequence, adapter, profile| {
        ExtractionRecord::new(make_id(sequence)?, adapter, profile, recorded_at)
            .map_err(ExtractionError::Domain)
    };
    let collection = make_record(last, ADAPTER, PROFILE)?;
    let later = make_id(first.sequence() + 1)?;
    let records = (0..paths.len())
        .map(|i| {
            make_record(
                first.sequence() + i as u64 + 2,
                super::ADAPTER,
                super::PROFILE,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
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
    let mut captured = Vec::with_capacity(paths.len());
    let (mut bytes_total, mut nodes_total, mut refs_total) = (0, 0, 0);
    for (path, record) in paths.into_iter().zip(records) {
        let bytes = crate::inspection::committed_blob(repository, commit, &path)
            .map_err(ExtractionError::Inspection)?;
        bytes_total += bytes.len();
        if bytes_total > MAX_SET_BYTES {
            return Err(SourceSetError::Limit.into());
        }
        let (nodes, references) = json::extract(&bytes)?;
        nodes_total += nodes.len();
        refs_total += references.len();
        if nodes_total > MAX_SET_NODES || refs_total > MAX_SET_REFERENCES {
            return Err(SourceSetError::Limit.into());
        }
        captured.push((path, bytes, record, nodes, references));
    }
    between();
    let after =
        crate::inspect_candidate(repo, contract, later).map_err(ExtractionError::Inspection)?;
    let bound = match bind_candidate(contract, Some(&before), Some(&after))
        .map_err(ExtractionError::Binding)?
    {
        BindingDecision::Compatible(bound) => *bound,
        decision => return Err(ExtractionError::Candidate(Box::new(decision)).into()),
    };
    let sources = captured
        .into_iter()
        .map(|(path, bytes, record, nodes, references)| {
            ReferenceIndex::new(bound.clone(), path, bytes, record, nodes, references)
                .map_err(ExtractionError::Domain)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ReferenceSet::new(sources, collection)?)
}

/// Historical collection plus contextual binding/coverage validity, never assessment.
pub struct SourceSetView<'a> {
    set: &'a ReferenceSet,
    correspondence: BindingDecision,
    validity: Option<Validity>,
    profile_matches: bool,
}
impl<'a> SourceSetView<'a> {
    /// Original immutable collection, inspectable even if stale or incomplete.
    pub fn set(&self) -> &'a ReferenceSet {
        self.set
    }
    /// Existing Core correspondence, including missing observations/coverage.
    pub fn correspondence(&self) -> &BindingDecision {
        &self.correspondence
    }
    /// Evidence vocabulary; None is unestablished, never an implicit Current.
    pub fn validity(&self) -> Option<Validity> {
        self.validity
    }
    /// Exact collection and every per-source mechanism/profile match.
    pub fn profile_matches(&self) -> bool {
        self.profile_matches
    }
}

/// Qualify against a supplied genuinely later capture. No I/O or assessment.
/// Matching candidate/profile and fully resolved declared-set coverage are required
/// for scoped Current validity. New candidates/profiles conservatively invalidate;
/// incomplete or missing coverage cannot establish Current.
pub fn qualify<'a>(
    set: &'a ReferenceSet,
    latest: Option<&CandidateObservation>,
) -> Result<SourceSetView<'a>, SourceSetExtractionError> {
    let correspondence = set
        .correspondence(latest)
        .map_err(ExtractionError::Binding)?;
    let profile_matches = set.provenance().source() == ADAPTER
        && set.provenance().version() == PROFILE
        && set.sources().iter().all(|s| {
            s.provenance().source() == super::ADAPTER && s.provenance().version() == super::PROFILE
        });
    let validity = if !profile_matches {
        Some(Validity::Stale)
    } else {
        match &correspondence {
            BindingDecision::Compatible(_) if set.complete() => Some(Validity::Current),
            BindingDecision::Incompatible(_) | BindingDecision::Stale(_) => Some(Validity::Stale),
            _ => None,
        }
    };
    Ok(SourceSetView {
        set,
        correspondence,
        validity,
        profile_matches,
    })
}

/// Obtain an actual new bounded capture, then qualify without relabeling the set.
/// IDs reserved for extraction cannot substitute for post-extraction reinspection.
pub fn reinspect<'a>(
    repo: &Path,
    set: &'a ReferenceSet,
    id: ObservationId,
) -> Result<SourceSetView<'a>, SourceSetExtractionError> {
    let latest = crate::inspect_candidate(repo, set.candidate().observation().contract(), id)
        .map_err(ExtractionError::Inspection)?;
    qualify(set, Some(&latest))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cortex_core::{CandidateId, TaskContract, TaskId};
    #[test]
    fn change_after_all_blob_reads_withholds_the_whole_collection() {
        let r = crate::support::Repo::new();
        r.commit_file("a.json", r##"{"$ref":"b.json#"}"##);
        r.commit_file("b.json", r##"{"$ref":"a.json#"}"##);
        let commit = String::from_utf8(r.git(&["rev-parse", "HEAD"]).stdout).unwrap();
        let contract = CandidateBindingContract::new(
            TaskContract::new(
                TaskId::new("source-set.race").unwrap(),
                "Acquire collection",
                "Withhold mixed captures",
            )
            .unwrap(),
            ContractRevision::new(1).unwrap(),
            CandidateRequirement::new(
                crate::inspect(&r.path)
                    .unwrap()
                    .repository_identity()
                    .unwrap(),
                CandidateId::from_git_commit(commit.trim()).unwrap(),
                WorktreeRequirement::Clean,
            ),
        );
        for commit_change in [false, true] {
            let result = extract_between(
                &r.path,
                &contract,
                ObservationId::new("race", 1).unwrap(),
                vec![
                    SourcePath::new(b"a.json").unwrap(),
                    SourcePath::new(b"b.json").unwrap(),
                ],
                std::time::UNIX_EPOCH,
                || {
                    std::fs::write(r.path.join("b.json"), "[]").unwrap();
                    if commit_change {
                        r.git(&["add", "b.json"]);
                        r.git(&["commit", "-m", "race"]);
                    }
                },
            );
            assert!(matches!(
                result,
                Err(SourceSetExtractionError::Extraction(
                    ExtractionError::Candidate(_)
                ))
            ));
            if !commit_change {
                std::fs::write(r.path.join("b.json"), r##"{"$ref":"a.json#"}"##).unwrap();
            }
        }
    }
}
