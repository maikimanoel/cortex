# Recoverable execution domains and resource disposition

This bounded extension advances CAP-CORE-003, CAP-WFL-001/004, CAP-EVD-003,
CAP-HRN-001/002, CAP-MEM-001, CAP-SEC-002 and CAP-CLI-004. It changes no
capability identifier, owner or complete milestone status. Its initial backend
is a provider-free Linux development profile with a fixed synthetic payload.
Source verification, controlled execution, actual host qualification and
operational authorization are separate acceptance boundaries.

## Five independent facts

| Fact | Meaning | What it cannot establish |
| --- | --- | --- |
| Historical outcome | Retained observations about what an operation did | Present liveness or resource availability |
| Present effect capability | Whether the exact execution domain can still act within the covered scope | Historical success, noncreation or absence of persistent effects |
| Cumulative consumption | Measured use, conservative retained debit and unresolved debt | Unknown use is never measured zero or bounded by an unenforced reservation |
| Reusable capacity | Occupancy held or settled for each dimension | Permission, dependency acceptance, a fresh cumulative budget or safe retry |
| Administrative disposition | An authorized pause or archive | Technical closure, cleanup, refund or artifact acceptance |

A valid new closure observation can establish that a domain can no longer act
without rewriting its historical Unknown. Closing a process domain cannot by
itself clear quarantined scratch, unknown remote outcomes or cumulative debt.
Each covered reusable dimension settles at most once. Original observations,
reservation identity and the basis for disposition remain inspectable.

CPU capacity uses `CpuMillicores`: 1000 is one CPU equivalent. Cumulative
CPU execution has its own dimension. Charged memory is not virtual address space;
task counts include threads where supported. Scratch bytes, objects and inodes
are distinct. A sampled CPU cutoff is not a hard lifetime CPU cap, a kill request
is not quiescence, and free memory or disk is not a protected reserve.

## One authoritative local admission boundary

Core owns domain identities, resource meanings and valid transitions. Evidence
composes registered program contracts and their current authority/dependency
checks with one shared pool projection. Storage serializes those inputs in the
existing encrypted Workflow journal. Harness observes the manager and performs
only authorized backend effects. CLI displays retained facts and missing proof.

The explicit new profile is a single-operator pool-coordinator collection. Member
programs share the same transactional admission boundary; separate per-program
capacity counters cannot each allocate the full shared pool. Its physical pool
and incarnation are immutable. Owner-supplied physical correspondence remains a
trust assumption: two labels or two databases do not prove disjoint hardware.
The supported operator must select one authoritative collection for an overlapping
allocation. This is not a distributed allocator or protection against a malicious
operator creating competing stores or restoring old snapshots.

New work requires current authority, accepted dependencies, exact candidate and
package correspondence, available reusable capacity and every required cumulative
budget. Settled capacity cannot bypass unresolved mandatory debt. A distinct new
task is admitted through the ordinary guards; changing an attempt label cannot
make a lost original operation safe to retry.

Each member's aggregate policy also limits its share of the same pool records;
this does not introduce a separate allocation ledger. Version 4 maps cumulative
the active `ElapsedMillis` dimension to elapsed milliseconds and `OutputBytes`
to output bytes. Historical versioned tokens remain unchanged. Unsupported
cumulative mappings are refused, including treating milli-CPU `CpuMillicores` as
elapsed CPU execution time.

## Outer boundary before payload release

The [identity-before-input checkpoint](process-recovery.md) remains useful for
its accepted fixture profile. An executable may act before reading stdin, so
that checkpoint does not contain arbitrary payload execution.

The new path records intended domain, operation, configuration, authority and
reservation before requesting creation. A fixed trusted Rust bootstrap runs in
the manager-created outer domain and waits for a one-use release. The controller
must establish and durably bind actual domain incarnation, membership and control
observations before releasing the supported payload. Fresh cancellation,
authority and deadline checks remain necessary across every persistence boundary.
The payload receives no journal key, manager connection or controller authority.

A lost creation reply remains uncertain. Recovery reads retained intent and
manager observations; it does not repeat creation. A unit name is a lookup key,
not an incarnation. Host boot identity, manager ownership, invocation identity,
cgroup identity and the relevant launch generation participate in correspondence.
Changed epochs, missing units and manager garbage collection are not closure or
noncreation proofs. Launch/setup helpers belong to the recorded trusted controller
boundary or the managed domain; a direct child's exit cannot hide their uncertainty.

After controller interruption, observation may reconstruct a still-matching domain
or an applicable closure. It cannot adopt an unrelated PID, reset a deadline,
replay payload release or infer success from present absence. A crash after
physical closure but before publication requires a fresh applicable closure
observation. Exact journal admission makes repeated settlement idempotent;
it does not promise universal exactly-once external execution.

