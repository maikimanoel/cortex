//! Candidate-bound native v1 Markdown heading/link extraction for CAP-KNW-001.
//!
//! This lexical subset is not CommonMark/GitHub rendering or documentation validation.
//! See `docs/architecture/knowledge-markdown.md` for anchors, escapes and exclusions.
//! No target file/URL is read, and these indexes must not enter JSON `ReferenceSet`.

mod scanner;
use super::KnowledgeView;
use cortex_core::knowledge::inspection::*;
use cortex_core::{binding::*, knowledge::*};
use cortex_evidence::unified::Validity;
use std::{error::Error, fmt, path::Path, time::SystemTime};

/// Independent native Markdown producer identity.
pub const ADAPTER: &str = "cortex.markdown-local-references";
/// Exact native lexical/anchor/configuration dependency; no renderer compatibility.
pub const PROFILE: &str = "v1;native;utf8-lf-crlf;atx-ascii-punct-slugs-global-suffix;inline-full-collapsed-shortcut;same-line;ascii-case-space-labels;ascii-punct-escapes;bare-paren-depth=32;fence3-indent4-exact-run-code;no-html-setext-nested-containers-titles-images";

/// Unsupported syntax that would make the declared coverage uncertain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnsupportedSyntax {
    /// Non-ASCII/formatted/explicit/Setext or empty heading anchor.
    Anchor,
    /// Raw HTML or autolinks may hide unsupported anchors/relationships.
    Html,
    /// Block quote or container-prefixed code/heading/definition syntax.
    Container,
    /// Link titles, images, nested brackets or unsupported destination syntax.
    Link,
    /// A reference definition/use label is outside the ASCII label rule.
    Label,
}

/// Owned, source-redacted failure; a failed extraction publishes no partial index.
pub enum ExtractionError {
    /// Existing bounded Git acquisition failed.
    Inspection(crate::InspectionError),
    /// Existing Core index invariants rejected input.
    Domain(KnowledgeError),
    /// Invalid binding identity/context.
    Binding(BindingError),
    /// Actual before/after captures could not bind.
    Candidate(Box<BindingDecision>),
    /// A committed HEAD is required.
    MissingCommit,
    /// Invalid UTF-8; bytes are never lossily normalized.
    InvalidUtf8,
    /// Empty/control-bearing, malformed or truncated Markdown syntax.
    InvalidMarkdown,
    /// Coverage cannot be established under the native v1 profile.
    Unsupported(UnsupportedSyntax),
    /// Two definitions have the same normalized label.
    DuplicateDefinition,
    /// An existing finite input/index bound or depth/sequence bound was reached.
    Limit,
}
impl fmt::Debug for ExtractionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Inspection(_) => f.write_str("Inspection([private cause])"),
            Self::Domain(e) => f.debug_tuple("Domain").field(e).finish(),
            Self::Binding(e) => f.debug_tuple("Binding").field(e).finish(),
            Self::Candidate(_) => f.write_str("Candidate([private context])"),
            Self::MissingCommit => f.write_str("MissingCommit"),
            Self::InvalidUtf8 => f.write_str("InvalidUtf8"),
            Self::InvalidMarkdown => f.write_str("InvalidMarkdown"),
            Self::Unsupported(reason) => f.debug_tuple("Unsupported").field(reason).finish(),
            Self::DuplicateDefinition => f.write_str("DuplicateDefinition"),
            Self::Limit => f.write_str("Limit"),
        }
    }
}
impl fmt::Display for ExtractionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Inspection(_) => "Markdown source inspection failed",
            Self::Domain(_) => "invalid Markdown source index",
            Self::Binding(_) => "invalid Markdown binding context",
            Self::Candidate(_) => "Markdown candidate could not bind",
            Self::MissingCommit => "Markdown extraction requires a committed candidate",
            Self::InvalidUtf8 => "Markdown source is not UTF-8",
            Self::InvalidMarkdown => "invalid or truncated Markdown source",
            Self::Unsupported(_) => "Markdown syntax is outside the declared profile",
            Self::DuplicateDefinition => "ambiguous Markdown reference definitions",
            Self::Limit => "Markdown input/resource limit exceeded",
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
    fn from(error: KnowledgeError) -> Self {
        match error {
            KnowledgeError::Limit => Self::Limit,
            _ => Self::Domain(error),
        }
    }
}
impl From<crate::InspectionError> for ExtractionError {
    fn from(error: crate::InspectionError) -> Self {
        Self::Inspection(error)
    }
}

