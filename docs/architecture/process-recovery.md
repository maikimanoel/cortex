# Durable process identity checkpoints and recovery inspection

This bounded extension belongs to CAP-CORE-003, CAP-WFL-001/004,
CAP-HRN-001/002, CAP-CTX-002, CAP-MEM-001, CAP-SEC-002 and CAP-CLI-004.
It preserves the [supervised-fixture](supervised-fixtures.md) and
[owned-scope](owned-scope-fixtures.md) ownership boundaries. It supplies no new
capability, operational authority, provider adapter or generally qualified host.

## Accepted behavior and ownership

A future explicitly approved scope fixture may select `BeforeInputV1` in its
immutable process specification. Core owns that policy and its transition guards.
The existing Evidence coordination kernel owns input admission; Storage retains
that same encrypted journal. Harness captures actual launcher/scope observations
and waits for a durable checkpoint before delivering package input. The CLI
projects retained facts without acquiring live host observations or performing
recovery actions. Core remains independent of infrastructure.

The checkpoint records the exact launcher PID/start tick, scope invocation and
cgroup incarnation, verified membership, matching controls and lifecycle,
helper quiescence and populated scope. It is nonterminal, carries no package
input and establishes neither completion nor acceptance. A checkpoint after
cancellation, stale authority, a terminal capture or a conflicting identity is
rejected. Changed operation or observation identities cannot launder a duplicate
receipt into a second launch or checkpoint.

Dispatch uncertainty commits before manager queries or spawn. There is still no
transaction spanning the store and the operating system. A crash between spawn
and the first receipt remains ambiguous. A crash after a scope readback and
before its checkpoint commits cannot be reconstructed into a durable checkpoint.
A committed checkpoint whose acknowledgement is uncertain withholds input until
exact journal reconciliation establishes that same operation. This bounded API
stops the attempt on checkpoint uncertainty; later receipt inspection does not
automatically resume input or adopt the process. Reopening or
replaying the journal never repeats the callback. A restart does not adopt a
process by its retained PID or create a new clock origin for an old effect.

Input release requires the actual checkpoint receipt and a fresh authority check
after append latency. Missing scope/member/helper proof, a refused or unresolved
append, cancellation or deadline exhaustion stops input and enters bounded
cleanup. Selected grace, escalation and cleanup intervals remain finite; a
receipt append cannot refresh them. Uncertain cleanup retains reservations.

## Compatibility and historical uncertainty

The optional policy extends the process-specification encoding. Without it,
original metadata, fixture and supervised records retain their exact encodings
and transition semantics. An older reader must reject an unknown extension;
there is no automatic migration or inference of a checkpoint from an old scope
observation. An explicit new checkpoint API is required for the opt-in policy;
legacy calls cannot silently claim its stronger behavior.

Original observations are immutable. A later missing PID, missing unit or missing
cgroup is an absence observation, not proof that no effect occurred or that an
owned process set became quiescent. Spawned launcher plus missing scope identity
remains Unknown. Neither a zero exit status nor lack of delivered input proves
absence of descendants or helper effects. A new recovery policy cannot clear an
old ambiguous operation by changing its meaning.

## Read-only recovery explanation

`OperationalProjection::inspect_process_recovery` inspects one exact retained
effect and scope. It reports whether the reservation remains held, which launcher,
scope and checkpoint receipts exist, and missing-proof categories. It proposes
an existing `Absent` or `Observed` reconciliation only when the same canonical
process guard accepts the constructed candidate. The projection performs no I/O,
changes no state and confers no permission. Caller assertions and source labels
are not authenticated host evidence.

`Absent` requires an actual retained pre-spawn refusal with `NoChildStarted`.
`Observed` requires a corresponding terminal outcome, exact applicable quiescence
and complete required usage. For the new checkpoint profile, final scope CPU,
charged-memory and task counters must also be known even after interruption.
An identity checkpoint alone never meets those requirements. A structurally
eligible candidate still needs applicable authority and ordinary exact journal
admission. Inspection does not release reservations, dispatch, signal, adopt or
retry. Already settled effects yield no new reconciliation candidate.

The existing `cortex program inspect` command presents those same typed facts.
Its reasons omit private unit/path/inode values and raw payloads. It reads no
manager state and cannot refresh an old observation. Distinguish a captured
checkpoint from current operational authority and actual host qualification.

## Verification and remaining gates

Required source tests cover crash cuts before spawn and identity publication,
missing/lost/duplicate receipts, helper and membership failures, parent exit with
retained descendants/handles, cancellation during acquisition, stale identities,
late captures, replay without launch, retained reservations and legitimate
quiescent recovery. Controlled adapter fixtures are simulation evidence. They do
not qualify cgroups, namespaces, network denial, hard resource limits or recovery
on an actual host.

A failed receipt append can leave the last durable status as `Running`, even
when the returned attempt result is unresolved. That status describes a historical
receipt, not current process liveness. Inspection explains the nonterminal or
missing proof and keeps the reservation; it does not fabricate an uncommitted
Unknown event.

Historical unknown effects remain reserved unless applicable proof independently
satisfies their existing contract. An owner disposition may preserve and archive
an unresolved campaign; it cannot retroactively fabricate quiescence. No new
host attempt may consume an occupied one-attempt slot. Actual host qualification,
protected integration, privacy/provider/credential decisions, live budgets and
all Stage 3/4 work retain their own gates. Capability IDs and milestone states
are unchanged by this source extension.
