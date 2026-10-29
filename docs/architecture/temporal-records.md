# Temporal record foundations (003-A)

This defines the implemented first slice of [003-A](../product/roadmap.md#003-temporal-knowledge-and-memory),
under [CAP-MEM-001](../product/capability-catalog.md#cap-mem-001),
[CAP-SEC-002](../product/capability-catalog.md#cap-sec-002) and
[CAP-CORE-002](../product/capability-catalog.md#cap-core-002). The catalog owns
requirements; this document owns the bounded domain contract implemented in
[`cortex_core::temporal`](../../crates/cortex-core/src/temporal/mod.rs).
The first encrypted local adapter is specified by the accepted
[durable-history decision](0002-durable-history.md). This remains partial
003-A/CAP-MEM-001 progress, not the complete memory subsystem. Its private format
selects no public interchange or general compatibility commitment.

## First usable slice and ownership

The implementation provides deterministic, caller-owned **in-memory canonical requirement/decision
history**: record a version, explicitly correct it, inspect retained versions and
query effective versions at supplied domain and recording times. Bind each collection
to one supplied candidate observation. This is a usable library operation, not an
empty memory crate or storage facade. This slice does not complete 003-A or CAP-MEM-001.

| Owner | Responsibility |
| --- | --- |
| Memory/continuity | Logical ownership of canonical records, corrections and historical interpretation |
| `cortex-core` | A cohesive temporal-record module with validated values, bounded transitions and queries; no filesystem, clock sampling, Git, CLI or backend dependency |
| `cortex-evidence` | Existing verification provenance, observations and assessments; no competing result/applicability/validity model or history-to-PASS conversion |
| `cortex-cli` | Existing observation producer, encrypted local history adapter and real-Git consumer tests; commands unchanged |

Reuse `TaskId`/`ContractRevision` for actual task references and
`CandidateObservation` for captured context, including its existing
`RepositoryIdentity`, `CandidateId`, `ObservationId`, state, coverage and limits.
Logical record ID and immutable record-version ID are distinct new roles;
uniqueness is local, not a global registry. Record changes do not mutate a
`TaskContract`, reuse its revision for changed contents, relabel captures or overwrite
terminal assessments.

Core retains bounded source/author/version references as **caller-reported record
provenance**, not a duplicate of Evidence's verification-provenance model. Operator
assertions and tool observations have distinct origins; do not invent verification
observations for assertions. Evidence references retain their owner; Core must not
depend on Evidence to resolve them. Evidence-linked learning consumers arrive in
003-C/003-D. Constructors do not authenticate sources or authorize work.

## Inputs, transitions and queries

Inputs supply logical ID, fresh version ID, requirement/decision kind, nonblank
bounded body, declared source/version references, collection context, domain-valid
interval and recording instant. Only explicitly canonical records are admitted;
other layers are excluded by the typed API, not silently canonical. Source classification
and handling remain trusted caller obligations, not inferred from author names.

- Domain intervals are nonempty and half-open `[from, until)`, with an explicit open end when
  needed. Caller-supplied instants are at/after the Unix epoch. Domain validity may
  precede or follow recording. Evidence's `ObservationTimes` chronology describes
  a different concept and must not forbid either case. Core samples no clock.
- A correction preserves logical ID/context, names the current predecessor in that
  record's chain and has a fresh version ID. Recording cannot precede the predecessor's
  recording; equal instants are ordered by the explicit relation. Late recording of
  a correction to an earlier domain interval is supported. Category cannot change
  within a chain. Independent entities need no global recording-time ordering.
- Retain prior bodies, intervals and provenance unchanged. Reject duplicate/aliased
  version IDs, unknown predecessors, cycles and branching/stale-predecessor corrections
  atomically. General conflicting-source reconciliation remains in 003-C; do not
  choose winners by arbitrary timestamps or lexical IDs.
- An as-of query supplies domain time and recording cutoff within the explicit context.
  Among versions recorded by the cutoff whose intervals contain the domain instant,
  select by supersession. Corrections override predecessors only where their own
  intervals apply; outside them, an applicable predecessor survives. Return version
  and source references and scope limits with data.
- No match, unavailable required history, wrong context and invalid input remain
  distinguishable. No match proves neither real-world absence nor criterion N/A.
  Historical selection is not CURRENT evidence, PASS or permission. Queries do not
  revalidate a live repository.

Unavailable history here means a missing named version/predecessor in the supplied
collection. The adapter reconstructs that collection without resolving external sources.

Attached, detached, unborn, dirty and incomplete observations retain their existing
distinctions; absent commits stay absent. Same-HEAD captures with different dirty
state or observation identity are not silently merged. Local worktree identity is
not portable project identity; branches are not one universal revision clock.
Cross-candidate/branch recovery and new snapshots are outside this slice.

Use typed errors for identity reuse, bad interval/time, predecessor/category/scope
mismatch, unavailable history and limit exhaustion. Unsupported categories/layers
are not constructible through the API. Rejection leaves the
accepted collection unchanged. Reapplying an accepted sequence to a new collection
gives equivalent history/query results without I/O or clock dependence; this is
reconstruction, not a crash-safe journal or external-effect replay.

## Rust API and validation boundary

[`record.rs`](../../crates/cortex-core/src/temporal/record.rs) owns validated values;
[`mod.rs`](../../crates/cortex-core/src/temporal/mod.rs) owns collection transitions
and queries. Core adds no crate/dependency, CLI command or Evidence API; the
adapter's native dependencies are isolated in CLI infrastructure.

| Interface | Contract |
| --- | --- |
| `RecordId` / `RecordVersionId` | Logical entity versus immutable version identity; duplicates are errors even for identical resubmission |
| `RecordKind` | Canonical `Requirement` or `Decision`; historical decisions grant no permission |
| `SourceReference` / `SourceOrigin` | Explicit `operator_reported` author/source/version or `observation_reference` source/version; no authentication or source resolver |
| `RecordContent` | Validated body and at least one bounded source reference; private fields, immutable accessors |
| `DomainInterval` / `RecordTime` | Valid domain interval and independently supplied recording time; no clock reads |
| `CanonicalRecord::initial` / `correction` | Locally validated draft, distinct initial/correction intent; cross-record validation occurs at append |
| `CanonicalHistory::new` / `append` | Own one exact `CandidateObservation`; validate all relationships and budgets before accepting a version |
| `records` / `version` / `history` | Immutable inspection of all append-ordered versions, a named version, or one entity's predecessor-ordered chain |
| `TemporalQuery` / `query` / `TemporalView` | Exact capture ID, domain instant, inclusive recording cutoff; return original candidate/coverage context and an optional selected version |
| `timeline::TimelineQuery` / `query_timeline` | One finite domain window and recording cutoff; borrowed selections in a bounded coalesced partition |

Missing named entities/versions/predecessors have distinct errors. A known entity
with no eligible version returns `TemporalView::record() == None`; an unknown entity
is `MissingEntity`. Querying before its first recording is a valid no-match view.
Recording and domain instants before the Unix epoch, empty/inverted finite intervals,
category changes and stale predecessors are rejected. Fresh IDs with an existing
latest predecessor exclude cycles; a second initial version or fork is rejected
instead of admitting an ambiguous canonical view. General source reconciliation
remains deferred.

`append` preserves accepted context, versions, order and byte accounting on every
returned error. Space reservation failure has a typed error; ordinary Rust
allocation failure while constructing/cloning caller-owned values is not a
recoverable-memory guarantee. Point/history queries borrow retained data without
allocating or mutating it; traversal is bounded by the retained-version limit.
The caller must reconstruct the same accepted sequence to obtain equivalent in-memory history;
the API does not supply persistence or revalidate the observation.

## Interval timelines

`cortex_core::temporal::timeline::query_timeline` consumes a validated
`TimelineQuery::new(context, from, until, recorded_by)` and borrows one known entity's
history. The finite nonempty half-open window is partitioned into ordered, disjoint
nonempty segments covering exactly that window. Each segment exposes its original
record or explicit absence; adjacent segments with the same selected version or
absence are coalesced. Distinct versions with equal bodies remain distinct.

Selection delegates to the unchanged point query at retained validity boundaries.
The recording cutoff is inclusive; equal recording times follow predecessor order.
A correction overrides only within its own interval, with earlier-version fallback
elsewhere. Later recordings cannot affect an earlier cutoff. Wrong capture and
unknown entity retain typed errors, distinct from no eligible retained version.
All times remain exact, with no clock reads, successor arithmetic or truncation.

Fallible vector reservations bound output construction: the existing 1024-version
limit permits at most 2050 endpoints and 2049 segments before coalescing. Selection
has bounded O(V²) traversal, not a measured latency or total-memory guarantee.
Returned errors do not mutate history. No backend, persisted field or storage format
changes. Absence does not represent privacy gaps, real-world absence or N/A, and
historical selection establishes no freshness, PASS or operational authority.
[Timeline tests](../../crates/cortex-core/tests/temporal_timeline.rs) include literal
partitions, cutoffs, nanosecond/upper host-time boundaries, fallback, limits and
immutable borrowing. Their execution evidence belongs in the external handoff.

## Bounds and privacy

Initial internal limits: 128-byte ASCII IDs, 4096-byte bodies/source descriptions,
32 source references per version, 1024 retained versions and 4 MiB aggregate retained
record text/reference bytes. Distinct record-ID types use the existing TaskId
character syntax; bodies/source descriptions use the existing nonblank/control-character
rules (line feed permitted). Validate before copying into accepted state. Count
superseded versions; do not evict history to fit. Bound point/history traversal by
the version limit and interval output by the derived endpoint limits above.
Existing observation bounds apply separately; retain context once
rather than cloning it per version. These are input bounds, not retention durations,
a wire format, performance targets or total-RSS guarantees.

`retained_bytes()` counts UTF-8 bytes for each entry's logical/version/context and
optional predecessor IDs, body, source/version strings and operator author or
observation reference. An observation ID contributes its scope bytes plus eight
sequence bytes. Times, enum tags, collection/string allocation overhead and the
separately retained candidate observation are excluded. Superseded content still
consumes the budget; references are counted as supplied, not deduplicated evidence.

Errors and default debug/summary output omit private bodies, raw candidate/task
context and caller-supplied source details; explicit accessors may expose supplied
data. Constructors cannot detect all secrets,
prove consent or authenticate handling labels. Use synthetic/non-sensitive fixtures.
Core provides no I/O; the separately invoked adapter persists explicit submissions
only. No automatic capture, export, background retention or payload logging is
provided. Dropping ordinary memory is not secure erasure.

## Consequential gates and alternatives

| Boundary | Minimum contract and unresolved choice | Decision point |
| --- | --- | --- |
| Privacy/layers | INV-012/CAP-SEC-002 require inspection/correction, authorized deletion and honest gaps. The [lifecycle contract](#privacy-lifecycle) separates operations; the [initial operator profile](#owner-policy-gates) is approved. Broader layers, delegation and selective removal remain open. | Scoped DEC-004 resolution for this adapter; trace policy before 004-D |
| Persistence/replay | The [durable admission contract](#durable-admission) defines atomic acceptance, uncertain completion and reconstruction. The [accepted decision](0002-durable-history.md) selects the qualified encrypted SQLite adapter and private format; migration and stronger host/durability claims remain open. | Scoped DEC-007 resolution; reopen before format/host changes |
| Candidate/snapshot | Reuse bounded 002-A/B observations. Clean-commit-only extraction and a separately observed immutable dirty manifest are possible later scopes; equal HEAD/status selects neither. Full 003-A still needs the scheduled snapshot decision before that extension. | DEC-011 before snapshots; extractor coverage before 003-B |
| Time/identity | Record/version, domain/recording time and task/candidate/capture references remain distinct. Internal values select no persisted-format version, portable project ID or cross-branch total order. | Minimum rules above; durable identity/format under DEC-007 |
| Evidence/invalidation | 002-C direct checks remain unchanged. Supersession supplies changed references; transitive invalidation/non-impact and incomplete dependency coverage belong to CAP-EVD-003. A lookup cannot refresh PASS. | 003-C; no second evidence authority in 003-A |
| Portable handoff | CAP-MEM-003/CAP-CTX-002 require integrity, candidate correspondence and privacy-aware omission. Digests and authenticated signatures differ; no package schema/authenticity mechanism is selected. | DEC-004/007 before 003-E/003-F |

Logical append-only correction describes retained history, not permanent physical
retention or an override of authorized deletion. Future deletion must propagate to
indexes/caches/exports under selected policy; tombstones must not recreate erased
content. Unavailable backups/history remain explicit gaps. Core provides no privacy
I/O; the adapter's whole-collection removal does not implement selective lifecycle views.

## Acceptance mechanisms and remaining scope

The [Core tests](../../crates/cortex-core/tests/temporal.rs) and
[real-Git consumer tests](../../crates/cortex-cli/tests/temporal.rs) exercise this
first slice. Executed outcomes, counts, candidate and environment belong to the
external implementation handoff; test source alone is not execution evidence.

| Case | Discriminating observation |
| --- | --- |
| Record/correction | Both immutable versions/provenance remain inspectable; the old value is not overwritten |
| Late correction | A cutoff before recording returns the earlier perspective; a later cutoff for the same domain instant returns the correction |
| Interval boundaries | Inclusive start, exclusive/open end behave exactly; outside a correction's interval an applicable predecessor survives |
| Invalid input | ID reuse with different bodies, duplicate submission, bad intervals, missing/forking predecessors and backward recording time yield typed errors and unchanged state |
| Reconstruction | The same accepted sequence gives equivalent history/queries without clock/I/O dependence |
| Candidate distinctions | Real-Git adapters preserve clean/dirty and detached/unborn context; a synthetic incomplete/indeterminate capture retains uncertainty; equal dirty HEAD cannot become a snapshot or CURRENT assessment |
| Missing scope/history | Wrong context or unavailable required history is explicit; no fabricated record, commit, authenticity or N/A result |
| Limits/privacy | Exact bounds pass; the next byte/reference/version fails atomically; diagnostics/debug omit a private-body sentinel |
| Compatibility | Run existing 002-A/B/C/D read-only, binding, evidence, CLI and compile-fail regressions; history neither rewrites evidence nor grants authority |

Core owns domain tests. Real-Git boundary tests belong in the CLI consumer using
existing fixtures, preserving dependency direction. Keep invariants documented and
errors cohesive; no generic framework/new dependency is justified. Full 003-A still
needs the remaining privacy/layer and snapshot gates. Adapter recovery/deletion
tests are described below. Extraction, cross-candidate recovery, portable packages, rule registries,
workflows and empirical evaluation retain later roadmap exits.

## Durable history extension: contract and first implementation

The following obligations govern CAP-MEM-001 and CAP-SEC-002. The
[first encrypted adapter](0002-durable-history.md) implements create, recovery/load,
admission, reconciliation and whole-collection removal. Logical withdrawal,
selective redaction and external-copy propagation remain future contracts.
DEC-004/007 retain broader open questions; neither this adapter nor its format
creates a new authority mechanism or public protocol.

### Privacy lifecycle

A request is data, not permission. A record author, affected person or consumer may
raise a correction/removal request; application requires the store operator or an
owner-approved delegate acting within an explicitly approved scope and policy.
How identities, delegation and disputes are established is an owner decision.
Record provenance, possession of an ID and filesystem access alone do not establish
that approval. The first adapter must be limited to trusted, explicit operator
invocations; agent-initiated or multi-principal mutation requires the later Core
authority boundary. No stored decision can authorize its own lifecycle operation.

| Operation | Application authority | Affected data and access | Reconstructable history | Completion observations and limits |
| --- | --- | --- | --- | --- |
| Ordinary correction/supersession | Operator/delegate authorized to amend that entity | Append a fresh version; prior body, source and times remain accessible | Original and corrected perspectives remain available under the existing temporal rules | Durable admission and reload preserve both versions; this removes no private data |
| Logical withdrawal/invalidation | Operator/delegate authorized to withdraw the declared scope | Retain payload, attach a separate inspectable disposition/reason; active-use consumers must expose and respect withdrawal | Historical content remains inspectable; an active-use view is distinct from Core's unchanged temporal selection | Reload confirms the disposition and its scope; it is not physical deletion or an automatic Evidence validity transition |
| Privacy redaction | Explicit approval of fields, affected versions/references and permitted remnants | Remove prohibited payload from managed storage and prohibited outputs, including earlier versions, source metadata and derived copies in scope | Only permitted surviving information and explicit unavailable-history gaps; no cutoff may recover redacted content | Inspect allowed views and managed storage for removal; report incomplete/unobservable copies, without logging the removed text |
| Local physical deletion | Explicit approval of entity/collection and managed-copy scope | Remove protected information from all covered live files, including free pages, journals, staging files and caches; a row deletion or tombstone alone is insufficient | Permitted non-sensitive receipt/gap only, if policy allows it; no invented original body or predecessor | Report which managed files/data were observed removed and outstanding failures; unlinking is not proof of sector sanitization or secure memory erasure |
| Backup/export/external-copy handling | Separate authority over each copy; the primary operator cannot grant rights over another system | Propagate deletion/redaction to controlled copies under policy; uncontrolled, inaccessible or provider-managed copies remain named coverage limitations | Only permitted material; restoration must not silently resurrect removed information | Per-copy observations, unresolved owners and inaccessible scopes; a local receipt cannot certify universal erasure |

Privacy restrictions apply across **all recording cutoffs**, independent of when
an old fact was valid. A tombstone, removal digest or receipt must not retain
forbidden content or enable its reconstruction. Even IDs, paths, hashes and reasons
may be sensitive; retain them only under the approved remnant policy. Completion
reports describe an operation and its coverage, not a new assessment-result vocabulary.

The existing Core cannot represent a missing predecessor as a valid chain. A future
redaction/deletion layer must return explicit unavailable history rather than skip
entries, rewrite an old version under its old ID, substitute placeholder provenance,
or reinterpret `TemporalView::record() == None` as a privacy gap. Withdrawals are
also not ordinary corrections containing magic text. Fine-grained lifecycle views
need their own narrow, reviewed contract before implementation; Core's present
queries and Evidence's result/applicability/validity meanings stay unchanged.

### Owner policy gates

The owner approved the initial profile below; the accepted
[storage decision](0002-durable-history.md) records its implementation and qualification.
These choices do not settle broader trace, multi-principal or synchronization policy.

| Policy area | Approved initial profile | Deferred obligation |
| --- | --- | --- |
| Capture/classification | Explicit canonical requirements/decisions and necessary full candidate context only; no transcript, secret or background capture | Caller must classify submissions; constructors cannot prove consent or detect every secret. Expanded collection needs separate controls |
| Access/location | One trusted explicitly authorized local operator; a private selected directory outside inspected repositories; caller-supplied encryption key | No delegated identity/permission engine, shared-user access or universal host support |
| Retention/layers | Retain canonical history until explicit authorized removal within finite capacity; fail instead of silently evicting | Transient/ephemeral expiry and sensitive trace policies remain open |
| Removal/remnants | Whole collection only; no retained payload-derived receipt. Empty directory/lock remain; a random 16-byte instance intent exists while removal is pending | Selective redaction/withdrawal needs gap-aware views; no narrow request may silently delete a broader collection |
| Confidentiality/deletion | SQLCipher from the initial persisted record; removal of covered managed live files with disclosed limitations | No universal forensic erasure, key recovery/rotation, secure RAM guarantee or assurance over snapshots, swap and external copies |
| Copies/disclosure | No automatic backup, export, import, synchronization or restoration; inspection remains an explicit caller operation | Independent copies need their own authority and observations; getters do not authorize unrestricted disclosure |

The persisted policy reference `local-operator-encrypted-v1` is distinct from task,
record and private format versions. Unknown policies are rejected, not implicitly
migrated. Policy changes require explicit review. Engine journals are protected
store data, not an exception to minimization or removal accounting. Missing broader
policy blocks only its dependent operations, not approved initial-profile admission.

### Durable admission

One first-slice store contains one complete bounded `CanonicalHistory` and its exact
candidate observation. Store-instance identity distinguishes a created container
from another store later occupying the same path; it is not a new candidate or an
authentication token. New containers must not reuse an old instance identity.
Record/version/task/capture identities retain their current roles and validation.

1. Require an approved handling policy and explicit target/operation authority before
   creating or changing files. Validate target identity, ownership, permitted paths,
   resource bounds and supported format. Source strings/IDs are never interpreted
   as paths, SQL or executable instructions. Do not overwrite an existing store on
   create, or create an empty store as a fallback for failed load.
2. Coordinate the store lifecycle across processes, then recover/load a consistent
   base and obtain exclusive admission access. Validate against the latest committed
   state while retaining that exclusion through commit. An in-memory snapshot read
   before acquiring access cannot authorize a lost update or stale predecessor.
3. Decode bounded stored values through existing validated constructors and rebuild
   by `CanonicalHistory::append`. Retain the entire original task/revision,
   repository/HEAD/worktree state, capture, coverage, consistency and comparison
   basis. No fresh Git query, source lookup or Evidence assessment occurs on load.
4. Stage the candidate append without publishing it to callers. Core still rejects
   duplicates, aliases, broken references, category/scope mismatch, forks and invalid
   chronology. A global per-store admission ordinal preserves original append order;
   it is separate from caller recording time. Never replay by timestamp, lexical ID
   or unspecified database row order; equal-time and interleaved-entity semantics
   must survive restart exactly.
5. Atomically commit one record, ordinal and necessary container metadata as one
   unit. Initial creation commits identity, format and context coherently. No bulk
   import, cross-store transaction or external-effect runner is implied. Publish
   the new in-memory view only after confirmed admission; discard/reload a staged
   view after storage failure or uncertain completion.
6. Acknowledge admission only after the qualified engine's required synchronization
   boundary. A process exit, successful write call or flushed user buffer alone is
   not that boundary. Distinguish rejection with confirmed non-admission, confirmed
   admission, and indeterminate completion. These are storage outcomes, not PASS,
   applicability or CURRENT. Disk/OS/controller guarantees must be scoped to the
   tested configuration; no universal power-loss claim follows.
7. After an interrupted/uncertain attempt, recover the same store instance and look
   up the existing version ID, comparing the **entire** intended record and context.
   Exact presence confirms that admission without appending again; absence in a
   successfully recovered complete store permits an explicit retry of the identical
   input, subject to Core validation against the latest state. Different content is
   an identity conflict. Missing/replaced/corrupt stores
   or unreadable outcomes remain unresolved, not safe-to-retry absence. Core's
   duplicate-append rejection remains unchanged; reconciliation is a separate operation.
8. Reopening validates container identity, supported format, ordinals, in-store
   predecessor/context relationships and semantic invariants before exposing a
   usable history. A corrupt/unsupported store is never silently truncated, reset
   or salvaged into an authoritative prefix.
   Any later salvage/migration needs explicit scope, privacy handling and separate
   validation. Recovery replays stored facts only, never tools, permissions or checks.

External `SourceReference` observations remain unresolved caller assertions, as in
Core today; their absence from this store is not an invented corruption finding.

Qualified engine rollback of a recognized interrupted transaction is normal recovery,
distinct from accepting a semantically corrupt prefix. Unknown application versions
must not trigger application writes or automatic migration; any engine recovery
needed before that determination remains an explicitly authorized, reported effect.

| Failure boundary | Required observable outcome |
| --- | --- |
| Preflight/Core rejection before storage effects | Confirmed not admitted; accepted state unchanged |
| Transaction failure with confirmed rollback | Confirmed not admitted; original usable state preserved |
| Interrupted write, failed synchronization, commit error or lost acknowledgement without confirmed rollback | Completion indeterminate; withhold the proposed view and reconcile on reopen |
| Successful commit but delivery of its reply is interrupted | Caller still has an uncertain attempt; exact stored presence resolves it without a second version |
| Reconciliation finds same version with different content/context | Identity conflict; no overwrite, renaming, retimestamping or automatic retry |
| Corrupt/unknown format, wrong instance or recovery failure | Typed diagnostic; no usable-history or empty-store success; preserve recoverable files |

The in-memory guarantee that every returned error preserves state does **not** mean
that every durable error proves no commit occurred. An adapter must retain the
distinction in its error/outcome boundary, including a minimal request reference
permitted by policy. Receipts attest only to the observed store operation and
declared storage guarantees, not source authenticity or permanent future retention.

### Core, encoding and concurrency boundaries

Core remains the deterministic admission/query authority, independent of I/O,
database types and serialization dependencies. Start with a cohesive infrastructure
module in the existing CLI library, consuming Core; no new crate, backend trait
framework, storage-neutral DTO authority or CLI command is required by this contract.
Its private decoding representation must reconstruct validated Core values rather
than duplicate temporal-policy decisions. Evidence is not stored or reassessed here.

`ComparisonBasis::material()` is the narrow explicit borrowed projection used for
lossless encoding. It retains redacted default `Debug`; encoding never serializes
Debug, drops comparison material or adds storage dependencies to Core. The handling
policy covers those potentially private bytes.

The Linux library exposes `HistoryStore::create/open/load/admit/reconcile/remove`
and explicit `resume_removal`, `StoreKey`, `StoreId`, `Access`, typed completion
outcomes and removal requests. See [the implementation](../../crates/cortex-cli/src/history_store/mod.rs)
and its Rustdoc example. No live connection or key is retained in a store handle.

Private format 1 uses bounded JSON projections, strict fields/schema/tags and
explicit unknown-version rejection. It is not a public interchange format. Preserve all `u64` values,
exact seconds/nanoseconds, optional fields and Unix path/branch bytes without lossy
UTF-8 conversion, floating-point numbers or unchecked signed casts. Failure to
represent a time/path on the reader's platform is explicit unsupported input.
Platform-tagged local paths do not become portable project identity. Checksums can
detect covered accidental corruption, not authenticate records. Private format
versioning selects no Portable Context Package or stable public protocol. The first
reader may reject every other version; no silent automatic migration is required.

The adapter serializes all cooperating operations, including readers, using a
standard-library exclusive file lock with a five-second contention deadline. Engine
busy errors are immediate and typed; there is no unbounded retry. All cooperating opens,
admissions, recovery and removal must share lifecycle exclusion: database transaction
locks alone do not make unlinking a file with other open handles safe. Whole-store
removal requires quiescing those handles and guarding against replacement/identity
changes; retain any necessary non-sensitive coordination file only under policy.
Caller-owned Core clones remain historical data, not revocable live handles.

Recovery may write store metadata. `Access::Recover` explicitly permits that effect;
`Access::ReadOnly` conservatively refuses a nonempty journal with recovery-required
instead of silently upgrading access. The Git `inspect` command
retains its existing read-only contract and never becomes a store recovery command.
Do not remove an interrupted engine's journal as generic cleanup. Unavailable locks,
unsupported filesystems, symlinks/identity changes or required permission controls
must fail explicitly; no hostile-host sandbox or network-filesystem support is implied.

Initial adapter budgets are one context/collection, existing Core count/text/source
and observation bounds, 64 MiB encoded main-store/input size, 256 MiB managed working
files and a five-second lock-contention budget. The per-record encoded limit is 1 MiB; memory-only temporary storage, checked
managed-file sizes, engine page limits and bounded decoding implement this scope; a stricter encoded limit may reject
an otherwise valid Core value explicitly, never truncate it. Bound decoded counts
and allocations before materialization and bound engine-managed file growth.
These are logical input/working-file limits, not RSS, hard I/O-deadline or performance
guarantees. Permission, busy, capacity, decode, unsupported-version, corruption,
identity-conflict and uncertain-completion diagnostics must remain distinguishable,
bounded and free of raw records, candidate buffers and arbitrary tool transcripts.

### Storage assessment

This compares mechanisms for this bounded store, not alternative product subsystems.
The transactional option is selected for the first adapter. The other alternatives
were assessed, not implemented as additional backends.

| Approach | Correctness/recovery obligations | Privacy and evolution | Dependency/maintenance cost |
| --- | --- | --- | --- |
| Framed append journal | Implement framing/integrity checks, synchronization, partial-tail recognition, exact-prefix replay and exclusion correctly | Deletion needs a qualified rewrite/compaction of segments and copies; headers/codecs need explicit evolution | Small I/O surface but substantial custom crash/recovery and compaction logic; standard-library use does not remove those obligations |
| Complete snapshot with atomic replacement | Write/validate/synchronize a new bounded image, replace under exclusion and synchronize required directory metadata; distinguish pre/post-replacement recovery | Simpler whole-history reconstruction, but old/temp images are additional protected copies; every admission rewrites the image | Plausible small-store alternative; host-specific replacement/locking and recovery protocol remain application responsibilities |
| Local SQLite transaction store | Validate through Core inside serialized admission; retain explicit ordinals; qualify journal/sync settings, error mapping and recovery | Whole-store removal includes engine sidecars/temp data; row deletion alone is not physical erasure; codec/version policy remains CORTEX-owned | Adds a reviewed Rust binding/native engine and build provenance obligations, while delegating page transactions/locking/recovery to one existing engine |

**Selected initial direction:** SQLite via rusqlite with stable bundled SQLCipher
Community and system OpenSSL 3, rollback journaling (`DELETE`) and
`synchronous=EXTRA`. The adapter verifies effective settings and rejects plaintext,
unsupported provider/header/mode/schema/format input. No WAL, virtual tables,
extension loading, attached databases or untrusted SQL interface is provided.
`secure_delete=ON` is not a filesystem/copy-erasure guarantee. Recovery files remain
intact until authorized recovery/removal.

The [accepted decision](0002-durable-history.md) records exact qualified versions,
key handling, Linux constraints, dependency/license costs and reopening conditions.
Selecting an engine delegates page recovery, not CORTEX's admission rules. No
interchangeable backend framework or public persistence schema follows.

### Persistence acceptance and remaining scope

[Storage tests](../../crates/cortex-cli/src/history_store/tests.rs) and
[real-Git integration tests](../../crates/cortex-cli/tests/history_store.rs) exercise
these mechanisms alongside existing Core/002 regressions. Candidate-scoped outcomes
and counts belong in the external handoff. The table specifies the oracle, not a
claim of independent assurance. Actual separate processes cover restart/locking.
Process-kill tests do not establish power-loss durability. Engine capacity, commit
contention, permissions and the [fault cases below](#storage-failure-qualification)
are exercised. Actual device/physical flush failure and power interruption remain
NOT_RUN. Later targeted lifecycle remains planned.

| Case | Discriminating oracle |
| --- | --- |
| Create/restart | New process loads identical context and all validated initial requirement/decision values; failed load never creates an empty replacement |
| Correct/restart | Original and corrected body/provenance/IDs remain; explicit predecessor and append order survive |
| Both time axes | Reuse the existing before/after-correction truth table, half-open boundaries, equal-time ordering and interleaved entities after reload |
| Duplicate/conflicting admission | Identical append is still rejected; changed content under an existing ID cannot overwrite; explicit reconciliation distinguishes exact presence from conflict |
| Invalid references | Missing/forking/cross-entity/category/context/backward-time inputs reject without committed changes |
| Codec boundaries | Full-width integers, nanoseconds, non-UTF-8 paths, absent commit and all coverage/consistency states round-trip or yield explicit platform/limit rejection; comparison material is preserved |
| Corruption/unsupported versions | Alter framing/schema/ordinals/payload/references or version tags; load diagnoses rather than repairs, truncates or exposes a partial authoritative history |
| Interrupted admission | Kill before mutation, during journal/data writes, before commit and after commit before reply; recovery yields a valid old or complete new state, never a partial entry. Once the qualified commit boundary completed, process restart must retain the new state |
| Ambiguous retry | Lose the reply after an observed completed commit: reopen must find the exact admission, without appending again. Earlier uncertain cut points may resolve to confirmed absence and an explicitly revalidated retry; replaced/missing/unreadable stores never establish safe absence |
| Resource/I/O failures | Inject capacity, short-write, flush, permission and busy failures; distinguish confirmed rollback from uncertain outcome; preserve recovery files and bounded diagnostics |
| Concurrent access | Two processes race corrections to one predecessor; at most one succeeds, the other revalidates/rejects; readers see one committed state, not mixed rows |
| Recovery authority | A journal requiring recovery cannot trigger writes through a read-only operation; approved recovery preserves the Core view and creates no assessment |
| Approved whole-store removal | Close/quiesce handles, remove all scoped files/sidecars, reopen yields explicit unavailable store; wrong instance or narrow authorization cannot delete broader data |
| Interrupted removal/copies | Inject interruption between removals; report partial/unknown coverage, preserve unauthorized files and do not restore an old backup automatically |
| Later targeted lifecycle | Only when separately authorized: withdrawal remains distinguishable from removal; redacted payload cannot be recovered at any cutoff, and chain gaps are explicit without fabricated records |
| Privacy/authority regression | Sentinel payload/context never leaks into forbidden logs/receipts; replay does not execute checks/tools, refresh CURRENT, grant permission or promote inference to PASS |

The first operator-scoped adapter now implements explicit create, bounded recovery/
load, one-record admission, exact reconciliation and whole-collection removal with
restart/fault/contention tests and a private codec. Correct/wrong/missing keys,
no plaintext fallback and confidential sentinels in covered at-rest files and process
output have focused tests. Temporary storage is configured to memory; inspected
active store descriptors do not establish exhaustive RAM or host-file coverage.

### Storage failure qualification

[Fault tests](../../crates/cortex-cli/src/history_store/fault_tests.rs) qualify the
existing adapter using isolated synthetic stores and fresh processes. No production
fault API, new backend, dependency or unsafe Rust is introduced.

- Child-only `RLIMIT_FSIZE` with ignored `SIGXFSZ` produces observed native short
  writes followed by kernel EFBIG in journal creation and removal-intent recording.
- A small [test-only C shim](../../crates/cortex-cli/tests/fixtures/storage_fault.c),
  compiled with warnings denied, injects EIO before selected libc write, sync or
  unlink calls for one exact fixture path. This is native-library return injection,
  not a kernel/device failure. Parent tests require a positive fault-hit trace and
  calibrate call order for the qualified stack; missing instrumentation fails tests.
- Initial-file/schema/sync failures never recreate a partial existing location.
  A lost creation reply may leave a complete valid store; explicit reopen validates
  its entire context instead of inferring success or absence from the error alone.
- Failed INSERT with confirmed rollback remains distinct from commit error.
  Journal/database sync and final commit-directory sync failures withhold a receipt.
  Exact reconciliation can find absence or an already committed version, preserving
  prior records, ordinal order and both temporal axes; retry cannot duplicate it.
- A larger interrupted transaction demonstrably changes main pages and leaves a
  hot journal header before exit without destructors. Injected recovery write/sync
  errors are `Io`, not `CannotUnlock`. Fault-free recovery reconstructs the exact
  confirmed Core history; read-only access still refuses recovery without mutation.
- Complete removal intent allows explicit matching-scope resume after intent-sync,
  unlink or intermediate directory-sync failure. A short/empty intent is
  `RemovalIncomplete` and is not silently repaired. If final sync fails after the
  intent was unlinked, completion remains unconfirmed: absent files do not supply
  the missing instance witness, and `resume_removal` rejects it. Those two cases
  require operator inspection or a separately specified reconciliation extension.

The locked SQLite Unix VFS intentionally ignores its journal-creation directory
sync return. Tests observe create/admit success when later database and final
directory syncs succeed. Conversely, failed final journal-deletion directory sync
can report indeterminate completion after the record is already present. The
adapter cannot promise that every underlying sync error is surfaced, nor infer
power-loss atomicity from these observations. This qualification does not change
SQLite/SQLCipher or create a competing transaction authority.

Covered confidentiality sentinels remain absent from managed files and child
diagnostics. Only operation/count/result/errno are traced, never data buffers,
keys or raw paths. Scope is dynamically linked Linux/glibc, the pinned native
stack and these representative failures. Simultaneous/persistent device faults,
lying caches, destructive power interruption, exhaustive fault interleavings and
other host configurations remain unverified. No privileged/global filesystem or
kernel settings are changed. Targeted redaction/withdrawal, per-layer expiry,
backup/export/sync, cross-candidate recovery, new snapshots, graph storage and
workflow durability remain separate work.
