# Candidate-scoped JSON reference knowledge (003-B first slice)

[CAP-KNW-001](../product/capability-catalog.md#cap-knw-001) owns extraction/indexing;
the [roadmap](../product/roadmap.md#003-temporal-knowledge-and-memory) owns sequencing.
This implements one useful structured-source adapter, not the complete multi-language
index, incremental invalidation or CAP-KNW-002 temporal graph queries.

## Contract and ownership

The programmatic operation accepts one explicit root-relative Git path, an existing
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

## Bounds, failures and privacy

One source per operation: 256 KiB captured input, 4096 nodes, 1024 references, depth
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

Permanent authority/evidence/temporal principles remain unchanged. Initial 003-B
choices are approved and implemented within this slice: one clean committed JSON
source, lexical local-reference rules, immutable Core index and library consumer.
DEC-007 resolves only this internal API/representation; DEC-011 resolves only this
declared extraction/clean-commit boundary. Reopen for parser/rule/feature/host changes
or broader coverage. Dirty snapshots, incremental updates, additional languages,
validation, graph persistence, interchange and privacy lifecycle remain open/planned.
