# GPE-B Stage 1 persistent coordination kernel

This bounded implementation path is authorized under the existing capability
owners: CAP-CORE-003, CAP-WFL-001/002/004, CAP-EVD-003, CAP-MEM-001,
CAP-SEC-002, CAP-CTX-001/002 and CAP-CLI-004. It does not complete those
capabilities or authorize an executor. G1, A-1 and P0 retain their advisory scope.

## Acceptance contract

An explicit local operator can create an encrypted, bounded program store,
retain its accepted baseline and immutable graph revisions, admit exact
expected-version coordination inputs, inspect the reconstructed state and
reconcile an uncertain admission. A failed admission changes no canonical state.
Conflicting claims cannot acquire the same declared responsibility. Late attempt
results cannot satisfy a newer generation. Reopening does not infer completion.

Dependency release requires A-1 inspection of the complete retained evidence
basis and a distinct scoped operator coordination decision. Releases apply only
to the named non-effectful consumer. Intermediate release does not complete its
producer. Evidence, validity, approval and execution authority remain distinct.
Historical decisions remain inspectable after invalidation. Input revisions
cannot silently change an accepted baseline or reuse an identity for new bytes.

Instruction packages deterministically retain accepted requirements, candidate,
explicit sources, omissions, method proposals, dependency state and stop rules.
They confer no authority. Scheduling is an inspectable simulation with explicit
capacity and verification reserves; unknown required capacity holds a proposal.

Tests must reject stale revisions, duplicate/conflicting identities, unsupported
effects, missing evidence, stale candidates, conflicting owners, capacity
overflow and malformed storage. Legitimate fan-out, independent work and explicit
non-impact findings must remain usable. Separate-process recovery, concurrency,
lost replies and CLI read-only behavior require observations on this schema.

## Ownership and storage boundary

Core owns validated contracts and pure transitions. Evidence composes those
transitions with its existing support/validity mechanisms. `cortex-storage`
owns the SQLCipher lifecycle and separate history/program codecs; it does not
decide acceptance. CLI composes these APIs. Core remains std-only. The existing
`cortex_cli::history_store` facade and history format 1 remain compatible.
No runtime crate or dispatch entry point is introduced in Stage 1.

The supported store profile is one trusted local Linux operator and cooperating
clients on a responsive local filesystem. Keys and locations are explicit;
there is no default location, plaintext fallback, automatic export or migration.
Program input counts and encoded sizes are finite caller selections. Data is
encrypted from its first persisted record. SQLCipher DELETE journaling and
synchronous EXTRA define the selected engine boundary, not universal physical
durability. Read-only inspection refuses recovery writes. Missing or replaced
stores cannot establish that an uncertain operation was absent.

Local operator assertions are not authenticated multi-user authorization.
No process, descendant, network, resource isolation, external effect or
exactly-once execution guarantee follows from this kernel. Stage 2 requires
separate effect-protocol, host, privacy, budget and authority decisions and
actual qualification. Protected merges remain owner-controlled.

## Contracts and replay

`cortex_core::program::ProgramDefinition` owns the exact program-wide objective/acceptance, baseline
revision/predecessor, approval reference, immutable G1 graph, workstream ownership,
task increments and declared exclusive claim-scope identities. Every graph task
has exactly one workstream and at least one increment. Every dependency artifact
belongs to an increment of its declared producer. The original single-criterion
`TaskContract` is preserved. No new capability identifiers are introduced.