## Initial Linux backend and alternatives

The selected implementation uses the systemd user manager's structured D-Bus
interface through safe Rust `zbus` calls on an explicitly selected local Unix
connection. Rust constructs typed arguments and decodes typed replies; no shell
command string or presentation-text parser supplies authority. Connection,
authentication and calls have separate absolute-deadline checks. A timed-out call
does not establish that the manager request failed to take effect.

Direct D-Bus adds dependencies but avoids out-of-domain client helper processes
whose lifetime could become uncertain after controller failure. Bounded
`busctl` subprocesses were considered as a smaller dependency alternative; their
separate cleanup boundary is a material recovery cost. Safe `rustix` operations
support nonblocking local transport, and SHA-256 binds executable bytes and
configuration across reconstruction. Cortex introduces no unsafe Rust or custom
cryptography. The manager is OS infrastructure; neither it nor the IPC library
becomes Cortex's workflow, scheduler or permission authority.

The service executes a pinned environment-cleaning OS wrapper inside its managed
domain before the fixed bootstrap. The wrapper supplies an empty environment;
the bootstrap rejects unexpected descriptors. Only the exact descriptor owned
by enumeration is exempt; another handle with the same procfs target is refused.
Startup and protocol share one absolute deadline. Fixed phase/error diagnostics
omit paths, descriptor targets, environment and payload. The finite development
fixture uses child-side POSIX spawn file actions to establish its descriptor set;
it does not alter the parent/Cargo descriptor table or qualify host isolation.
Manager-library buffering is a
trusted control-plane limitation, separate from the bounded payload transport.
No hard whole-controller memory or real-time guarantee follows from call deadlines.

The first payload is a fixed public-synthetic nonce operation. Arbitrary programs,
model providers and user sessions are unsupported. Controlled fake-manager tests
and finite trusted bootstrap fixtures validate protocol ordering and recovery;
they do not qualify manager delegation, namespaces, cgroup enforcement or a
hostile workload boundary.

Actual activation requires exact tool/configuration identities, existing effective
delegation, applicable grants, available budgets, independently accepted source
and procedure, and a test safety envelope that remains bounded if the tested
control fails. No new delegation or shared host policy is installed. Rootless
filesystem/network isolation and all mandatory resource controls require their
own observations. An isolated VM is a provisioning alternative, not an automatic
solution or an authorized operation.

The threat boundary assumes a cooperating single operator and trusted kernel,
manager, bootstrap, controller and selected client executable. It does not defend
against a malicious same-user controller, host administrator, compromised kernel,
snapshot rollback or unrelated host workloads consuming unreserved resources.
Unsupported threats and controls remain visible instead of being inferred from a
service, namespace or container label.

## Persistence, privacy and compatibility

### Prepared native lifecycle

The fixed native consumer separates preparation from release. Preparation commits
creation intent once, acquires the bootstrap's real channel and peer identity,
and publishes a `BoundObserved` record before returning an owned prepared handle.
That handle retains the channel and original deadline; it is neither clonable nor
reconstructible from a journal. Returning from preparation does not publish a
release request or send a nonce. The existing nonce executor composes preparation
and explicit release rather than maintaining a second execution path.

Explicit release consumes the handle and rechecks current grants, cancellation,
generation, configuration, live incarnation and retained observations at the
serialized publication/effect boundary. The deadline is not restarted by the
handoff. Dropping the handle closes its controller-owned transport only: it
does not stop the managed domain, establish closure, refund reservations or retry
creation. Reconstruction has no handle to adopt and remains observation-only.

The binding record retains main-process birth identity, bounded direct cgroup
members, exact domain/configuration correspondence and resource/control readbacks.
These are non-atomic observations of direct membership, not a whole-subtree
fence or denial proof. Intended names remain distinct from actual observed
identities. A partial or mismatched binding cannot permit release. CLI inspection
reports the presence and coverage of these retained facts without contacting the
manager or exposing private process, path or mechanism identities.

`domain-observation-v1` is an explicit additional command encoding in the existing
domain-coordinator journal. Legacy `Bound` records and their bytes retain their
meaning; they do not acquire observations retrospectively. Readers without the
new command tag refuse it as unsupported. No historical store migration, new
effect ledger, allocation waiver or change to cumulative-debt guards is supplied.
The added public enum variant and record field are pre-alpha Rust API additions:
downstream exhaustive matches and record literals must be updated when rebuilding.
The compatibility claim concerns retained wire bytes and meanings, not an unchanged
binary ABI or a general source-stability promise.

### Closure and subsequent admission

