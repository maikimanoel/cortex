# Temporal record foundations (003-A, first in-memory slice)

This defines the implemented first slice of [003-A](../product/roadmap.md#003-temporal-knowledge-and-memory),
under [CAP-MEM-001](../product/capability-catalog.md#cap-mem-001),
[CAP-SEC-002](../product/capability-catalog.md#cap-sec-002) and
[CAP-CORE-002](../product/capability-catalog.md#cap-core-002). The catalog owns
requirements; this document owns the bounded domain contract implemented in
[`cortex_core::temporal`](../../crates/cortex-core/src/temporal/mod.rs).
This is partial 003-A/CAP-MEM-001 progress, not a durable memory system.
Persisted formats, backends and broader public compatibility commitments remain open.

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
| `cortex-cli` | Existing observation producer and commands unchanged; real-Git consumer tests can supply observations to Core |

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
collection. This slice adds no storage reader or external source resolver.

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
and queries. No new crate, dependency, CLI command or Evidence API is introduced.

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
recoverable-memory guarantee. Queries borrow retained data without allocating or
mutating it; traversal is bounded by the retained-version limit. The caller must
reconstruct the same accepted sequence to obtain equivalent in-memory history;
the API does not supply persistence or revalidate the observation.

## Bounds and privacy

Initial internal limits: 128-byte ASCII IDs, 4096-byte bodies/source descriptions,
32 source references per version, 1024 retained versions and 4 MiB aggregate retained
record text/reference bytes. Distinct record-ID types use the existing TaskId
character syntax; bodies/source descriptions use the existing nonblank/control-character
rules (line feed permitted). Validate before copying into accepted state. Count
superseded versions; do not evict history to fit. Bound query output/traversal by the
version limit. Existing observation bounds apply separately; retain context once
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
No automatic capture, disk writing, serialization/export, background retention or
payload logging is provided. Dropping ordinary memory is not secure erasure.

## Consequential gates and alternatives

| Boundary | Minimum contract and unresolved choice | Decision point |
| --- | --- | --- |
| Privacy/layers | INV-012/CAP-SEC-002 require inspection/correction, authorized deletion and honest gaps. The [lifecycle contract](#privacy-lifecycle) separates those operations; [owner policy gates](#owner-policy-gates) remain unapproved. Unknown policy blocks persistent capture/disclosure. | DEC-004 before persistent memory; trace policy before 004-D |
| Persistence/replay | The [durable admission contract](#durable-admission) defines atomic acceptance, uncertain completion and reconstruction. The [storage assessment](#storage-assessment) recommends one local transactional adapter; backend qualification, encoding and migration implementation remain open. Demonstrate crash behavior before claiming durability. | DEC-007 before the first durable adapter in 003-A |
| Candidate/snapshot | Reuse bounded 002-A/B observations. Clean-commit-only extraction and a separately observed immutable dirty manifest are possible later scopes; equal HEAD/status selects neither. Full 003-A still needs the scheduled snapshot decision before that extension. | DEC-011 before snapshots; extractor coverage before 003-B |
| Time/identity | Record/version, domain/recording time and task/candidate/capture references remain distinct. Internal values select no persisted-format version, portable project ID or cross-branch total order. | Minimum rules above; durable identity/format under DEC-007 |
| Evidence/invalidation | 002-C direct checks remain unchanged. Supersession supplies changed references; transitive invalidation/non-impact and incomplete dependency coverage belong to CAP-EVD-003. A lookup cannot refresh PASS. | 003-C; no second evidence authority in 003-A |
| Portable handoff | CAP-MEM-003/CAP-CTX-002 require integrity, candidate correspondence and privacy-aware omission. Digests and authenticated signatures differ; no package schema/authenticity mechanism is selected. | DEC-004/007 before 003-E/003-F |

Logical append-only correction describes retained history, not permanent physical
retention or an override of authorized deletion. Future deletion must propagate to
indexes/caches/exports under selected policy; tombstones must not recreate erased
content. Unavailable backups/history remain explicit gaps. This kernel provides
neither privacy-deletion enforcement nor production persistence.

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
needs privacy/layer and snapshot gates; persistence needs actual recovery/deletion
tests. Extraction, cross-candidate recovery, portable packages, rule registries,
workflows and empirical evaluation retain later roadmap exits.

## Durable history extension: contract only

The following requirements govern a **future**, separately authorized implementation
under CAP-MEM-001 and CAP-SEC-002. No storage, lifecycle operation, recovery, format
or new authority mechanism is implemented by this documentation checkpoint.
DEC-004 remains open for owner policy; DEC-007 is resolved only for the technical
admission/reconstruction obligations below, not a backend or public protocol.

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

Every recommendation below is **unapproved**. Missing answers block real persistent
capture/disclosure; documenting an option does not select it. Broader trace capture,
team governance and synchronization policies remain at their original milestones.

| Question | Concrete alternatives | Recommended first profile and consequence |
| --- | --- | --- |
| Capture/classification | Explicit submitted canonical data only; or automated/expanded capture with classification and consent controls | Explicit operator-submitted, declared non-sensitive requirements/decisions and their full necessary candidate context only. No transcripts, environment secrets or automatic working-context capture. Classification is a caller declaration, not proof that a secret is absent |
| Access/location | One operator-owned local directory; or shared/project-managed storage with a defined identity/access model | One explicit private directory outside the inspected repository, controlled by the operator, with restrictive host permissions. No implicit current-directory/default store, account or remote service. Owner must approve the operator/delegate model and whether local plaintext is acceptable |
| Retention/layers | Manual retention until authorized removal; or explicit per-layer expiry schedules | Canonical history retained until explicit removal, subject to finite capacity with no silent eviction. Transient/ephemeral material is not admitted by this adapter; their future durations remain open. Capacity limits are not retention policy |
| Removal/remnants | Whole-collection deletion first; or targeted redaction/deletion with gap-aware views. No persisted receipt; or expressly permitted non-sensitive receipt | Whole-collection deletion as the first privacy mutation, with explicit approval of its broader scope; finer redaction/withdrawal deferred. Recommend no persistent payload-derived receipt by default; owner must approve any coordination/remnant metadata. Never expand a narrow deletion request to unrelated data automatically |
| Deletion/confidentiality assurance | Removal from covered managed files with disclosed residual-media/memory risks; or a stronger forensic threat model requiring additional protection | Declare managed-file removal only. OS snapshots, unallocated blocks, SSD remapping, swap, crash dumps and caller-held in-memory clones are not securely erased by ordinary deletion. Stronger requirements block this profile pending an encryption/key/lifecycle design; none is selected here |
| Copies/restoration/disclosure | No automatic copies; or registered managed backups/exports/synchronization with propagation and restoration rules | No automatic backup, export, import or sync. Operator must identify pre-existing external copies and permitted inspection/receipt disclosure. Future restore must reconcile removal directives or refuse activation when their absence cannot be established; external copies cannot be silently declared deleted |

The selected policy must have an inspectable version/reference distinct from task,
record and storage-format versions. Changing policy requires explicit review, not
an implicit rewrite of historical consent. Unknown policy blocks dependent actions;
it does not prevent unrelated in-memory operations. Human inspection/export is
disclosure: explicit getters do not authorize unrestricted dumps or raw diagnostics.
Engine journals and staging files are protected store data within the approved
capture/removal scope, even when external backups are disabled; they are not an
exception to minimization or deletion accounting.

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

There is one concrete API prerequisite for a lossless adapter:
`ComparisonBasis::material` is private and currently has no accessor. A later
implementation must review a narrow explicit borrowed accessor or equivalent
projection, retaining redacted default `Debug`. It must not drop that material,
serialize `Debug`, infer replacement provenance or add storage dependencies to Core.
The handling policy must cover those bytes, which can contain private repository
metadata. This checkpoint changes no Rust API.

The first codec needs a private format/version identifier, bounded length/count
validation and explicit unknown-version/tag rejection. Preserve all `u64` values,
exact seconds/nanoseconds, optional fields and Unix path/branch bytes without lossy
UTF-8 conversion, floating-point numbers or unchecked signed casts. Failure to
represent a time/path on the reader's platform is explicit unsupported input.
Platform-tagged local paths do not become portable project identity. Checksums can
detect covered accidental corruption, not authenticate records. Private format
versioning selects no Portable Context Package or stable public protocol. The first
reader may reject every other version; no silent automatic migration is required.

Allow one writer and transaction-consistent readers; contention has a finite busy
deadline and explicit retryable error, not unbounded retry. All cooperating opens,
admissions, recovery and removal must share lifecycle exclusion: database transaction
locks alone do not make unlinking a file with other open handles safe. Whole-store
removal requires quiescing those handles and guarding against replacement/identity
changes; retain any necessary non-sensitive coordination file only under policy.
Caller-owned Core clones remain historical data, not revocable live handles.

Recovery may write store metadata. A future `recover-and-load` operation must declare
that effect and require write authority; a truly read-only inspection must report
recovery-required instead of silently upgrading access. The Git `inspect` command
retains its existing read-only contract and never becomes a store recovery command.
Do not remove an interrupted engine's journal as generic cleanup. Unavailable locks,
unsupported filesystems, symlinks/identity changes or required permission controls
must fail explicitly; no hostile-host sandbox or network-filesystem support is implied.

Initial adapter budgets are one context/collection, existing Core count/text/source
and observation bounds, 64 MiB encoded main-store/input size, 256 MiB managed working
files and a five-second lock-contention budget. Qualify enforcement, temporary-file
accounting and codec fit before implementation; a stricter encoded limit may reject
an otherwise valid Core value explicitly, never truncate it. Bound decoded counts
and allocations before materialization and bound engine-managed file growth.
These are logical input/working-file limits, not RSS, hard I/O-deadline or performance
guarantees. Permission, busy, capacity, decode, unsupported-version, corruption,
identity-conflict and uncertain-completion diagnostics must remain distinguishable,
bounded and free of raw records, candidate buffers and arbitrary tool transcripts.

### Storage assessment

This compares mechanisms for this bounded store, not alternative product subsystems.
None has been implemented or qualified for CORTEX durability/privacy acceptance.

| Approach | Correctness/recovery obligations | Privacy and evolution | Dependency/maintenance cost |
| --- | --- | --- | --- |
| Framed append journal | Implement framing/integrity checks, synchronization, partial-tail recognition, exact-prefix replay and exclusion correctly | Deletion needs a qualified rewrite/compaction of segments and copies; headers/codecs need explicit evolution | Small I/O surface but substantial custom crash/recovery and compaction logic; standard-library use does not remove those obligations |
| Complete snapshot with atomic replacement | Write/validate/synchronize a new bounded image, replace under exclusion and synchronize required directory metadata; distinguish pre/post-replacement recovery | Simpler whole-history reconstruction, but old/temp images are additional protected copies; every admission rewrites the image | Plausible small-store alternative; host-specific replacement/locking and recovery protocol remain application responsibilities |
| Local SQLite transaction store | Validate through Core inside serialized admission; retain explicit ordinals; qualify journal/sync settings, error mapping and recovery | Whole-store removal includes engine sidecars/temp data; row deletion alone is not physical erasure; codec/version policy remains CORTEX-owned | Adds a reviewed Rust binding/native engine and build provenance obligations, while delegating page transactions/locking/recovery to one existing engine |

**Recommendation, not backend approval:** use one local SQLite adapter with rollback
journaling (`DELETE`) and `synchronous=EXTRA`, after owner policy and dependency/host
qualification. Verify effective settings rather than assuming defaults or ignoring
unsupported options. Prefer no WAL, virtual tables, extension loading, attached
databases or untrusted SQL in this slice. Ordinary-table `secure_delete=ON` can be
part of a qualified removal procedure; it is not a complete filesystem/copy-erasure
guarantee. Keep engine recovery files intact until authorized recovery/removal.

The rationale is to avoid maintaining a second transaction/recovery engine while
preserving a small local store. The cost is a native dependency, extra protected
files and a required safe-binding/build/license/toolchain review. No Cargo dependency
is selected or added here. Finalize the private codec and store-instance/lifecycle
coordination mechanism in the separately authorized implementation contract.
Reopen the recommendation if an approved privacy threat model requires stronger
protection, native-engine constraints are unacceptable, or measured/required
multi-writer/host support changes. No distributed store or interchangeable backend
framework follows from this choice.

### Future persistence acceptance

All cases below are **NOT_RUN future acceptance**, separate from the executed
in-memory/002 regression suite. Tests must use controlled temporary stores and
actual separate processes where restart/locking is the property. Process-kill tests
alone do not establish power-loss durability; injected I/O/flush faults or an
appropriate filesystem/engine fault harness must state its modeled limits.

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

The recommended first persistence implementation is one operator-scoped local
adapter: explicit create, bounded recover/load, one-record admission, exact
reconciliation and owner-approved whole-collection removal, with restart/fault/
contention tests and a private versioned codec. It depends on the owner answers
above, backend/binding qualification and the narrow Core projection review.
Until then it is not authorized or ready for real private-data capture. Targeted
redaction/withdrawal, per-layer expiry, backup/export/sync, cross-candidate recovery,
new snapshots, graph storage and workflow durability remain outside that increment.
