# Trusted native executable components

This owner-authorized source continuation extends CAP-HRN-001/002,
CAP-WFL-001/004 and CAP-SEC-001/002. It preserves the restricted
[fixed-nonce profile](native-linux-isolation.md), canonical domain journal,
500 ms admission budget, original Unknown classification and reservation authority.
No capability or milestone status is promoted by this source document.

## Implemented components

Harness owns an explicit executable policy referencing the existing admitted
PreparedInput and InstructionPackage. ExecutionPlan pins bounded approved regular
file bytes into sealed copies, commits the immutable policy to a configuration
reference, and rechecks the existing domain/package authority. A registered member's
input is inspected using that member kernel; release checks use the coordinator's
registered-member view. Arguments, public environment keys, runtime files,
read-only source inputs, scratch limits and expected outputs are explicit.
These remain trusted-operator assertions, not authenticated production permission.

The separate single-threaded executable bootstrap accepts bounded setup and
policy-bound release frames. Before installing namespaces it independently checks
its actual cgroup membership, directory/boot identity, domain type, exact configured
limits and exclusive bootstrap membership. It closes these inspection descriptors
before confinement. This remains a trusted-controller protocol, not independent
authentication of canonical admission; the guarded controller owns that authority. It verifies and seals the file copies it actually
mounts. Individual immutable executable/runtime mounts are executable and read-only;
inputs are read-only and non-executable. The minimal root is read-only and scratch
is bounded tmpfs with noexec. No host root, home, credential, network socket or
control descriptor is delivered to the executed program. There is no shell or
unrestricted fallback. Dynamic executables require every needed interpreter/runtime
file explicitly. Native user/mount/network/IPC controls and privilege reduction
reuse Campaign C; PID namespaces and seccomp are not implemented by this profile.
Same-user replacement of the controller-selected bootstrap path remains a
cooperating-host assumption.

Raw standard-stream collection feeds bounded input while draining stdout and
stderr together, enforcing independent and aggregate capture bounds. Overflow
returns the original interruption with a finite prefix, never successful complete
output. Explicit ExecutionPlan::with_json_lines selects the existing Core
JSON-lines contract: each nonempty stdout line is a JSON value, the first complete
valid value is startup, and EOF may finish a final non-newline value. There is no
payload request/version/kind/terminal schema in this contract. Repeated object
tags therefore carry no special authority or completion meaning. Binary stdout
continues to use the raw collector unchanged.

The selected consumer bounds frame bytes (at most 1 MiB), record count and retained
capture/frame buffers. Each byte is scanned once and each completed frame is
syntax-checked once. Partial UTF-8 and tokens survive read boundaries; malformed
encoding, empty frames, partial EOF, startup timeout and overflow retain their
original interruption. The original preparation clock supplies startup/wall
deadlines; stderr progress does not establish startup. A read-only observation
binds the summary to existing full intent, binding and release identities. These
caller-correlated observations are not authentication or accepted artifacts.
Process exit, stream EOF, cancellation and whole-domain quiescence remain separate.

Artifact capture consumes only declared normal relative paths through an owned
directory descriptor, with openat2 beneath/no-symlink/no-mount-crossing resolution,
regular-file and single-link checks, aggregate byte/count bounds and actual hashes.
The domain adapter acquires private scratch through the owned live bootstrap's
proc-root view before release, retains its descriptor, and captures only after
recursive group emptiness. This cross-namespace acquisition has not been qualified
on a delegated host and fails closed if permission is absent. Retained scratch
descriptors continue to carry resource obligations until disposed.

## Delegation, cancellation and recovery

The cgroup adapter requires an explicit operator-selected authorization containing
canonical root, device, inode and boot. Inspection requires cgroup v2, domain type
and already enabled CPU, memory and pids controllers. No ancestor controller or
systemd setting is changed. A new exclusive child is held by directory descriptor;
CPU, memory, swap and pids writes require exact readback before attachment. The
trusted bootstrap is blocked until the exclusive unreaped Child is attached.
Controller availability, installation readback and measured enforcement are
separate facts.

The cgroup.kill operation addresses only the held verified group, after the
canonical cancellation owner has recorded intent. Recursive cgroup.events
population is observed separately from direct-child exit. Read-only recovery
reopens only the exact recorded directory incarnation, classifying present,
gone, replaced and unverifiable observations without release-channel recreation,
execution replay or reservation settlement. Controller interruption and incomplete
closure retain uncertainty. Direct-child transport cancellation is not a
whole-domain cancellation proof.

