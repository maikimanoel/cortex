# Owned process scopes for provider-free qualification

This bounded extension of the [supervised fixture contract](supervised-fixtures.md)
belongs to CAP-CORE-003, CAP-WFL-001/004, CAP-HRN-001/002, CAP-CTX-002,
CAP-MEM-001, CAP-SEC-001/002 and CAP-CLI-004. It introduces no additional
capability identifier, workflow authority, provider or permission service.
Source implementation, fixture verification and actual host qualification are
separate acceptance boundaries.

## Observable contract and ownership

An explicitly approved owned fixture can request an exact temporary user scope.
The existing encrypted Workflow journal retains that request and commits dispatch
uncertainty before the native Harness invokes the operating-system manager.
Exact replay does not repeat the invocation. A changed request, missing resource
reservation, revoked grant or unavailable prerequisite refuses the operation.

Core owns the typed request and transition guards. Evidence's coordination kernel
remains the canonical journal input owner; Storage retains its existing locking,
encrypted admission and replay contract. Harness owns the manager adapter and
process lifetime. CLI projects retained state without creating a scope, adopting
a process or turning a readback into an acceptance decision. Core has no host I/O.

The operating-system manager supplies the process grouping and kernel resource
controls. Cortex supplies the original Rust contracts, admission, correspondence,
supervision and reconciliation. The adapter is not a separate workflow engine.
It belongs in Harness because it is consumed by the existing launch boundary and
owns actual process resources; neither Storage nor CLI acquires launch authority.

## Exact request and retained observations

`ProcessSpec::containment` is optional. An `OwnedScopeSpec` identifies a fresh
unit, the exact already-delegated parent, explicit local manager routing, pinned
helper executables, requested CPU rate, charged memory, swap and task ceilings,
and the selected cancellation stages. The supervisor revalidates correspondence
at the launch boundary. No path search, ambient environment adoption or shell
expansion selects these inputs.

The original process-specification and observation encodings remain unchanged
when containment and unresolved scope helpers are absent. An explicitly extended record has an additional
field; an older reader must reject it, never drop a required control. The same
explicit supervised-fixture profile and journal are retained. No metadata-only
program acquires process authority by reopening or inspecting it.

`ScopeIdentity` retains the unit's invocation identity and cgroup device/inode
correspondence, separately from the direct process PID and start tick. Captured
scope observations retain control readbacks, membership, population and available
CPU, charged-memory and task measurements. Missing observations remain unknown;
they are not zeros. These are mechanism records, not authenticated statements or
independent acceptance. A name, path or PID by itself cannot authorize adoption.

CPU quota per period is rate capacity. `CpuMillicores` in the existing resource
projection remains milli-CPU capacity, not elapsed CPU execution. Requested rate
and memory must fit the corresponding retained reservation. Cumulative CPU usage
is recorded in microseconds. A sampled CPU-time cutoff remains cooperative and
does not satisfy a mandatory kernel-enforced cumulative CPU budget.

## Narrow qualification profiles

`ScopeOnlyInertFixture` is an audited finite provider-free bootstrap for the
owned process-set primitive. Ambient environment is cleared; the exact nonsecret
runtime-directory and user-bus routing values required by the manager remain
visible to this trusted fixture. It supplies no filesystem, network, credential
or user-bus denial. It must not run an untrusted worker.

`RootlessSyntheticCanary` requests a pinned rootless namespace helper, an explicit
bounded set of read-only files and bounded scratch. The manager routing environment
is removed before the fixture. Namespace activation requires its own observed
prerequisites and synthetic allowed/denied cases. Merely selecting this profile,
installing a helper or receiving exit zero does not qualify isolation.

Only explicitly owned synthetic files and finite fixture behavior belong in
these profiles. A source path is an explicit operator input, not permission to
search or copy a home directory, repository, credential store or previous session.
No AI executor is available through these profiles. An externally proposed prompt
cannot change their permissions or control requirements.

