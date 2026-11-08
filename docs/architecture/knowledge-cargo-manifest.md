# Bounded Cargo manifest declarations (CAP-KNW-001 / 003-B)

This adapter extends the [knowledge contract](knowledge-references.md) within
[CAP-KNW-001](../product/capability-catalog.md#cap-knw-001). It supplies source
facts, not Cargo dependency resolution, manifest semantic validation or execution.
The [roadmap](../product/roadmap.md#003-temporal-knowledge-and-memory) still owns
sequence; this slice advances no capability or milestone status.

## Finite v1 syntax profile

Before implementation, the acceptance boundary is: parse complete UTF-8 Cargo
TOML declarations, retain exact key/header/value spans, and refuse malformed,
ambiguous, unsupported or over-limit input without an index. Representative
workspace and crate manifests, including this repository's styles, must work.

Supported syntax: LF/CRLF, comments, ASCII bare keys, single-line basic/literal
quoted keys, dotted keys and quoted/target-qualified table paths; single-line
basic/literal strings with TOML escapes; booleans; signed decimal integers;
nested arrays with comments/newlines/trailing commas; single-line inline tables;
ordinary tables and repeated arrays of tables. Decoded keys determine identity.
Multiline strings, dates, floats, nondecimal numbers and multiline inline tables
are explicitly unsupported. This is a finite lexical subset, not universal TOML.

## Source profile and relationships

The root locator is empty. Decoded key components use `/k:` followed by JSON
Pointer escaping (`~0`, `~1`); array indexes use `/i:N`. Each declaration key also
has an `/@key` node. Values retain their literal source extent, including quotes,
arrays and braces. Explicit tables retain their header span; implicit tables
retain the key token that introduced them. Spans are half-open byte offsets.
Locators are adapter-owned, not JSON pointers; do not pass these indexes to the
JSON-only `ReferenceSet`. Unicode and escaped spellings remain in captured bytes.

Only explicit feature/declaration relationships in this one manifest may resolve
locally. No relationship establishes an effective version, activated dependency,
feature activation, target applicability or workspace inheritance. External
package/path/workspace/target information remains typed unresolved. No Cargo
invocation, file following, registry/cache lookup, network, build or code execution
occurs in extraction. Cargo.lock and cross-project resolution are excluded.

Dependency declarations are recognized only in root `dependencies`,
`dev-dependencies`, `build-dependencies`, `workspace.dependencies`, and the three
dependency sections beneath `target.<literal key>`. String versions, inline
specifications and ordinary dependency subtables work. Supported specification
fields are `version`, `path`, `package`, `git`, `branch`, `tag`, `rev`, `registry`,
`registry-index`, `features`, `workspace`, `optional`, `default-features`.
Unknown specification fields or wrong field shapes refuse as `Unsupported`;
`workspace` must be literal `true`. Contradictory combinations can be retained as
syntax; this operation does not certify Cargo validity. All other tables/values
are indexed as syntax without inventing relationships for their contents.

The inherited package-field subset is `version`, `authors`, `categories`,
`description`, `documentation`, `edition`, `exclude`, `homepage`, `include`,
`keywords`, `license`, `license-file`, `readme`, `repository`, `rust-version`,
and `publish`. Arbitrary `package.metadata` keys never imply inheritance.

Dependency alias keys, version/path/package/source strings, requested dependency
features, workspace inheritance, workspace member/exclusion paths and target keys
produce `Unresolved(OutsideCoverage)` occurrences. `optional` and
`default-features` booleans retain value nodes, without activation claims.
Dependency subtables and inline tables share the same locator paths. Renaming a
package never substitutes its package spelling for its declared alias.

The root `features` table (including equivalent dotted/inline syntax) must
contain string arrays. An exact declared feature name
can point to its value node only when this manifest has an explicit string
`package.name`. `dep:alias` can point to a unique root normal dependency
declaration only when `optional=true` is explicit and `workspace` is absent.
Any same alias in another captured dependency scope prevents that local edge.
These edges describe the written declaration reference only. Missing local names
are `MissingTarget`. Implicit optional-feature names, external `alias/feature`,
weak `alias?/feature`, absent package scope and inherited/ambiguous declarations
remain `OutsideCoverage`. No target selection or activation is inferred. For
example `/k:features/k:fast/i:0` containing `"dep:alias"` may point to
`/k:dependencies/k:alias`, while that dependency's path remains unresolved.
Repeated array strings receive separate `/i:N` occurrences and exact spans.
Feature names with reference punctuation are outside the local-name subset.

## Public API, acquisition and qualification

`cortex_cli::knowledge::cargo_manifest::extract_cargo_manifest_references` takes
`(&Path, &CandidateBindingContract, ObservationId, SourcePath, SystemTime)` and
returns `Result<ReferenceIndex, ExtractionError>`. It uses existing bounded
`inspect_candidate`, committed regular-blob acquisition pinned to the before
commit, an actual second capture, Core binding and `ReferenceIndex::new`.
Only the explicit source is acquired. Paths retain Core's literal root-relative
byte semantics, including supported non-UTF-8 Git names. Source text requires UTF-8.
Clean attached/detached candidates are supported. Missing/dirty/mismatched/racing
or symlink sources refuse without an index; replaced working bytes are never
silently treated as the committed source. Existing Git budgets and read-only
controls apply, including no lazy fetch, hooks, filters or textconv.

Sequences n/n+1 identify Git captures and n+2 extraction; overflow is checked
before I/O. IDs, recording time and provenance are producer/caller assertions,
not authentication. `ADAPTER` and `PROFILE` identify the exact syntax, locator and
relationship rules. The whole retained candidate, source path/bytes and these
rules are dependencies; no scoped non-impact proof is supplied.

`inspect_validity` takes a supplied later observation; `reinspect` obtains one. Both return
the existing `KnowledgeView`, with Core correspondence, profile match and Evidence
`Validity`. Candidate/profile mismatch is `Some(Stale)`; compatible matching
profiles establish `Some(Current)` only when all relationships resolve within
this declared coverage. Missing observations or unresolved coverage give `None`
(unestablished). Many useful manifests therefore remain unestablished because
external dependencies are intentionally unresolved. CURRENT is scoped lexical
coverage, never effective dependency completeness, semantic validation or PASS.
Original bytes, targets and provenance remain unchanged after reinspection.
All relationships remain Extracted/Unverified. This adds no assessment, CLI
command, store, source set, automatic consumer composition or adoption authority.

## Bounds, failures and verification mechanisms

Existing per-source bounds apply: 256 KiB input, 4096 nodes, 1024 references,
4096 bytes per locator/spelling and 256 KiB aggregate node-locator bytes. Depth is
32 with root depth zero (both syntax paths and nested values); key path parsing
also stops at 32 components. Key nodes count toward node and locator budgets.
Bounds permit exactly N and refuse N+1 without partial results. Temporary parser
syntax state is bounded by existing node/input limits and discarded after
projection; no second knowledge model or index is exposed. These are finite
input/work limits, not a hard RSS or latency guarantee.

Empty input, decoded control-bearing keys and reference spellings are unsupported.
Mixed dotted-key extension of an already header-defined table refuses as an
ambiguous redefinition. Inline tables are sealed; duplicate decoded keys,
redeclared tables and scalar/table collisions refuse. Table-array descendants
belong to the latest declared element; ordinary repeated tables remain duplicates.
Integer spellings must fit signed 64-bit decimal with valid separators. Errors
identify inspection, binding, domain, missing commit, invalid syntax, duplicate,
unsupported or limit failure without source snippets; underlying operational
causes remain available through `Error::source`. Default index/node/reference
and adapter error Debug output omits source payloads. Explicit getters expose
potentially private bytes and labels; no logging or export occurs automatically.

[Public real-Git fixtures](../../crates/cortex-cli/tests/knowledge_cargo_manifest.rs)
commit the repository's actual root and three crate manifest styles into isolated
repositories. They check literal endpoints/spans, aliases/inheritance/cfg
uncertainty, detached captures, raw-byte paths, source/Git metadata invariance,
wrong/missing/replaced/symlink sources, chronology and profile/candidate staleness.
[Private parser tests](../../crates/cortex-cli/src/knowledge/cargo_manifest/parser.rs)
exercise malformed/truncated/unsupported syntax, escaped duplicates, comments,
Unicode, arrays/tables and exact bounds. The producer unit test injects a change
between captures and requires complete refusal. Test source describes mechanisms;
actual commands/counts, candidate identity, limitations and delivery status belong
in the private external handoff. Coordinator receipt and accepted integration are
required before a future method/context consumer uses this adapter.

## Independent inspection dimensions

The [inspection contract version 2](semantic-contracts.md#knowledge-inspection-contract-version-2)
separates supplied candidate/profile correspondence, bounded extraction coverage,
resolution of emitted references and support for a particular lexical claim.
The view exposes `extraction_coverage()`, `reference_resolution()` and
`inspect_claim()`; `resolved_projection_validity()` explicitly names the retained
resolution-dependent summary. It is not a completeness or semantic-truth flag.

The adapter's supported subset and omissions above still apply. Zero emitted
references may satisfy `all_references_resolved()` without complete extraction.
A retained unresolved reference can support its lexical occurrence while its
target remains outside coverage or missing. Source acquisition remains exclusive
to the documented extraction/reinspection paths; inspecting a retained view does
not acquire files or refresh provenance.
