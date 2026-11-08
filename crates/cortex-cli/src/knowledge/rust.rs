//! CAP-KNW-001: bounded Rust 2024 **lexical** structure, never Rust name resolution.
//!
//! # V1 coverage (fixed by [`PROFILE`])
//!
//! Input is one explicit UTF-8 committed file, interpreted without Cargo configuration.
//! Empty input is refused (the Core root span must be nonempty).
//! Names use ASCII identifiers (including `r#` identifiers); Unicode is supported in
//! comments/literals and all spans are byte offsets. Non-ASCII identifiers are refused.
//! The lexer skips line/doc/nested block comments, escaped string/byte/C literals,
//! raw string/byte/C literals and character/byte-character literals; lifetimes remain
//! tokens. Delimiters must balance. Attributes and visibility (including `pub(in ...)`)
//! are consumed without evaluating them. No text inside these opaque regions is indexed.
//!
//! Named `mod`, `fn`, `struct`, `enum`, `union`, `trait`, `type`, `const`, `static`
//! and `extern crate` declarations retain their original name-token spans. Common
//! generics, lifetimes, bounds, `where`, ABI, async/const/unsafe/default signatures,
//! tuple/unit items, trait/impl/foreign bodies and nested block items are traversed.
//! Fields, variants, parameters, locals, expressions and signature types are opaque;
//! they are not declarations/references in this profile. This is not syntax validation.
//! Unknown item forms, unclosed items, invalid lexical material and exceeded bounds
//! refuse the entire extraction rather than publishing truncated knowledge.
//!
//! Each `use` tree leaf is an occurrence: nested groups, `self`, glob, raw names,
//! `as Name` and `as _` are retained. Its span covers that leaf's contiguous tokens
//! including its alias (a group prefix is represented in normalized spelling).
//! A non-group path spans the whole path. Comments/trivia may occur inside a span;
//! spelling joins tokens with `::` and ` as `, without private trivia/control bytes.
//! `mod name;` also has an unresolved occurrence at its name token; inline modules
//! have a declaration and a traversed scope. External-crate declarations also have an
//! unresolved source-name/alias occurrence. Macro definitions/invocations in traversed
//! item/body/value positions have one
//! unresolved occurrence at their path/name, and their token trees are opaque.
//! No expansion-generated names are invented. Imports inside macros are not observed;
//! macros inside opaque field/signature/variant syntax are outside occurrence coverage.
//!
//! Locators are adapter-owned: the root is `""`; children are
//! `scope/kind@OOOOOO/name` (six-digit decimal original byte offset, no leading `/`).
//! Nested scopes append module/function/trait/value declaration locators or anonymous
//! `block@OOOOOO`, `impl@OOOOOO`, `extern@OOOOOO` components. Use locators end in
//! the imported alias/name (`*` for glob); macro locators retain their path. Duplicate
//! names/spellings retain distinct offsets. Locators are not JSON Pointers.
//!
//! Local correspondence is limited to explicit `self::`/`super::` paths through
//! uniquely named inline modules to an unambiguous named module-level declaration
//! in this file. `self` group leaves may name the current inline module. All duplicate
//! names (even across Rust namespaces), imports that bind a competing name, globs,
//! attributes and module-scope macros prevent affected local proof conservatively.
//! Block/associated declarations are preserved but never searched as module names.
//! External modules/crates, `crate::` (the selected file is not assumed a crate root),
//! unanchored paths, import chains, associated/type resolution, conditional compilation,
//! macro expansion, privacy, calls and runtime dependencies remain outside coverage.
//! Ambiguous/dynamic targets use `Unresolved(OutsideCoverage)`; absent explicit local
//! names use `MissingTarget`. Glob leaves use `OutsideCoverage`.
//!
//! Success establishes supported structural coverage only. [`ReferenceIndex::all_references_resolved`]
//! means all emitted occurrences have lexical local targets; it never establishes
//! complete Rust semantics, compilation, validation or authority. Errors distinguish
//! unsupported coverage from malformed lexical/structural material and finite limits.
//! Source, node, reference, locator and aggregate locator limits reuse Core; tokens
//! are capped at 65,536 and delimiter/comment/use-tree nesting at 32. No compiler,
//! proc macro, build script, discovery, network, persistence or execution is used.

mod lexer;
mod parser;