Helper and fixture correspondence assumes a cooperating local operator and
non-mutating system executables. A retained fixture descriptor does not magically
survive a chain of helper executions. Each actual helper and selected mount is
checked under the documented native path; no hostile same-user replacement or
universal executable-authentication guarantee follows.

## Dispatch, cancellation and uncertainty

The manager request occurs only in the fresh serialized dispatch callback. There
is no transaction spanning the encrypted store and the operating-system manager.
A lost request reply can leave an effect even if no positive receipt was stored.
Reopening the program never retries that operation or adopts a possible process.

Before supplying the prepared package, the supervisor must observe the exact owned
scope, its requested controls and actual process membership. Later authority loss,
cancel requests, deadline exhaustion or indeterminate containment trigger bounded
termination. Grace, escalation and cleanup have separate explicit intervals.
Helper execution consumes the applicable remaining deadline; starting another
query cannot refresh the overall allowance.

`OwnedScopeEmpty` requires more than parent exit: the owned scope's applicable
identity and empty population must be established, the direct child reaped and
owned streams closed. Missing or changed identity, unresolved helper processes,
failed reads or incomplete termination remain `Unknown`. Scope disappearance
alone is not fabricated into an empty-population observation. An uncertain effect
retains its resource reservation until explicitly justified reconciliation.

Captured counters survive subsequent missing reads. Original interruption and
failure observations are retained; successful later cleanup does not erase them.
No universal exactly-once effect or hard real-time cleanup guarantee is claimed.

## Inspection and qualification gates

`cortex program inspect` exposes the exact requested numeric controls separately
from retained observations, unknown population/usage and pending reconciliation.
It does not call the manager or refresh historical evidence. Private unit names,
cgroup paths, mount paths, routing details and inode identities remain outside the
CLI summary; the explicit keyed Rust record retains the corresponding identity.
The host metadata command remains read-only and activates no backend.

Qualification first establishes a safe finite owned primitive, then exercises
only boundary probes whose required containment is already demonstrated. Record
the actual executable/configuration identities, exact owned effects, positive
neighbors, negative oracles, observations, costs and cleanup. A blanket setup
failure cannot establish a successful denial.

Full operational support remains blocked by any required unqualified control,
including independent scratch byte/file/inode limits, hard cumulative CPU limits,
protected verification/recovery reserves, applicable filesystem/network denials
and whole-process-set recovery. A tmpfs byte size does not independently constrain
file count or inode count. Free host capacity is not a protected reservation.
No host-wide policy changes, privilege escalation or permission bypass may supply
a missing prerequisite. Qualification results are candidate- and host-specific
private evidence; this contract does not assert that any unperformed test passed.

The 36 capability identifiers and milestone statuses remain unchanged. Protected
integration of this extension and all live-provider operations require their own
applicable authority. The broader [host qualification gates](host-qualification.md)
and [open decisions](../decisions/open-decisions.md) continue to apply.

A scope observation uses the optional thirteenth field. A bounded unresolved-helper
list uses the optional fourteenth field, including when no scope identity was
confirmed. CLI inspection exposes these pending helpers without granting PID
adoption authority. Workload transport-buffer limits exclude the separately
bounded manager-output buffer and controller/parser allocations; they are not
a controller resident-memory ceiling.

A failed real bootstrap may leave a spawned launcher without a captured scope
incarnation. The canonical result remains Unknown even when a later query finds
no unit and the retained PIDs are absent. The current reconciliation contract
cannot turn that absence into a quiescent scope observation. Retain the original
reservation and block a replacement until applicable reconciliation is established.

The scope launcher may add its generated, non-secret `INVOCATION_ID` after the
controller clears the environment and supplies explicit manager routing values.
A trusted fixture must validate that identity against the observed owned unit;
this is not permission to inherit arbitrary manager or user environment. The
rootless namespace command clears its environment separately.

The opt-in [process recovery extension](process-recovery.md) adds a durable
identity checkpoint before input and typed inspection of existing reconciliation
guards. Legacy records retain their original meaning; a new policy cannot clear
an old missing scope identity or release its reservation.
