//! Read-only, candidate-bound Cargo TOML declaration and lexical-reference adapter.
//! Relationships remain Extracted/Unverified; unresolved coverage is explicit.
//! No effective dependency, feature activation, cfg evaluation or inheritance claim.
//!
//! ```no_run
//! use cortex_cli::knowledge::cargo_manifest::{extract_cargo_manifest_references, reinspect};
//! use cortex_core::{binding::{CandidateBindingContract, ObservationId}, knowledge::SourcePath};
//! fn declarations(repo: &std::path::Path, contract: &CandidateBindingContract)
//!     -> Result<(), Box<dyn std::error::Error>> {
//!     let index = extract_cargo_manifest_references(repo, contract,
//!         ObservationId::new("cargo-example", 1)?, SourcePath::new(b"Cargo.toml")?,
//!         std::time::SystemTime::now())?;
//!     let view = reinspect(repo, &index, ObservationId::new("cargo-example", 4)?)?;
//!     assert!(view.profile_matches());
//!     // Unresolved external declarations keep validity unestablished.
//!     Ok(())
//! }
//! ```
mod parser;
use super::KnowledgeView;
use cortex_core::knowledge::inspection::*;
use cortex_core::{binding::*, knowledge::*};
use cortex_evidence::unified::Validity;
use std::{error::Error, fmt, path::Path, time::SystemTime};

/// Stable internal adapter identity; version is independent of task/format versions.
pub const ADAPTER: &str = "cortex.cargo-manifest-references";
/// Exact parser/rules/configuration dependency of this bounded extraction.
pub const PROFILE: &str = "v1;toml=singleline-strings,decimal-i64,arrays,inline,tables,aot;loc=k-i-key;refs=explicit-features,optional-declarations;no-inheritance-cfg-resolution;depth=32";

/// Bounded non-disclosing adapter failure. No parser snippets/source bytes are exposed.
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
    /// Malformed TOML or invalid UTF-8.
    InvalidToml,
    /// TOML syntax or locator text is outside the finite v1 subset.
    Unsupported,
    /// A decoded key/table has more than one declaration or conflicting shape.
    DuplicateDeclaration,
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
            Self::InvalidToml => "invalid TOML source",
            Self::Unsupported => "unsupported Cargo TOML syntax/locator",
            Self::DuplicateDeclaration => "duplicate or ambiguous TOML declaration",
            Self::Limit => "knowledge input/resource limit exceeded",
        })
    }
}
impl fmt::Debug for ExtractionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Causes remain available through Error::source; default diagnostics omit payloads.
        write!(f, "CargoManifestExtractionError({self})")
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

/// Extract one explicit regular committed Cargo TOML source with before/after captures.
///
/// See `docs/architecture/knowledge-cargo-manifest.md` for the finite syntax and
/// locator profile. This does not invoke Cargo or resolve effective dependencies.
///
/// `first` reserves sequences n, n+1 (Git) and n+2 (extraction). The recording
/// instant is caller-reported, not an authenticity/freshness claim. No files are
/// written, source content printed, remote references followed or evidence assessed.
pub fn extract_cargo_manifest_references(
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
    let (nodes, references) = parser::extract(&bytes)?;
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

/// Inspect validity against an explicitly supplied later capture. No I/O or assessment.
///
/// Trusted callers must really reobserve; changing an ID alone establishes no freshness.
pub fn inspect_validity<'a>(
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
            BindingDecision::Compatible(_) if index.all_references_resolved() => {
                Some(Validity::Current)
            }
            BindingDecision::Incompatible(_) | BindingDecision::Stale(_) => Some(Validity::Stale),
            _ => None,
        }
    };
    Ok(KnowledgeView {
        index,
        correspondence,
        validity,
        profile_matches: profile,
        coverage: super::coverage(profile, ExtractionProfile::CargoManifestLexical),
    })
}
/// Perform a real later bounded capture, then inspect retained knowledge validity; no mutation.
pub fn reinspect<'a>(
    repo: &Path,
    index: &'a ReferenceIndex,
    id: ObservationId,
) -> Result<KnowledgeView<'a>, ExtractionError> {
    let latest = crate::inspect_candidate(repo, index.candidate().observation().contract(), id)?;
    inspect_validity(index, Some(&latest))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::support::Repo;
    use cortex_core::{CandidateId, TaskContract, TaskId};
    #[test]
    fn concurrent_replacement_refuses_the_entire_extraction() {
        let r = Repo::new();
        r.commit_file("Cargo.toml", "[package]\nname='private-sentinel'\n");
        let commit = String::from_utf8(r.git(&["rev-parse", "HEAD"]).stdout).unwrap();
        let c = CandidateBindingContract::new(
            TaskContract::new(
                TaskId::new("cargo.race").unwrap(),
                "Extract manifest",
                "Reject changed source",
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
        let result = extract_between(
            &r.path,
            &c,
            ObservationId::new("cargo-race", 1).unwrap(),
            SourcePath::new(b"Cargo.toml").unwrap(),
            std::time::UNIX_EPOCH,
            || std::fs::write(r.path.join("Cargo.toml"), "changed").unwrap(),
        );
        assert!(matches!(result, Err(ExtractionError::Candidate(_))));
    }
}
