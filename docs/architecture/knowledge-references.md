# Candidate-scoped JSON reference knowledge (003-B first slice)

[CAP-KNW-001](../product/capability-catalog.md#cap-knw-001) owns extraction/indexing;
the [roadmap](../product/roadmap.md#003-temporal-knowledge-and-memory) owns sequencing.
This implements one useful structured-source adapter, not the complete multi-language
index, incremental invalidation or CAP-KNW-002 temporal graph queries.

## Contract and ownership

The single-source operation accepts one explicit root-relative Git path, an existing
candidate-binding task contract, a producer observation series and a caller-reported
recording instant. It inspects the real repository, reads a regular blob pinned by
the observed committed HEAD, extracts JSON syntax, reinspects and requires Core
binding before returning an immutable index. Ordinary/linked worktrees, detached
commits and invocation from subdirectories inherit 002-A support. Dirty, unborn,
mismatched or inconsistent candidates cannot create a successful index. No dirty
snapshot, permanent freshness or atomic repository snapshot is claimed.

| Owner | Implemented responsibility |
| --- | --- |
| Core `knowledge` | Validated source paths, byte spans, nodes, references, extraction identity/provenance and immutable index; borrowed incoming-reference queries and existing candidate correspondence |
| CLI `knowledge` | Committed source acquisition through the existing bounded Git adapter; JSON extraction; contextual inspection using existing Evidence `Validity` |
| Evidence | Existing result/applicability/validity semantics unchanged; extraction creates no assessment |
| Memory | Canonical requirements/decisions remain authoritative; indexed JSON is derived source context, not a competing canonical record store |

The existing 003-A records and 002 binding/evidence contracts provide the needed
identity/provenance foundation. Full 003-A privacy/layer and snapshot exits remain
open; this explicit, in-memory source slice neither activates new persistent capture
nor depends on exceptional storage-removal repair.

## Extraction and query semantics

Every JSON value receives an adapter-defined JSON Pointer locator and an exact
half-open byte span in the captured bytes. Object keys are decoded before identity
comparison; duplicate keys, including escaped aliases, are rejected. Arrays use
zero-based index locators. The root locator is the empty string. Source identity is
the retained repository/candidate/path, with exact input bytes available explicitly.

Every object member named `$ref` must contain a string. The extractor records its
value location, original spelling and target. `#` and supported `#/...` local
pointers resolve only to actual captured nodes. `~0`/`~1`, Unicode and arrays retain
their distinct locations. External documents, missing targets, named anchors,
malformed pointer escapes and percent-encoded fragments remain explicitly unresolved;
no invented nodes or remote resolution. Control-bearing locators are unsupported.

This is **lexical JSON reference extraction**, not JSON Schema evaluation. It walks
all JSON values, including examples, and does not interpret `$id`, dynamic references,
schema drafts, application semantics or runtime dependencies. It accepts JSON by
explicit operation, not filename convention. Other languages/formats need separate
qualified adapters; CORTEX is not restricted to Rust projects.

`ReferenceIndex::incoming(locator)` returns borrowed resolved relationships to a known
node. A known node with no incoming references returns zero; an unknown locator is
`MissingNode`. `references()` also exposes unresolved occurrences. Nodes/references
are ordered lexically by locator, independent of map enumeration. Repeated builds
with identical supplied context/IDs/time return equivalent values. Later captures
have their own provenance; equality of semantic output is distinct from identity.

All relation origins are `Extracted`; all dispositions are `Unverified`. There is
no inference input, validation-promotion API, PASS conversion or authority token.
Core construction checks structural consistency, not source authenticity or truth.

## Version, provenance and contextual validity

The index retains Core's `BoundCandidate`, complete task/revision/candidate capture,
a separate extraction `ObservationId`, existing `SourceReference` provenance and
the supplied recording instant. Sequences n/n+1 identify actual Git captures and
n+2 the extraction. Reobservation must use a later capture, conventionally n+3.
IDs/times are producer reports, not authentication; Core reads no clock.
Reusing the extraction ID for a repository reobservation is stale observation order,
even when its sequence is greater than the original candidate capture.

The adapter profile identifies its version, serde_json version, raw-value feature,
local-fragment rule and depth configuration. Immutable committed bytes/path and the
whole captured candidate are declared dependencies. Every new candidate invalidates
current reuse conservatively; this slice supplies no transitive/non-impact proof.
Old indexes remain inspectable against their original candidate without relabeling.

`qualify` consumes an explicit later observation; `reinspect` actually obtains it.
Both reuse Core binding, without making a verification judgment:

| Context | Existing Evidence validity |
| --- | --- |
| Compatible clean candidate, matching adapter profile, all references resolved within the declared subset | `Some(Current)` for this captured context/coverage only |
| Demonstrated candidate/revision/state or adapter-profile mismatch | `Some(Stale)`; original facts remain historical |
| Missing/uncertain observation or unresolved reference coverage | `None` (unestablished), never implicit CURRENT |

The view exposes correspondence and profile-match information. CURRENT neither
validates a relationship nor proves semantic completeness, source-code correctness
or consent. Matching HEAD with dirty status cannot establish CURRENT. A supplied
ID increment alone is not reinspection, and caller-built reports are unauthenticated.
Full bitemporal graph history, scoped dependency invalidation and incremental versus
full recomputation remain later CAP-KNW-001/002 and CAP-EVD-003 work.

## Additive source-set contract

`cortex_cli::knowledge::source_set::extract_json_source_set(repo, contract, first,
paths, recorded_at)` accepts 1–8 explicit `SourcePath` values under one existing
candidate-binding contract. It validates count, duplicate byte paths, chronology and
sequence capacity before I/O, sorts paths by exact bytes and obtains one actual
before capture. Every selected regular blob is read at that capture's committed
HEAD through the existing Git helper and unchanged JSON extractor. One after capture
and Core binding qualify the entire batch. A source/parser/limit/binding failure
publishes no collection; individually successful per-file captures are not composed
into an assumed common snapshot.

Core `knowledge::source_set::ReferenceSet` owns the original `ReferenceIndex`
values and a private projection of their occurrence/node offsets. It admits only
identical bound candidate/contract/capture contexts, valid ordered extraction
identities and recording times, bounded sources and non-contradictory lexical
targets. It does not duplicate source bytes or node indexes. Construction validates
reported structure, not authenticity or semantic truth.

`endpoint(&path, locator)` and `incoming(&path, locator)` identify a node by **both**
source-path bytes and locator. Missing source and missing node are distinct typed
errors; a known endpoint may have zero incoming relationships. Borrowed `Endpoint`,
`ReferenceOccurrence` and `Resolution` views expose original source bytes, token spans,
reference spelling and provenance. Occurrences are ordered by referring source bytes,
then referring locator. Input permutation with identical context/IDs/time yields
equivalent output; cycles are finite edges, not recursive evaluation.

### Declared lexical reference forms

| Spelling | Source-set projection |
| --- | --- |
| `#`, supported `#/...` | Same selected document |
| `common.json`, `common.json#`, `common.json#/Id` | Selected relative document root or pointer |
| `models/user.json#/Id` | Selected downward relative path from the referring file's directory |
| Unselected document | `OutsideCoverage`, even if the file exists in Git |
| Missing locator within a selected document | `MissingTarget` |
| Scheme/host, absolute path, backslash, query, percent-encoded document, empty/`.`/`..` path component | `OutsideCoverage`; never decoded, normalized or followed |
| Unsupported anchor/escape, percent-bearing fragment or extra `#` separator | `UnsupportedFragment` |

An empty spelling is outside coverage. Fragment classification precedes document
lookup. Relative UTF-8 document spelling is appended to the referring path's raw-byte
parent; supported non-UTF-8 identities are never coerced. No filename extension,
schema draft, `$id`, remote request, transitive discovery or dynamic-reference
semantics are inferred. The source-set single-separator subset also leaves a literal
extra `#` in a pointer unresolved, while the old lexical single-source API can still
resolve that exact key. This is an explicit new projection limit, not a legacy change.

The original per-source `ReferenceTarget` values and `complete()` never change.
A collection can resolve a selected cross-document occurrence while its original
source correctly remains unresolved/incomplete. Every relationship is still
Extracted/Unverified. Neither collection completeness nor CURRENT creates an
assessment, PASS, semantic validation or permission.

### Collection provenance, validity and bounds

For M sorted sources, captures use n/n+1, source extractions use n+2 through n+M+1,
and the collection uses n+M+2. Overflow is rejected before I/O. Recording times must
match the supplied collection instant. Reinspection needs a genuinely later capture,
conventionally n+M+3, strictly beyond all reserved extraction identities. IDs/times
remain caller assertions. The collection has its own adapter/profile, with exact
per-source parser/profile dependencies; legacy ADAPTER/PROFILE values are unchanged.

The source-set `qualify`/`reinspect` functions reuse Core correspondence and existing
Evidence `Validity`. Matching later capture, matching collection and all source
profiles, and fully resolved declared-set coverage can establish scoped CURRENT.
Known candidate/profile mismatch is STALE; missing/uncertain capture or unresolved
coverage cannot establish CURRENT. Historical sets remain immutable. This is whole-
candidate conservative invalidation, not scoped non-impact or incremental analysis.

The collection adds adapter-specific aggregate limits: 8 sources, 1 MiB captured
bytes, 8192 nodes and 2048 references. Every single-source limit below also applies.
Admission/query errors distinguish empty/duplicate/limited input, mixed contexts,
invalid provenance, conflicting references, missing source and missing node.
The CLI error wrapper preserves acquisition/parser/binding causes. A failed admission
exposes no partial collection. Existing bounded candidate context is retained per
source; input limits do not imply a total-memory bound.

At most 24 extra Git queries and two existing captures are used. Existing five-second
process, 15-second blob and 30-second capture budgets and read-only controls are
unchanged. These composed bounds are not a new hard whole-operation latency guarantee.
No new crate, dependency, command, storage, runner or source collection is implicit.

## Bounds, failures and privacy

Per source: 256 KiB captured input, 4096 nodes, 1024 references, depth
32 (root depth zero), 4096 bytes per path/locator/spelling and 256 KiB aggregate
node-locator bytes. Existing candidate-context bounds apply separately. Parsing
validates UTF-8 and bounds containers before materialization; failure publishes no
partial index. These are input/work bounds, not total-RSS or latency guarantees.

Git uses the existing sanitized, read-only process controls, literal paths and no
filters/textconv, hooks or lazy remote object retrieval. Three additional queries
have a 15-second observation budget with existing five-second process, 4 MiB stdout,
16 KiB stderr and 512 MiB Linux address-space limits. The before/after inspections
each retain their 30-second budget and cleanup limitations. There is no new runner.
Missing objects, symlinks/gitlinks, invalid paths, malformed/ambiguous JSON, unsupported
values, resource exhaustion and binding failure remain typed non-successful results.

The existing serde_json `raw_value` feature provides borrowed token extents instead
of a handwritten JSON parser. Checked safe address/range arithmetic maps those
borrows to byte spans; it dereferences no raw pointer. No new package, Core parser
dependency, persistent format, database, command or background indexing is added.
Only explicit getters expose captured bytes, path/labels and reference spellings;
default index/node/reference diagnostics omit private source data. No automatic
collection, logging, export or knowledge persistence is implemented.

## Acceptance mechanisms and decisions

[Core tests](../../crates/cortex-core/tests/knowledge.rs),
[parser tests](../../crates/cortex-cli/src/knowledge/json.rs),
[real-Git tests](../../crates/cortex-cli/tests/knowledge.rs) and the controlled
concurrent-change seam exercise actual queries/spans, duplicate/invalid/limited
inputs, historical versus current context, unresolved coverage, profile changes,
detached/unborn states, literal paths, missing objects and repository non-mutation.
The Rustdoc consumer example compiles; execution counts/candidate/platform evidence
belong in the external handoff. Existing storage-fault and 002/003-A regressions remain.
The additive [Core source-set tests](../../crates/cortex-core/tests/knowledge_set.rs)
and [real-Git source-set tests](../../crates/cortex-cli/tests/knowledge_set.rs) exercise
exact cross-document endpoints/spans, common-context admission, supported/unsupported
paths, aggregate and per-source limits, contextual validity and repository invariance.
[Single-source conformance tests](../../crates/cortex-cli/tests/knowledge_conformance.rs)
separately exercise exact limits, pointer near-neighbors, byte spans and uncertainty
through the unchanged public API. Test sources are mechanisms, not executed results.

Permanent authority/evidence/temporal principles remain unchanged. Initial 003-B
choices are approved and implemented within this slice: one clean committed JSON
source, lexical local-reference rules, immutable Core index and library consumer.
The source-set extension adds only a bounded explicit-selection projection over
those same indexes, with its own collection identity/profile and aggregate limits.
DEC-007 resolves only this internal API/representation; DEC-011 resolves only this
declared extraction/clean-commit boundary. Reopen for parser/rule/feature/host changes
or broader coverage. Dirty snapshots, incremental updates, additional languages,
validation, graph persistence, interchange and privacy lifecycle remain open/planned.