Normative mechanism reference: [Linux cgroup v2 documentation](https://docs.kernel.org/admin-guide/cgroup-v2.html).
Controller presence or writable-looking desktop metadata is not authorization.
This development host has no owner-authorized delegated scope; actual cgroup
mutation, resource-denial and descendant-termination qualification are blocked.

## Independent manager composition

The original direct-cgroup executable adapter retains its fail-closed
ManagerGuarantees guard before CreateRequested. A cgroup plus a controller timer
still cannot provide independent manager lifetime or retained-state guarantees.

The separate ManagedExecutionDomain composes the existing typed systemd user-manager
adapter with the same canonical CreateRequested, BoundObserved, ReleaseRequested
and Report engines and unique preparation channel. It binds explicit executable
policy to manager/configuration identity, uses an explicit bounded standard-stream descriptor handoff over the held
bootstrap channel, and validates actual service type, restart/kill/privilege controls,
runtime and stop readbacks. The independently installed runtime is an explicit
caller-selected sub-budget: actual manager activation plus that runtime must not
extend past the original monotonic deadline. The separate 500 ms manager stop
grace must fit the explicit cleanup budget. Runtime expiry is not cleanup proof.

The selected writable scratch quota must fit the same domain's still-held
canonical `DiskBytes` reservation. The managed adapter checks the committed
intent and quota before creation and again in the locked create and release
callbacks. The quota is a configured byte ceiling. Actual consumption and
terminal closure require separate evidence.

RemainAfterExit retains an active exited service for readback within that manager's
actual lifetime. It is not permanent retention: manager restart, garbage collection,
lost bus ownership, absent name and changed invocation are distinct read-only
recovery observations. Missing properties or ambiguous submission refuse rather
than substituting requested configuration. Consumed pipe submission cannot be
regenerated after a lost creation reply. Completion status comes from the manager;
the controller never adopts a numeric PID or reaps a manager-owned child.

An explicitly authorized delegated resource root remains mandatory before any
manager connection through this composition. The exact manager-created cgroup is
pinned beneath that root by descriptor and boot/device/inode identity before
payload release. Canonical Cancel precedes termination through that held group's
cgroup.kill; the strict profile sends no name-based StopUnit request. Actual
recursive population, direct exit, stream closure, scratch artifacts and historical
effects remain separate. Neither stop acknowledgment nor empty population creates
a ClosureReceipt or refunds reservations. Legacy fixed-nonce manager behavior and
retained bytes keep their existing meanings and documented cooperation assumptions.

These source mechanisms do not qualify a host. The dedicated delegated-host,
controller-interruption/restart, resource-denial, complete descendant cleanup and
strict two-independent-plus-dependent GPE graph remain BLOCKED pending separate
authority and candidate-bound actual-host evidence. Source fake-manager scenarios
are SOURCE_SIMULATION, not strict host or operational acceptance.

## Verification and performance

Pure tests challenge missing package/authority, policy mutation, mount geometry,
secret-like environment keys, interrupted limit writes, inconsistent readback,
seals, output bounds, artifact traversal/symlinks/hard links and digest mismatch.
Two finite prebuilt Rust fixture binaries independently exercise exact copy and
length/digest behaviors through the raw transport. Their explicitly selected
ignored tests are counted trusted component launches, not strict-domain success.
The full workspace does not silently start these new fixtures.

DEC-004/005/006/007/008/012 retain their current decision boundaries. Existing
provider mediation still refuses live routes and carries no credentials. No
provider, hostile workload, production resource defaults or authenticated
principal is introduced. P1 component timings are observations only; P2 governed
comparison and P3 accepted software-development productivity are not established.
No speedup or operational-readiness claim follows from compile/test success.

Manager readback in the serialized release callback preserves the existing
authority cut. The journal's 500 ms admission budget is checked before that
external-effect callback; it is not a measured end-to-end manager response bound.
The callback can occupy the journal lock until its original deadline, affecting
neighbor admission latency. Stream pumping and parsing occur after that lock is
released. Neither latency nor service responsiveness has been host-qualified.
On capture overflow, the retained raw prefix can include bytes not yet parsed;
record counts describe parsed observations rather than every raw-prefix record.

The executable profile leaves manager stdio null. After authenticating the actual
bootstrap peer, the controller transfers exactly three owned FIFO endpoints using
SCM_RIGHTS on the existing control channel. The single-threaded bootstrap validates
count, roles, distinct identities and close-on-exec, intentionally duplicates them
only onto stdin/stdout/stderr, closes ancillary copies and acknowledges installation.
Canonical binding subsequently checks the actual bootstrap pipe identities.
The manager never receives workload-pipe writers: retained manager state therefore
does not itself hold output EOF open. A missing/truncated/ambiguous handoff refuses
before readiness and is never resent. Explicit stdio-v1 selection preserves the
legacy setup profile; it is not a new workflow command or journal format.

Partial descriptor installation is a preparation failure: no readiness or payload
release follows it. Ancillary descriptors are RAII-owned; only the dedicated
single-threaded bootstrap intentionally replaces its standard descriptors.
In-process source tests never replace the controller's standard streams.

The canonical source simulation uses existing independent B/E tasks and dependent
F. Parsed/process reports cannot release BF/EF dependencies; explicit synthetic
artifact/evidence/review acceptance is required. Fresh F authority after both
accepted edges still does not permit a new reservation when B/E final cumulative
consumption is unresolved: Core returns Debt(CpuExecutionMillis). Reported
execution success and unknown consumption are separate dimensions. The source
scenario therefore observes two reports and one debt-held dependent, preserving
occupancy and performing no settlement. A completed strict three-task graph has
not been demonstrated. Read-only direct-cgroup recovery treats a missing lookup
path as unverifiable, since a renamed original group may still exist.

Standard-stream installation refuses multithreaded callers before receiving
descriptors or replacing stdin/stdout/stderr. This finite snapshot relies on the
dedicated cooperating bootstrap not creating threads; it does not exclude an
adversarial thread-creation race. Partial installation remains a preparation
failure with no payload release.

The exact cgroup.kill descriptor must be openable for writing during preparation,
before readiness or process attachment. No termination value is written then.
Canonical cancellation writes only that retained descriptor after current exact
group checks; a later pathname replacement cannot redirect it. Read-only recovery
does not acquire termination authority. Descriptor availability is a prerequisite,
not proof that an eventual kernel write, quiescence or cleanup will succeed.

## CPU accounting observations

The managed executable can publish a partial tree observation after durable
cancellation through the existing domain journal. It brackets the manager's
tree observation with bounded reads of the held cgroup's CPU counter and matches
the sampled boot, invocation, device, and inode. The capture uses the original
deadline. Publication retains the canonical store's completion classification,
including indeterminate admission. Inspection of that event does not repeat
cancellation or signal a domain.

The selected counter is cumulative CPU microseconds from the cgroup v2
`cpu.stat` field `usage_usec`. Missing, malformed, duplicated, truncated,
out-of-range or contradictory reads supply no numeric evidence. An empty tree
and stable counter do not cover external helpers, prove absence of re-entry, or
exclude snapshot resurrection.

A read-only check can compare an independently proposed closure receipt with the
latest retained tree sample and reject an amount below the sample rounded up to
whole CPU execution milliseconds. It verifies exact recorded correspondence and
screens the proposed debit. It cannot establish the independence of an observer,
complete accounting coverage, or the finality of the proposed measurement. Only
the existing canonical closure and settlement operations can account final use.
No measurement is written from a partial native sample.

A source simulation supplies independent synthetic closure assertions for B and
E through those existing operations. F remains held when either final debit is
missing, even when both artifact edges are accepted. After both final debits are
admitted, fresh F reservation and its simulated lifecycle proceed. Native
final-counter production and actual host execution remain unqualified.

## Managed end observation

The managed adapter brackets its bounded end-of-transport tree read with CPU
usage reads through the held group. The readback must match the retained intent,
binding, and cgroup incarnation within the original deadline. It separately
retains direct completion, transport completeness, recursive population and
the CPU observation. Unavailable readings and contradictory counters remain
non-numeric. Even a stable empty readback with a numeric value supplies only a
cumulative lower bound in microseconds.

This in-memory observation is bound to one release request. Reading it does not
refresh live state or authorize a write. It adds no retained journal event,
snapshot-exclusion proof, independent whole-domain witness, final debit, or
settlement. The existing post-cancel observer retains its prior partial event
encoding and cancellation prerequisite.

`observe_managed_executable_closure_tree_with_attempt` retains the complete
captured operation and store identity alongside a publication failure. The
caller can reconcile against the same store and initial contract without
capturing again. A matching observation ID with different observed facts is
a conflict.

The managed adapter also retains an in-memory CPU history for the exact store,
domain, invocation, cgroup and monotonic interval. It samples the held directory
while bootstrap awaits setup, at normal transport end and after durable
cancellation when the group remains observable. Bootstrap may already have
consumed CPU at the first cut. Missing readings, wrong epochs and regressions
preserve uncertainty even if a later number looks valid.

A separate whole-domain observer report can bind one empty-tree counter
bracket to an initially unused, fully attributed and continuously fenced
lifetime. The local admission path requires the configured observer role on
the protected Unix channel and checks the exact operation, store, boot and
time namespace. The bracket completion is sampled after the final clock
namespace check. Managed tree publication and CPU disposition use the
original absolute cutoff through reconstruction and canonical admission.
The protected observer profile and local clock namespace are rechecked at
the pre-commit cut for a new CPU disposition. A late acknowledgment leaves
the potential write indeterminate. The existing journal decides whether
to admit the collector's rounded whole CPU millisecond debit in `Closed`
or `Settle`. CPU-only disposition leaves occupancy held. A controller
restart preserves admitted receipts and cannot reconstruct unretained proof.
Physical observer coverage, actual kernel-counter retention and P1 remain
unqualified.