`ProgramState` separates that baseline from `OrderingPlan`, which can only reorder the
same tasks through `ordering_plan()` / `set_ordering_plan()`.
`CoordinationPermission::Plan` retains its broader metadata responsibility,
including capacity and package preparation. Baseline amendments require explicit `Amend` scope and retain their
predecessor. Stage 1 preserves workstream/task/increment responsibility membership;
artifact revisions and task contracts may change. Affected consumers and their
transitive closure are reset. Unchanged components replay their original inputs;
explicit non-impact must name exactly the unaffected complement. Unknown declared
coverage cannot establish non-impact. This is reasoning over declared contracts,
not automatic semantic impact discovery or live source verification.
An amendment cannot reuse any retained task/revision identity for different
objectives, acceptance criteria or candidate requirements, even after an intervening
baseline. An exact historical contract may be selected by a new accepted baseline.
G1's artifact alias rules also apply across retained baselines: an artifact ID
keeps its producer, and its revision cannot acquire a different candidate or kind.

`CoordinationGrant` binds the claimed operator, exact program/baseline, task
scope and metadata permissions. Changed reuse of a grant identity is refused.
The trusted local caller supplies these assertions; neither signatures nor a
multi-user permission service are implemented. A grant covering an invalidation
root must also cover every task whose projected state changes. The accepted
baseline remains separate from mutable plans and external text.
Sender and recipient grants in one handoff must also agree when they share an ID;
the check precedes any change to either attempt.

Claims serialize ownership of the same task/increment or any shared declared
claim scope (`ClaimScopeId`). These declared semantic exclusion keys are not
physical resource-pool identities. Claiming marks the G1 task active, without running it or
making it accepted. Handoff requires both scoped operator acknowledgments, closes
the original attempt and retains a distinct new attempt. Unknown attempts continue
to block replacement. Reports require exact baseline/generation correspondence;
stale attempts may be explicitly cancelled to release metadata ownership. Reports
and cancellation do not prove external completion, rollback or task acceptance.
Stage 1 does not admit whole-task acceptance or protected integration commands.
`TaskAcceptance` edges therefore remain held. `program.*` notification names
are reserved for derived claim lifecycle records.

`cortex_evidence::program::CoordinationKernel` is the canonical pure composition
boundary. Its program journal has logical Workflow ownership; it lives with its
Evidence consumer to compose A-1 without a Core-to-Evidence dependency or an empty
runtime crate. Core still owns the guards and Evidence still owns validity.
It retains original observations, terminal assessment inputs, examinations,
complete A-1 contracts/witnesses, coordinator decisions and operator grants.
An `Operation` includes its unique ID and exact expected predecessor ordinal.
Identical replay returns the original ordinal without consuming another slot;
changed identity reuse or stale expected state refuses atomically. No raw G1
review can bypass this kernel's A-1 examination. Core's independent G1 API remains
advisory and does not become a second persistence/authority path.

A release applies only to its exact non-effectful consumer. Every required
predicate and assigned witness must have applicable, current support under the
supplied correspondence, and the distinct scoped coordinator review must be
admitted by G1. Negative support findings are retained. A missing-input proposal
cannot revoke a previously valid release by itself. Explicit contextual
invalidation holds the affected current consumers while preserving terminal
results and release history. Invalidating an older record does not revoke a newer
release that uses different evidence. Re-evaluation updates only tasks with
changed inputs/incoming decisions; logical graph scans and bounded replay still
have costs, and no performance improvement is claimed.

## Encrypted program format and lifecycle

`cortex_storage::program_store::ProgramStore` uses `program.db`, schema version 1,
and policy `local-operator-program-metadata-v1`. The canonical history store keeps
`history.db`, its original schema/policy and the compatibility facade. Neither
store silently opens or migrates the other. Program tables retain the initial
validated definition and ordinal, operation identity and complete canonical input
bytes for each subsequent admission. Derived projections are rebuilt through
Core/Evidence, never deserialized as trusted acceptance state. Canonical encoding
rejects changed/extra fields, malformed revisions and conflicting identities.
New inputs must reconstruct through the same codec before an append is acknowledged.
Domain-equal path spellings retain their original bytes in frozen support scopes;
replay does not replace them with a different equivalent representation.

