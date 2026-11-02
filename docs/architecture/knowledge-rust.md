# Candidate-scoped Rust lexical references (003-B)

[CAP-KNW-001](../product/capability-catalog.md#cap-knw-001) owns this bounded
read-only source adapter. The [roadmap](../product/roadmap.md#003-temporal-knowledge-and-memory)
retains the incomplete 003-B sequence. This adds no capability status advancement,
consumer release, CLI command, compiler integration or semantic Rust resolver.
The existing [Core reference index](knowledge-references.md) and
[candidate binding](candidate-binding.md) contracts remain unchanged.

## Programmatic boundary

`cortex_cli::knowledge::rust::extract_rust_references(repo, contract, first, path,
recorded_at)` returns the existing `ReferenceIndex` or an owned typed
`rust::ExtractionError`. Its argument types match `extract_json_references`:
`&Path`, `&CandidateBindingContract`, `ObservationId`, `SourcePath`, `SystemTime`.
The explicit path is a literal root-relative Git byte path, including supported
non-UTF-8 filenames. Source contents must be UTF-8. No Cargo manifest/path discovery
or crate-root assumption is made.

The producer reserves n/n+1 actual Git captures and n+2 extraction with checked
arithmetic before I/O. It uses `inspect_candidate`, the existing bounded
`committed_blob` pinned to the before commit, another actual capture,
`bind_candidate`, and `ReferenceIndex::new`. Only a compatible clean committed
candidate publishes an index. Stable attached/detached candidates inherit existing
inspection support. Dirty, changed, wrong, unborn, absent, symlink and otherwise
unsupported sources refuse publication. The two captures are not an atomic snapshot;
ignored paths and races retain the existing inspection limits.

Core owns identities, byte spans, nodes, target distinctions, provenance and the
immutable index. CLI owns this lexical mechanism and source acquisition. Evidence's
existing `Validity` is reused by the view; no evidence record or verdict is created.
There are no new dependencies, stores, generalized source profiles or public wire
formats. Rust indexes must not be passed to the JSON-Pointer-only `ReferenceSet`.

## Fixed v1 lexical profile

The exact contract is the [module Rustdoc](../../crates/cortex-cli/src/knowledge/rust.rs)
and its `ADAPTER`/`PROFILE` constants. This is a structural subset of Rust 2024,
without rustc, proc macros, build scripts or macro interpretation.

| Captured form | Representation |
| --- | --- |
| Named `mod`, `fn`, `struct`, `enum`, `union`, `trait`, `type`, `const`, `static`, `extern crate` | Name-token node; lexical parent scopes retained |
| Inline `mod name { ... }` | Module declaration and traversed child scope |
| Static `mod name;` | Declaration and explicitly unresolved module occurrence |
| Nested/grouped/renamed `use`, including `self`, `*`, `as _`, raw identifiers | One occurrence per leaf with alias/name locator and full normalized logical path spelling |
| Function/block items; trait/impl/foreign declarations | Separate named/anonymous scopes; no search of associated or block names as module declarations |
| Macro definition/invocation in traversed item/body/value positions | One unresolved occurrence at the actual macro name/path; opaque token tree; no generated symbols |
| Visibility and attributes | Consumed lexically, never evaluated; attributes conservatively obstruct local proof |
| Comments, doc comments, strings, byte/C/raw strings, characters, lifetimes | Tokenized/skipped without false imports/declarations inside literal/comment text |

Names are ASCII identifiers, including `r#` names. Non-ASCII identifiers are refused
rather than approximating Rust's Unicode identifier rules. UTF-8 comments and
literals are supported; all ranges use original byte offsets. Function signatures
support common generics, lifetimes, bounds, `where`, return types and qualifiers.
Balanced fields, variants, parameters, local bindings, signature types and expressions
are opaque, not claimed as extracted symbols, references or validated Rust syntax.
Macros in opaque fields/signatures/variants are outside occurrence coverage.
Unknown item forms and malformed/truncated supported structure refuse the whole
operation. Arbitrary semantic or expression errors are not diagnosed by this adapter.
Function/aggregate header scanning refuses recognizable subsequent named items or
imports instead of borrowing their body or semicolon. Bare `fn(...)` return types,
qualified types and generic/where delimiters remain opaque signature material.
A shared finite named-item catalog drives dispatch and header refusal. Named
`union` declarations are distinguished from weak-keyword `union` types; `impl`
at an opaque return-type start or within generic types remains type material.
Unsupported `macro name...` syntax refuses in traversed blocks; its body is never
visited as ordinary code. Supported `macro_rules!` and invocation trees stay opaque.

The root locator is empty. A declaration/occurrence locator is
`scope/kind@OOOOOO/name`, without an initial slash; `OOOOOO` is a six-digit original
byte offset. Parent named declarations or anonymous `block@OOOOOO`, `impl@OOOOOO`,
`extern@OOOOOO` identify scopes. Examples: `mod@000004/models`,
`mod@000004/models/struct@000024/Item`, `use@000080/Alias`.
Offsets distinguish duplicate names and repeated spellings. These adapter-defined
locators are not JSON Pointers and make no cross-edit identity promise.

Declaration and static-module spans cover the original name token, including a raw
identifier prefix. Use spans cover the leaf's contiguous path tokens through its
optional alias. Group prefixes contribute to spelling but are not invented contiguous
source spans: `use self::m::{Item as Alias}` records `Item as Alias` at its actual
location and spelling `self::m::Item as Alias`. Ungrouped paths span the whole path.
Trivia inside the selected range stays in retained source bytes. Spelling omits
trivia and uses `::`/` as `, preserving raw identifiers. No source snippets enter
adapter errors or default knowledge diagnostics. Explicit getters expose private data.

## Local correspondence and coverage limits

Only explicit `self::` and `super::` paths through uniquely named captured inline
modules may become `ReferenceTarget::Local`. The target must be a unique module-level
named declaration. Same-name declarations across any Rust namespaces remain ambiguous;
the adapter does not choose a namespace or an arbitrary duplicate. Import bindings,
globs, attributes and module-scope macros conservatively obstruct affected proof.
Aliases are recorded but never followed as a resolution chain. A grouped trailing
`self` can identify its captured inline module.

Unanchored/external/`crate::` paths, external modules, associated paths, import chains,
macros and ambiguous/conditional targets remain typed `Unresolved(OutsideCoverage)`.
An absent name on an otherwise supported explicit local path is `MissingTarget`.
External-crate declarations also retain an unresolved source-name/alias occurrence.
No nonexistent target node is invented. A file containing opaque macros can still
return its observed structure; expansion contents do not become that structure.
Privacy, cfg evaluation, namespace/type/name resolution, calls, runtime dependencies
and compilation correctness are outside v1 even when every occurrence has a target.

A successful producer result means supported lexical structural coverage.
`ReferenceIndex::complete()` separately says every emitted occurrence has a local
target. Neither implies complete Rust semantics. Errors independently distinguish
`Encoding`, `InvalidRust`, `UnsupportedSyntax`, `Limit`, inspection, binding, candidate
and Core construction failures. All relationships remain `Extracted` and `Unverified`.
No assessment, validation, consent or authority is inferred from extraction.

## Qualification, limits and provenance

`rust::qualify(index, latest)` performs no I/O. `rust::reinspect(repo, index, id)`
actually captures the repository again. Both return the existing `KnowledgeView`:
matching profile and compatible later clean candidate with resolved lexical coverage
may be `Some(Current)`; demonstrated candidate/profile change is `Some(Stale)`;
missing/uncertain observation or unresolved targets is `None` (unestablished).
The Rust qualifier checks its own exact profile. JSON qualification remains unchanged
and rejects Rust profiles. Reusing the extraction ID does not establish freshness.
Historical nodes, bytes, candidate and provenance remain immutable after staleness.

Whole captured candidate, explicit path/bytes and exact adapter/profile are declared
dependencies. New source/profile/candidate requires a new extraction; no non-impact,
refresh-cache or cross-profile normalization is implemented. Observation IDs, recording
instants and constructed provenance remain unauthenticated caller reports.

Bounds reuse Core: 256 KiB source, 4096 nodes (including root/occurrence nodes),
1024 references, 4096 bytes per path/locator/spelling, and 256 KiB aggregate node
locator bytes. Private lexer bounds are 65,536 tokens and depth 32 for delimiters,
nested comments, signature angles and use trees. Counts are checked before adding
material. A malformed, unsupported or exceeded source publishes no partial index.
Empty byte input is refused because Core requires a nonempty root span; nonempty
whitespace/comment-only input is supported. These are work/input bounds, not hard
RSS/latency or allocation-failure guarantees. Existing Git budgets and sanitized
read-only process controls apply unchanged; acquisition never invokes Cargo or rustc.

## Acceptance mechanisms

The [real-Git tests](../../crates/cortex-cli/tests/knowledge_rust.rs) assert exact
locators/spans and local/unresolved targets, grouped imports/aliases, duplicate
occurrences, scoped ambiguity, UTF-8 byte offsets, opaque false occurrences,
qualification, stable detached/byte paths, source/candidate refusal and repository
content/metadata invariance. Private parser/lexer tests assert exact count/byte/depth
boundaries, common Cortex-source structure and controlled concurrent-change refusal.
These files define acceptance mechanisms; actual counts, commands, candidate,
toolchain and delivery results belong in the external candidate-scoped handoff.
This increment does not complete CAP-KNW-001, 003-B, Milestone 003 or a multi-format
consumer, and releases no downstream consumer automatically.
