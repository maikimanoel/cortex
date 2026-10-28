//! First CAP-KNW-001 adapter: lexical JSON `$ref` occurrences and local pointers.
//! Not a JSON Schema validator, an execution mechanism or a complete project index.
//! ```no_run
//! use cortex_cli::knowledge::{extract_json_references, reinspect};
//! use cortex_core::{binding::{CandidateBindingContract, ObservationId}, knowledge::SourcePath};
//! fn inspect_schema(repo: &std::path::Path, contract: &CandidateBindingContract)
//!     -> Result<(), Box<dyn std::error::Error>> {
//!     let index = extract_json_references(repo, contract,
//!         ObservationId::new("schema", 1)?, SourcePath::new(b"schema.json")?,
//!         std::time::SystemTime::now())?;
//!     for reference in index.incoming("/$defs/Id")? {
//!         let span = index.node(reference.from())?.span();
//!         assert!(span.end() <= index.bytes().len());
//!     }
//!     let current = reinspect(repo, &index, ObservationId::new("schema", 4)?)?;
//!     // Inspect current.validity()/correspondence(); neither grants authority.
//!     Ok(())
//! }
//! ```

mod json;
/// Bounded committed JSON source-set acquisition and contextual qualification.
pub mod source_set;
#[cfg(test)]
use crate::support;
use cortex_core::{binding::*, knowledge::*};
use cortex_evidence::unified::Validity;
use std::{error::Error, fmt, path::Path, time::SystemTime};

/// Stable internal adapter identity; version is independent of task/format versions.
pub const ADAPTER: &str = "cortex.json-local-references";
/// Exact parser/rules/configuration dependency of this bounded extraction.
pub const PROFILE: &str = "v1;serde_json=1.0.154;raw_value;local-pointer-fragments;depth=32";

/// Bounded non-disclosing adapter failure. No parser snippets/source bytes are exposed.
#[derive(Debug)]
pub enum ExtractionError {
    /// Existing bounded Git adapter failed.
    Inspection(crate::InspectionError),
    /// Core rejected source/index invariants.
    Domain(KnowledgeError),
    /// Binding input identities were invalid or reused inconsistently.
    Binding(BindingError),
    /// Candidate did not bind; no index was published.
    Candidate(Box<BindingDecision>),
    /// No resolved committed HEAD exists.
    MissingCommit,
    /// Malformed/duplicate-key JSON or invalid UTF-8.
    InvalidJson,
    /// Valid JSON uses unsupported reference value/locator text.
    Unsupported,
    /// A finite parsing/input bound was reached.
    Limit,
}
impl fmt::Display for ExtractionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Inspection(_) => "knowledge source inspection failed",
            Self::Domain(_) => "invalid knowledge source/index",
            Self::Binding(_) => "invalid knowledge binding context",
            Self::Candidate(_) => "knowledge candidate could not bind",
            Self::MissingCommit => "knowledge extraction requires a committed candidate",
            Self::InvalidJson => "invalid or ambiguous JSON source",
            Self::Unsupported => "unsupported JSON reference/locator",
            Self::Limit => "knowledge input/resource limit exceeded",
        })
    }
}
impl Error for ExtractionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Inspection(e) => Some(e),
            Self::Domain(e) => Some(e),
            Self::Binding(e) => Some(e),
            _ => None,
        }
    }
}
impl From<KnowledgeError> for ExtractionError {
    fn from(e: KnowledgeError) -> Self {
        Self::Domain(e)
    }
}
impl From<crate::InspectionError> for ExtractionError {
    fn from(e: crate::InspectionError) -> Self {
        Self::Inspection(e)
    }
}

