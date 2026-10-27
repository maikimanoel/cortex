# Temporal record foundations (003-A, planned)

This prepares the first slice of [003-A](../product/roadmap.md#003-temporal-knowledge-and-memory),
under [CAP-MEM-001](../product/capability-catalog.md#cap-mem-001),
[CAP-SEC-002](../product/capability-catalog.md#cap-sec-002) and
[CAP-CORE-002](../product/capability-catalog.md#cap-core-002). No temporal-record
runtime is delivered here. The catalog owns requirements; this document owns the
bounded domain contract. Public signatures, persisted formats and backends remain open.

## First usable slice and ownership

Implement a deterministic, caller-owned **in-memory canonical requirement/decision
history**: record a version, explicitly correct it, inspect retained versions and
query effective versions at supplied domain and recording times. Bind each collection
to one supplied candidate observation. This is a usable library operation, not an
empty memory crate or storage facade. Separate implementation authorization is
required. This slice does not complete 003-A or CAP-MEM-001.

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
other layers are unsupported here, not silently canonical. Source classification
and handling remain trusted caller obligations, not inferred from author names.

- Domain intervals are half-open `[from, until)`, with an explicit open end when
  needed. Caller-supplied instants are at/after the Unix epoch. Domain validity may
  precede or follow recording. Evidence's `ObservationTimes` chronology describes
  a different concept and must not forbid either case. Core samples no clock.
- A correction preserves logical ID/context, names the current predecessor in that
  record's chain and has a fresh version ID. Recording cannot precede the predecessor's
  recording; equal instants are ordered by the explicit relation. Late recording of
  a correction to an earlier domain interval is supported.
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

Use typed errors for identity reuse, bad interval/time, predecessor/scope mismatch,
unsupported layer, unavailable history and limit exhaustion. Rejection leaves the
accepted collection unchanged. Reapplying an accepted sequence to a new collection
gives equivalent history/query results without I/O or clock dependence; this is
reconstruction, not a crash-safe journal or external-effect replay.

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

Errors and default debug/summary output omit private bodies, raw candidate/task
context and caller-supplied source details; explicit accessors may expose supplied
data. Constructors cannot detect all secrets,
prove consent or authenticate handling labels. Use synthetic/non-sensitive fixtures.
No automatic capture, disk writing, serialization/export, background retention or
payload logging is provided. Dropping ordinary memory is not secure erasure.

## Consequential gates and alternatives

| Boundary | Minimum contract and unresolved choice | Decision point |
| --- | --- | --- |
| Privacy/layers | INV-012/CAP-SEC-002 require inspection/correction, authorized deletion and honest gaps. Owner must choose classification, capture/consent/access, per-layer retention, omission and backup/export handling. Operator profiles versus project policies are alternatives, not selected defaults. Unknown policy blocks persistent capture/disclosure. | DEC-004 before persistent memory; trace policy before 004-D |
| Persistence/replay | Preserve identities, atomic acceptance and corruption/partial-write diagnostics. Local journal, transactional records or snapshot-plus-log are alternatives; no backend, encoding, migration or recovery protocol is selected. Demonstrate crash behavior before claiming durability. | DEC-007 before the first durable adapter in 003-A |
| Candidate/snapshot | Reuse bounded 002-A/B observations. Clean-commit-only extraction and a separately observed immutable dirty manifest are possible later scopes; equal HEAD/status selects neither. Full 003-A still needs the scheduled snapshot decision before that extension. | DEC-011 before snapshots; extractor coverage before 003-B |
| Time/identity | Record/version, domain/recording time and task/candidate/capture references remain distinct. Internal values select no persisted-format version, portable project ID or cross-branch total order. | Minimum rules above; durable identity/format under DEC-007 |
| Evidence/invalidation | 002-C direct checks remain unchanged. Supersession supplies changed references; transitive invalidation/non-impact and incomplete dependency coverage belong to CAP-EVD-003. A lookup cannot refresh PASS. | 003-C; no second evidence authority in 003-A |
| Portable handoff | CAP-MEM-003/CAP-CTX-002 require integrity, candidate correspondence and privacy-aware omission. Digests and authenticated signatures differ; no package schema/authenticity mechanism is selected. | DEC-004/007 before 003-E/003-F |

Logical append-only correction describes retained history, not permanent physical
retention or an override of authorized deletion. Future deletion must propagate to
indexes/caches/exports under selected policy; tombstones must not recreate erased
content. Unavailable backups/history remain explicit gaps. This kernel provides
neither privacy-deletion enforcement nor production persistence.

## Observable acceptance for the next implementation

These are **NOT_RUN planned cases**, not evidence of delivered code.

| Case | Discriminating observation |
| --- | --- |
| Record/correction | Both immutable versions/provenance remain inspectable; the old value is not overwritten |
| Late correction | A cutoff before recording returns the earlier perspective; a later cutoff for the same domain instant returns the correction |
| Interval boundaries | Inclusive start, exclusive/open end behave exactly; outside a correction's interval an applicable predecessor survives |
| Invalid input | ID reuse with different bodies, duplicate submission, bad intervals, missing/forking predecessors and backward recording time yield typed errors and unchanged state |
| Reconstruction | The same accepted sequence gives equivalent history/queries without clock/I/O dependence |
| Candidate distinctions | Existing real-Git adapters preserve clean/dirty, detached/unborn and incomplete context; equal dirty HEAD cannot become a snapshot or CURRENT assessment |
| Missing scope/history | Wrong context or unavailable required history is explicit; no fabricated record, commit, authenticity or N/A result |
| Limits/privacy | Exact bounds pass; the next byte/reference/version fails atomically; diagnostics/debug omit a private-body sentinel |
| Compatibility | Existing 002-A/B/C/D read-only, binding, evidence, CLI and compile-fail regressions pass; history neither rewrites evidence nor grants authority |

Core owns domain tests. Real-Git boundary tests belong in the CLI consumer using
existing fixtures, preserving dependency direction. Keep invariants documented and
errors cohesive; no generic framework/new dependency is justified. Full 003-A still
needs privacy/layer and snapshot gates; persistence needs actual recovery/deletion
tests. Extraction, cross-candidate recovery, portable packages, rule registries,
workflows and empirical evaluation retain later roadmap exits.