use super::KnowledgeView;
use cortex_core::knowledge::inspection::*;
use cortex_core::{binding::*, knowledge::*};
use cortex_evidence::unified::Validity;
use std::{error::Error, fmt, path::Path, time::SystemTime};

/// Stable producer identity, distinct from the unchanged JSON adapter.
pub const ADAPTER: &str = "cortex.rust-lexical-references";
/// Exact, fixed source-language/mechanism coverage; not a configurable profile language.
pub const PROFILE: &str = "v1;rust=2024;utf8;ascii-raw-ident;lexical-items-use-mod;self-super-inline;opaque-attrs-macros;depth=32;tokens=65536";

/// Non-disclosing, atomic acquisition/extraction failure; no partial index is returned.
#[derive(Debug)]
pub enum ExtractionError {
    /// Existing bounded repository/blob capture failed.
    Inspection(crate::InspectionError),
    /// Existing Core index/source invariants rejected construction.
    Domain(KnowledgeError),
    /// Invalid/reused binding identities or context.
    Binding(BindingError),
    /// Actual before/after captures could not bind the requested candidate.
    Candidate(Box<BindingDecision>),
    /// The observed repository has no committed HEAD.
    MissingCommit,
    /// The source is not UTF-8.
    Encoding,
    /// Malformed or truncated lexical/structural syntax in the supported subset.
    InvalidRust,
    /// Syntax outside the declared structural/identifier coverage.
    UnsupportedSyntax,
    /// An input, count, depth, metadata or observation-sequence bound was exceeded.
    Limit,
}
impl fmt::Display for ExtractionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Inspection(_) => "Rust source inspection failed",
            Self::Domain(_) => "invalid Rust knowledge source/index",
            Self::Binding(_) => "invalid Rust knowledge binding context",
            Self::Candidate(_) => "Rust knowledge candidate could not bind",
            Self::MissingCommit => "Rust knowledge requires a committed candidate",
            Self::Encoding => "Rust source requires UTF-8",
            Self::InvalidRust => "malformed or truncated Rust lexical structure",
            Self::UnsupportedSyntax => "unsupported Rust lexical coverage",
            Self::Limit => "Rust knowledge input/resource limit exceeded",
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
        if e == KnowledgeError::Limit {
            Self::Limit
        } else {
            Self::Domain(e)
        }
    }
}
impl From<crate::InspectionError> for ExtractionError {
    fn from(e: crate::InspectionError) -> Self {
        Self::Inspection(e)
    }
}

/// Acquire one explicit regular committed Rust source with actual before/after captures.
///
/// Matches the JSON producer's argument contract. `first` reserves n/n+1 captures
/// and n+2 extraction with checked arithmetic before I/O. `recorded_at` is caller
/// reported, unauthenticated and never a freshness oracle. All relations remain
/// extracted/unverified. Captured bytes are available only through explicit getters.
pub fn extract_rust_references(
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
        .ok_or(ExtractionError::UnsupportedSyntax)?;
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
    ReferenceIndex::new(bound, path, bytes, record, nodes, references).map_err(Into::into)
}

/// Inspect validity of retained Rust coverage against a supplied later observation, without I/O.
///
/// Profile mismatch/demonstrated candidate change is stale; missing observation or
/// unresolved targets is unestablished. CURRENT is scoped lexical correspondence,
/// never a Rust semantic/validation claim. Caller-built provenance is unauthenticated.
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
        coverage: super::coverage(profile, ExtractionProfile::RustLexical),
    })
}
/// Obtain a real later bounded capture and inspect validity without changing the historical index.
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
    fn concurrent_source_change_withholds_the_entire_rust_index() {
        let r = Repo::new();
        r.commit_file("source.rs", "struct Item; use self::Item as Alias;");
        let commit = String::from_utf8(r.git(&["rev-parse", "HEAD"]).stdout).unwrap();
        let c = CandidateBindingContract::new(
            TaskContract::new(
                TaskId::new("rust.race").unwrap(),
                "Capture Rust structure",
                "Refuse changed captures",
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
            ObservationId::new("rust-race", 1).unwrap(),
            SourcePath::new(b"source.rs").unwrap(),
            std::time::UNIX_EPOCH,
            || std::fs::write(r.path.join("source.rs"), "struct Changed;").unwrap(),
        );
        assert!(matches!(result, Err(ExtractionError::Candidate(_))));
    }
}