The lifecycle lock spans opening, exact incarnation checking, reconstruction,
Core/Evidence validation and an IMMEDIATE transaction append. A confirmed receipt
follows the selected SQLite commit synchronization. A failed commit acknowledgment
is indeterminate. Reconciliation compares store incarnation, complete initial
contract, complete operation and expected predecessor; missing, corrupt or replaced
stores are errors, never confirmed absence. There is no cross-store transaction.
A changed input needs a new operation; retry is always an explicit caller action.

The caller supplies positive retained baseline/entry/encoded-byte bounds. Aggregate
encoded initial plus operation bytes must fit the program and caller read limits;
capacity refuses rather than evicts. Program bytes are capped by the inherited
64 MiB engine/input envelope. The inherited managed-file check remains 256 MiB;
these are separate from logical inputs and are neither global disk nor RSS quotas.
Lock waiting is bounded by the smaller of the supplied read timeout and five
seconds. Cooperative deadlines are checked during replay and around engine calls;
blocked kernel I/O is not preempted. All claims assume the stated responsive local
filesystem profile.

Stores must be explicit private directories outside declared source repository
roots and Git metadata. Exclusion is checked at creation, on proposed amendments
and against every retained baseline during reconstruction, including relocated
stores. Keys are caller-supplied raw 32-byte values; length/nonzero
validation does not establish entropy. Data is encrypted from the first record.
No default key, key recovery, raw trace capture, automatic backup, sync, export,
provider disclosure or retention eviction is supplied. Manual whole-store removal
requires the exact observed incarnation and retains a minimal random-ID intent
until managed payload removal is complete. Empty directory/lock remain. Caller
copies, backups, swap, filesystem snapshots and physical sectors are outside the
removal guarantee. No hostile same-user or universal power-loss protection is
claimed. The original failure is retained when a later observation resolves it.

## Packages and simulation

`program::context::assemble` emits deterministic versioned, length-delimited
instruction bytes. It fills objective/acceptance/candidate/baseline/generation,
graph, increment and resource responsibility from canonical state; retains exact
explicit source text/version and repository/worktree/commit correspondence, omissions, method applicability/exception proposals,
dependency artifact/predicate/release state, deliverables and stop rules. Exact
package identity/revision/bytes are retained with the journal. Structural checking
compares current reassembly with the original and proposed submitted bytes. It does
not prove natural-language fidelity, source truth, executor compliance or method
qualification. Qualification references remain caller references; P0 is unchanged,
and automatic selection, adoption and trusted blocking remain planned. No package
is submitted to an external model by Stage 1. This first explicit-source profile
requires each source to match the target task's repository/worktree/commit
requirement; other-candidate acquisition/translation remains a later context
adapter obligation, with missing context declared in omissions.

`program::scheduling::simulate` consumes explicit owner priority bands, waiting
rounds, risk/cost ranks and required resource demands. Its disclosed order is band,
waiting age, downstream-unblocking count, risk, coordination cost and stable task ID.
It reserves declared verification capacity in the simulation before subtracting
worker proposals. `WorkerSlots`, `BuildJobs`, `TestProcesses`, `CpuMillicores`,
`RamBytes`, `DiskBytes`, `ElapsedMillis`, `OutputBytes`, `CiJobs` and `ModelTokens`
are separate dimensions. Worker slots are declared logical capacity units; their
reusable or cumulative treatment belongs to the consuming profile. They are not
observed people, processes or threads. Missing demand/capacity or an undeclared
enforcement ability holds a required dimension. These assertions are not resource measurement or host
qualification. Results are proposals; authorized/reserved/dispatched remain false.

## Operator interface

Rust consumers compose the validated definition and `CoordinationKernel`, then
explicitly call `ProgramStore::create`, `admit`, `load`, `inspect` or `reconcile`.
The module-qualified `program_store::AdmissionReceipt` identifies the retained
operation, store incarnation and original ordinal, including duplicate or
reconciled-present admission. It is distinct from the history store receipt, an
in-memory kernel receipt and a closure receipt. It does not assert a new append,
external completion, acceptance or permission.