/// Extract one explicit regular committed JSON file using before/after Git captures.
///
/// `first` reserves sequences n, n+1 (Git) and n+2 (extraction). The recording
/// instant is caller-reported, not an authenticity/freshness claim. No files are
/// written, source content printed, remote references followed or evidence assessed.
pub fn extract_json_references(
    repo: &Path,
    contract: &CandidateBindingContract,
    first: ObservationId,
    path: SourcePath,
    recorded_at: SystemTime,
) -> Result<ReferenceIndex, ExtractionError> {
    extract_between(repo, contract, first, path, recorded_at, || {})
}
fn extract_between(
    repo: &Path,
    contract: &CandidateBindingContract,
    first: ObservationId,
    path: SourcePath,
    recorded_at: SystemTime,
    between: impl FnOnce(),
) -> Result<ReferenceIndex, ExtractionError> {
    let seq = first
        .sequence()
        .checked_add(2)
        .ok_or(ExtractionError::Limit)?;
    let extraction = ObservationId::new(first.scope(), seq).map_err(ExtractionError::Binding)?;
    let record = ExtractionRecord::new(extraction, ADAPTER, PROFILE, recorded_at)?;
    let later = ObservationId::new(first.scope(), seq - 1).map_err(ExtractionError::Binding)?;
    let before = crate::inspect_candidate(repo, contract, first)?;
    let commit = match before.state().head() {
        Head::Attached { commit, .. } | Head::Detached { commit } => commit,
        _ => return Err(ExtractionError::MissingCommit),
    };
    let repository = before
        .state()
        .repository()
        .ok_or(ExtractionError::Unsupported)?;
    let bytes = crate::inspection::committed_blob(repository, commit, &path)?;
    let (nodes, references) = json::extract(&bytes)?;
    between();
    let after = crate::inspect_candidate(repo, contract, later)?;
    let bound = match bind_candidate(contract, Some(&before), Some(&after))
        .map_err(ExtractionError::Binding)?
    {
        BindingDecision::Compatible(bound) => *bound,
        decision => return Err(ExtractionError::Candidate(Box::new(decision))),
    };
    ReferenceIndex::new(bound, path, bytes, record, nodes, references)
        .map_err(ExtractionError::Domain)
}

/// Query-context qualification, independently of unverified relationship disposition.
pub struct KnowledgeView<'a> {
    index: &'a ReferenceIndex,
    correspondence: BindingDecision,
    validity: Option<Validity>,
    profile_matches: bool,
}
impl<'a> KnowledgeView<'a> {
    /// Retained historical facts remain inspectable even when stale/unestablished.
    pub fn index(&self) -> &'a ReferenceIndex {
        self.index
    }
    /// Existing Core correspondence explains candidate mismatch or missing coverage.
    pub fn correspondence(&self) -> &BindingDecision {
        &self.correspondence
    }
    /// Existing Evidence vocabulary; None means unestablished, never implicit CURRENT.
    pub fn validity(&self) -> Option<Validity> {
        self.validity
    }
    /// Whether adapter/parser/rule configuration matches this implementation.
    pub fn profile_matches(&self) -> bool {
        self.profile_matches
    }
}
/// Qualify an index against an explicitly supplied later capture. No I/O or assessment.
///
/// Trusted callers must really reobserve; changing an ID alone establishes no freshness.
pub fn qualify<'a>(
    index: &'a ReferenceIndex,
    latest: Option<&CandidateObservation>,
) -> Result<KnowledgeView<'a>, ExtractionError> {
    let correspondence = index
        .correspondence(latest)
        .map_err(ExtractionError::Binding)?;
    let profile = index.provenance().source() == ADAPTER && index.provenance().version() == PROFILE;
    let validity = if !profile {
        Some(Validity::Stale)
    } else {
        match &correspondence {
            BindingDecision::Compatible(_) if index.complete() => Some(Validity::Current),
            BindingDecision::Incompatible(_) | BindingDecision::Stale(_) => Some(Validity::Stale),
            _ => None,
        }
    };
    Ok(KnowledgeView {
        index,
        correspondence,
        validity,
        profile_matches: profile,
    })
}
/// Perform a real later bounded capture, then qualify retained knowledge; no mutation.
pub fn reinspect<'a>(
    repo: &Path,
    index: &'a ReferenceIndex,
    id: ObservationId,
) -> Result<KnowledgeView<'a>, ExtractionError> {
    let latest = crate::inspect_candidate(repo, index.candidate().observation().contract(), id)?;
    qualify(index, Some(&latest))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cortex_core::{CandidateId, TaskContract, TaskId};
    #[test]
    fn concurrent_source_change_withholds_the_entire_index() {
        let r = support::Repo::new();
        r.commit_file("schema.json", "{\"$ref\":\"#\"}");
        let head = String::from_utf8(r.git(&["rev-parse", "HEAD"]).stdout).unwrap();
        let repository = crate::inspect(&r.path)
            .unwrap()
            .repository_identity()
            .unwrap();
        let c = CandidateBindingContract::new(
            TaskContract::new(
                TaskId::new("race").unwrap(),
                "Index source",
                "Bind same clean candidate",
            )
            .unwrap(),
            ContractRevision::new(1).unwrap(),
            CandidateRequirement::new(
                repository,
                CandidateId::from_git_commit(head.trim()).unwrap(),
                WorktreeRequirement::Clean,
            ),
        );
        let result = extract_between(
            &r.path,
            &c,
            ObservationId::new("race", 1).unwrap(),
            SourcePath::new(b"schema.json").unwrap(),
            std::time::UNIX_EPOCH,
            || std::fs::write(r.path.join("schema.json"), "changed during extraction").unwrap(),
        );
        assert!(matches!(result, Err(ExtractionError::Candidate(_))));
    }
}