The native closure-inspection path retains a bounded, versioned post-cancellation
observation in the same journal. It requires the actual binding and durable Cancel
before capture/publication, and matches the store, scope, configuration, manager,
boot, invocation, cgroup incarnation and process birth observations. Duplicate,
out-of-order or mismatched samples are refused. Replay only reconstructs records;
it cannot invoke the observer or repeat a stop request.

These samples preserve their observation tick, direct-member coverage, provenance,
population and optional sampled usage. They do not become final consumption,
closure receipts or settlement. Even an empty direct-member list with zero sampled
CPU leaves explicit gaps for whole-domain, helper, stream, snapshot and final-use
proof. The read-only CLI reports those gaps separately from any existing
caller-asserted closure receipt; inspection does not contact the manager or refresh
old evidence. Partial native capture never releases occupancy or clears debt.

An actual closure producer still needs an accepted observer outside the interrupted
controller, complete membership/helper/stream coverage, exact immutable identities,
applicable restart/snapshot evidence and final dimension-specific measurements.
The current adapter supplies no snapshot-exclusion guarantee. Missing units,
garbage collection, changed epochs or unavailable counters remain missing proof.
No boolean copied from a fixture or configuration readback supplies that guarantee.

New closure observations use a distinct command/event encoding; existing domain
and binding encodings retain their original bytes and meaning. Readers lacking
the extension refuse it. There is no historical migration, changed receipt trust,
debt exception or broader source/API compatibility promise. The observation count
is bounded per domain; reaching the bound refuses another sample rather than
overwriting evidence, dropping uncertainty or granting more execution budget.

Preparation is not closure. A normal cancellation must be durable before its
exact stop request, but the acknowledgement establishes neither quiescence nor
resource disposition. The current native observer still cannot prove snapshot
exclusion or supply a complete `ClosureReceipt`. Parent exit, a missing unit,
manager garbage collection and a lost reply retain uncertainty. Inspection must
not copy proof flags from a fixture into a real observation.

The existing closure guard requires exact domain/incarnation/configuration/scope,
whole-member and helper coverage, independently supported quiescence, prevention
of restart and snapshot resurrection, and dimension-specific resource evidence.
A closure can preserve historical Unknown while settling only covered reusable
capacity. Missing final consumption remains debt and blocks further admission.
Even sufficient closure is not permission to retry the old operation or accept
its artifacts. A distinct next attempt needs every current authority, dependency,
pool, capacity and cumulative-budget guard. Administrative pause/archive changes
none of those facts.

Controlled fixtures exercise preparation, explicit release/cancellation, durable
reconstruction and the existing positive/negative settlement rules. A real normal
cycle and a second actual admission need separately accepted physical evidence.
That normal control precedes deliberate controller interruption in a future
qualification procedure; normal-path closure would not prove post-crash closure.
No host qualification, policy exception or operational continuation is inferred
from this source extension.

New records use an explicitly selected versioned profile. Legacy bytes and meanings
remain unchanged; older readers reject unsupported records. There is no automatic
migration, retrospective checkpoint or rewrite of historical observations.
Reconstruction invokes no manager, payload, provider or integration operation.

The existing SQLCipher synchronization and single-local-operator assumptions still
apply. No transaction spans storage and the manager. Exact operation identities,
immutable observations and uncertain acknowledgments describe that gap rather than
claiming it cannot occur. Tests must include both sides of every publication cut.

Diagnostics retain bounded phase, typed manager error category, configuration
correspondence and redacted causes. They retain neither arbitrary private manager
output nor raw model sessions. Explicit privacy, retention and disclosure policy
remains necessary for operational data. Inspection does not contact a manager,
refresh liveness, signal a process or confer authority.

## Acceptance and downstream gates

Required source cases include lost creation reply without duplicate execution;
valid closure with historical Unknown; exact-once per-dimension settlement;
positive admission of independently authorized work; refusal for unresolved
cumulative debt; retained helpers; wrong boot/domain/incarnation; cancellation and
revocation at publication/release cuts; crash after closure; manager restart/GC;
administrative pause without refunds; cross-program contention; quarantined disk;
legacy/new replay; and read-only causal CLI output.

These mechanisms need candidate-scoped executed evidence and independent technical
review. Source hashes, test counts and green CI cannot alone qualify a host or
establish operational readiness. The initial bounded implementation does not
change the treatment of an older unresolved effect: absent applicable proof and
authority, its historical journal, debt and reservations remain held.

The construction sequence is recovery/resource semantics, then the recoverable
outer backend, then qualification of that exact profile. Compatible interfaces
may support later provider transport, substantial engineering tasks and GPE
programs, but those remain separately authorized downstream stages.