Mutations always require complete typed inputs and expected state. There is no
implicit persistent location or automatic ingestion of a chat/planner output.

The Linux CLI has one new read-only command:

```text
cortex program inspect --store /explicit/private/program \
  --program program-id --key-fd 0 --max-bytes 16777216 \
  --read-timeout-ms 10000 --json
```

Numbers here illustrate explicit caller choices, not production defaults. Descriptor
0 must supply exactly 32 raw key bytes followed by EOF; another inherited readable
regular-file/pipe descriptor can be selected. Keys never enter arguments or
configuration lookup. The command refuses a missing/mismatched program, malformed
key, deadline expiry or required journal recovery. It does not recover, dispatch,
review, approve or recapture Git. The read timeout is cooperative for storage; pipe
key acquisition uses a nonblocking read with an explicit elapsed deadline.

### Read-only CLI schemas

Omitting `--schema-version` or selecting `--schema-version 1` retains the existing
human and machine contracts. Structured output schema `cortex.program.v1` is
separate from check JSON v1. It exposes baseline/journal identity, workstreams,
exact task candidates, dependency readiness versus eligibility, per-edge release/hold reasons, attempts,
unknown metadata outcomes, historical support decisions and package correspondence.
It explicitly reports unavailable execution authority, absent resource reservations,
unsupported integration and unperformed live verification. Human output summarizes
tasks/blockers and attempts, retaining the existing `Mission` label in version 1.
`--schema-version 2` selects canonical terminology in either human output or, with
`--json`, machine output. Schema selection does not change inspection or accounting.

| Projection | Explicit version 2 representation | Preserved meaning |
| --- | --- | --- |
| Program and workstreams | `cortex.program.v2`, `tasks`, `task`, `task_status` | Same task identities and coordination states, separate from attempts |
| Resource quantities | Canonical names such as `WorkerSlots` and `CpuMillicores` | Same numeric values, units and profile-specific accounting |
| Fixture/supervised projections | `cortex.fixture-effects.v2`, `cortex.supervised-fixtures.v2` | Same bounded effect records and observations |
| Domain projection | `cortex.domains.v2` | Same retained domain facts; no manager contact |
| Guidance | `next_step_guidance`, scope `RETAINED_STATE_ADVISORY_NOT_EXHAUSTIVE` | Conditional information, not authorization or an exhaustive prerequisite check |
| Retained binding | `prepared_handle_availability: UNESTABLISHED_BY_JOURNAL` | The journal does not establish a current usable controller channel |
| Closure receipt | `closure_receipt_recorded`, `closure_receipt_status`, `closure_receipt_source` | Receipt retention and declared source, not authenticated physical closure |
| Native samples | `native_support_status: INCOMPLETE`, `latest_direct_observation`, `latest_recursive_observation`, `retained_native_observation_count` | Separate bounded samples, combined count and retained proof gaps; see the recursive presentation contract below |

Receipt status is `CLOSURE_RECEIPT_RETAINED` or `NO_CLOSURE_RECEIPT_RETAINED`; an
absent receipt has no asserted receipt source. Native support remains separate
from any caller-asserted receipt and actual resource disposition. Its guidance is
`DO_NOT_USE_PARTIAL_NATIVE_SAMPLES_AS_CLOSURE_OR_SETTLEMENT`. Neither sample
presence nor schema selection grants closure, settlement, retry or new admission.
The [domain contract](recoverable-domains.md) defines the retained proof boundary.

Missing, unsupported or repeated schema values are refused as invalid arguments
before key acquisition, without echoing those values. The accepted values are
exactly `1` and `2`; the parser does not normalize an unsupported spelling. Other command families, including check JSON, retain their
existing output contracts. Exit 0 means inspected metadata, not engineering
acceptance; 2 means invalid arguments, 3 means a store/key/inspection refusal, and
1 means output failure. Output is requested local private program content; avoid
redirecting it into public logs.