/// Extract one explicit regular committed Markdown blob using actual Git captures.
///
/// Reserves checked sequences n/n+1 for captures, n+2 for extraction. Recording
/// time/IDs remain caller assertions. No source/target writes, target discovery,
/// network, assessment or permission follows. Unsupported parsing refuses the
/// entire source. Repeated occurrences have distinct offset-based locators.
///
/// ```no_run
/// use cortex_cli::knowledge::markdown::{extract_markdown_references, reinspect};
/// use cortex_core::{binding::{CandidateBindingContract, ObservationId}, knowledge::SourcePath};
/// fn inspect_docs(repo: &std::path::Path, contract: &CandidateBindingContract)
///     -> Result<(), Box<dyn std::error::Error>> {
///     let index = extract_markdown_references(repo, contract,
///         ObservationId::new("docs", 1)?, SourcePath::new(b"docs/contract.md")?,
///         std::time::SystemTime::now())?;
///     let view = reinspect(repo, &index, ObservationId::new("docs", 4)?)?;
///     for link in view.index().references() {
///         let span = index.node(link.from())?.span();
///         assert!(span.end() <= index.bytes().len());
///     }
///     Ok(())
/// }
/// ```
pub fn extract_markdown_references(
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
    let sequence = first
        .sequence()
        .checked_add(2)
        .ok_or(ExtractionError::Limit)?;
    let extraction =
        ObservationId::new(first.scope(), sequence).map_err(ExtractionError::Binding)?;
    let record = ExtractionRecord::new(extraction, ADAPTER, PROFILE, recorded_at)?;
    let later =
        ObservationId::new(first.scope(), sequence - 1).map_err(ExtractionError::Binding)?;
    let before = crate::inspect_candidate(repo, contract, first)?;
    let commit = match before.state().head() {
        Head::Attached { commit, .. } | Head::Detached { commit } => commit,
        _ => return Err(ExtractionError::MissingCommit),
    };
    let repository = before.state().repository().ok_or(ExtractionError::Binding(
        BindingError::InvalidRepositoryIdentity,
    ))?;
    let bytes = crate::inspection::committed_blob(repository, commit, &path)?;
    let (nodes, references) = scanner::extract(&bytes)?;
    between();
    let after = crate::inspect_candidate(repo, contract, later)?;
    let bound = match bind_candidate(contract, Some(&before), Some(&after))
        .map_err(ExtractionError::Binding)?
    {
        BindingDecision::Compatible(bound) => *bound,
        decision => return Err(ExtractionError::Candidate(Box::new(decision))),
    };
    ReferenceIndex::new(bound, path, bytes, record, nodes, references)
        .map_err(ExtractionError::from)
}

/// Inspect validity of retained facts against a supplied later observation without I/O.
///
/// Exact profile and candidate correspondence plus fully resolved relationships
/// are needed for scoped CURRENT. This authenticates neither reports nor syntax.
pub fn inspect_validity<'a>(
    index: &'a ReferenceIndex,
    latest: Option<&CandidateObservation>,
) -> Result<KnowledgeView<'a>, ExtractionError> {
    let correspondence = index
        .correspondence(latest)
        .map_err(ExtractionError::Binding)?;
    let profile_matches =
        index.provenance().source() == ADAPTER && index.provenance().version() == PROFILE;
    let validity = if !profile_matches {
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
        profile_matches,
        coverage: super::coverage(profile_matches, ExtractionProfile::MarkdownLexical),
    })
}

/// Obtain an actual bounded later Git capture and inspect validity without changing history.
pub fn reinspect<'a>(
    repo: &Path,
    index: &'a ReferenceIndex,
    id: ObservationId,
) -> Result<KnowledgeView<'a>, ExtractionError> {
    let latest = crate::inspect_candidate(repo, index.candidate().observation().contract(), id)?;
    inspect_validity(index, Some(&latest))
}

#[cfg(test)]
mod tests;