## Acceptance mechanisms and remaining gates

| Program artifact | Capability responsibility | Mechanism and discriminating cases |
| --- | --- | --- |
| C1 | CORE-003, WFL-001/004 | `cortex-core/tests/program.rs`: exact membership, protected plans, scoped grants, amendments, stale attempts |
| D1/V1 | EVD-003, WFL-004 | `cortex-evidence/tests/program.rs`: actual A-1 consumption, intermediate release, fan-in/out, false non-impact, stale/older evidence and replay |
| S1/O1 | MEM-001, SEC-002, WFL-001 | `cortex-storage/src/program_store/tests.rs`: encrypted reconstruction, competing processes, commit crash cuts, injected native I/O errors, capacity, corrupt/replaced stores and scoped removal |
| P1/K1 | CTX-001/002, WFL-002 | Core package tests and persisted package reconstruction; explicit proposals only, wrong candidate/altered bytes refused |
| Q1 | WFL-004 | Core simulation tests: independent dimensions, verification reserve, unknown/unenforceable limits and legitimate contention neighbors |
| I1/F1-kernel | CLI-004, WFL-001/004 | CLI real-command tests plus process fixtures: blocked/eligible/unknown views, key deadlines, no hidden recovery or file changes |

Test source identifies mechanisms; candidate-scoped execution records belong in
the delivery handoff. Process kill and test-local libc fault interposition do not
establish physical power-loss guarantees or an executor sandbox. CI and local
self-review are not independent engineering acceptance. All 36 catalog capability
statuses and milestone exits retain their broader obligations.

Stage 1 delivery ends at a verified reviewable source candidate and permitted PR;
protected integration/owner acceptance remain external to this implementation.
Stage 2 requires explicit effect-intent/reconciliation schema and retry rules,
Core operation authority, numeric enforceable budgets and review reserves, trace/
provider privacy policy, approved host qualification experiments, actual Codex
executable/transport and host denial qualification, and applicable method/reviewer
policy (DEC-004/005/006/007/012). A real-host resource or revocation claim cannot
be inferred from these simulations. Stage 3 requires connected multi-worker and
protected integration evidence; Stage 4 requires independent operational acceptance
and a new finite roadmap program. No stage activates automatically on upgrade.

### Recursive observation presentation

Schema 1 remains the default with unchanged bytes for previously supported states.
For newly retained recursive observations, its existing native inspection fields
provide a bounded projection: `retained_observations` counts direct plus recursive
records, `latest` still describes only the last direct sample (or null), and
`native_observation_missing_proofs` includes the combined Core assessment. The
legacy direct membership label describes that direct sample, not the scope of all
retained evidence. No recursive object or canonical field is added to schema 1.

With `--schema-version 2`, `native_closure_inspection` exposes
`latest_direct_observation` and `latest_recursive_observation` separately. Either
can be null. `retained_native_observation_count` counts both record kinds;
`direct_sample_membership_coverage` and `recursive_sample_membership_coverage`
describe their separate non-atomic coverage. `combined_native_inspection_basis`
is `LATEST_DIRECT_OR_RECURSIVE_SAMPLE_AND_LATEST_RECURSIVE_POPULATION_AND_MEMBERS`:
the existing Core algorithm uses the later direct/recursive sample for sample
conditions, and retains population/member gaps from the last recursive tree even
when a later direct sample is empty. The displayed direct sample is never replaced
by the recursive sample. Neither ordering proves closure or final consumption.

The additional recursive proof flags remain false and `native_support_status`
remains `INCOMPLETE` for this bounded native support. A retained admission receipt
for an observation is distinct from a closure receipt. Guidance remains advisory,
and journal bindings do not establish a live prepared handle. All other schemas,
exit codes, redaction and read-only inspection behavior remain unchanged.
