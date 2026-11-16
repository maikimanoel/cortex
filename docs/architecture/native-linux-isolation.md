# Native Linux bootstrap isolation

This owner-authorized Campaign C source continuation implements a bounded native
profile under CAP-HRN-001/002, CAP-WFL-001/004 and CAP-SEC-001/002. It extends the
[recoverable domain](recoverable-domains.md) and its prepared lifecycle. It does
not complete those capabilities or provide a general untrusted-code executor.
Source implementation, controlled-fixture observation and operational host
qualification remain different claims. Candidate-specific results belong in the
private engineering handoff, not in test-source existence.

## Decision and ownership

Harness owns `native_linux`, the separate compiled bootstrap and manager adapter.
Core retains capability observations/requirements and `executor::negotiate`;
Storage retains the same durable Workflow journal, admission and recovery APIs.
No crate, persistent format, authority engine or reservation ledger is added.
The CLI gains no execution command. Existing fixed-nonce callers remain compatible.

The selected implementation uses safe APIs of the already locked Rust `rustix`
and `nix` dependencies. `nix` becomes a Linux runtime dependency with `sched`;
`rustix` enables process, thread, mount and event interfaces. No package/version
or lockfile changes are necessary. Namespace setup uses only the dedicated
single-threaded bootstrap and never a multithreaded controller pre-exec callback.
Project unsafe Rust remains forbidden. In particular, the deprecated nominally
safe `rustix::thread::unshare` interface is not used.

Alternatives include an external namespace helper, Landlock and seccomp. This
profile uses native mount/network namespaces with a minimal root because those
mechanisms enforce its exact fixed-payload filesystem/network needs. Landlock
and a syscall filter are not implemented or advertised as substitutes. A general
executable, read-only source-input view, syscall policy or provider transport is
not silently selected by this decision. DEC-004/005/006/007/008/012 remain open
outside this explicit source and fixture boundary.

## Launch contract

1. The owner supplies explicit service CPU rate, charged memory, swap, tasks and
   wall limits, legitimate delegation, pinned bootstrap/cleaner and controller.
2. `NativeIsolation::inspect` validates a canonical, owner-only empty mount point
   and explicit page-aligned scratch byte/inode limits. It installs no controls.
3. `SystemdNonceDomain::inspect_isolated` binds that policy digest into the exact
   existing configuration reference. Changed root identity/permissions/contents,
   executable bytes, bus/manager identity or limits invalidate preparation.
4. Existing durable creation dispatch precedes the manager request. The service
   is configured with CPU/memory/swap/tasks limits, control-group termination,
   null stdio, no restart and a finite runtime. Correspondent readback remains
   required; configured values are not independent behavioral qualification.
5. The dedicated bootstrap rejects unexpected descriptors and connects the
   bounded controller socket. It captures host PID, start ticks and boot before
   removing `/proc`; no PID namespace or container-PID translation is claimed.
6. Native setup completes before hello. Any failure emits at most a bounded,
   advisory `isolation-refused` frame and exits without readiness or nonce.
7. The controller verifies exact socket peer PID, manager/cgroup identity and
   policy-bound hello. It acquires a pidfd around matching start-tick observations
   and durably binds the existing observation.
8. Explicit release still calls `ProgramStore::dispatch_domain`; current authority,
   cancellation, exact binding, manager controls, native context and pidfd liveness
   are rechecked under the original lifecycle lock before one release frame.
9. Only the exact fixed public nonce is returned. Result, historical outcome,
   caller completion and closure/capacity settlement remain separate facts.

There is no executable, shell, provider, credential, environment or private input
field in a release. No general input transport is claimed. The controller socket
is a retained bootstrap-only capability; a future arbitrary payload must not
inherit it. A successful admission or configured service does not bypass setup.

## Installed child controls

The bootstrap verifies it is single threaded, accepts only null-device stdio and
its one controller socket, creates a user namespace, denies supplementary-group
changes and maps only its original UID/GID. It then creates new mount, network
and IPC namespaces. Namespace identities must change. Mount propagation becomes
recursively private before any mount operation.

A new tmpfs root is nodev/nosuid/noexec. Its only writable child is a separate
tmpfs `/scratch` with explicit byte and inode ceilings. `pivot_root`, cwd reset,
old-root detach and removal eliminate the inherited host filesystem view. The
root becomes read-only. Actual scratch capacity readback must be positive and
no greater than the request; kernel page rounding cannot enlarge the advertised
limit. No arbitrary host source directory is mounted. Cooperative operator
preservation of the pinned mount-point parent is required during preparation;
this does not protect against a malicious same-user host administrator.

Before readiness the bootstrap sets `no_new_privs`, clears ambient and bounding
capabilities, locks root/setuid securebits, clears effective/permitted/inheritable
sets and checks the resulting sets and privilege flag. New network namespaces
provide a distinct interface/abstract-socket domain; inherited network sockets
are refused, except for the intentionally retained fixed controller channel.
No socket from the host credential/provider environment is handed to a payload.

`NativeReadiness` projects only filesystem, network and this profile's scratch
`DiskBytes` into existing Core `ControlledFixture` capability observations. It
cannot satisfy a `HostQualificationReport` requirement. Absent controls remain
held by Core; metadata-only `planned_support` never reports enforcement. Boot
and policy changes invalidate affected observations. A historical ready sample
is not continued child liveness; the unique prepared channel, pidfd and manager
checks remain separate. General transport preparation still requires its own
standard-input/structured-output claims and exact package correspondence.

## Lifecycle, resource and recovery limits

The existing framing bounds each control message at 4096 bytes and applies the
original absolute deadline to partial reads/writes. Null stdio prevents a child
output buffer from becoming an unbounded allocation. The new profile does not
implement arbitrary stdout/stderr forwarding or general commands.

CPU rate, charged memory, swap and task limits reuse existing systemd/cgroup
configuration and readback. Controller presence, readable files or a writable
shared desktop scope do not establish permission to create a strict attempt.
No shared/global cgroup is configured by this extension. Cumulative CPU ceilings,
protected host reserves and a fully qualified whole-descendant execution envelope
are not established by scratch or namespace success.

Existing cancellation commits intent, targets the exact retained domain and
preserves incomplete closure. A pidfd is a stable direct-process reference;
direct exit is not cgroup quiescence. Name-based manager stop retains the documented
cooperating-operator race. Bounded recursive cgroup, helper, stream, snapshot and
final-use observations still govern closure. Missing proof retains Unknown and
reservations, without automatic refund, settlement, retry or relaunch. No database
transaction is claimed atomic with physical process execution. Replay is read-only.

## Provider mediation

`provider_mediation` validates exact already-admitted dispatch correspondence and
existing `check_supervised_current` authority against the supplied canonical
kernel. Only a bounded public-nonce in-memory fixture route exists. Live routes
always refuse; an adapter cannot select a fallback provider. The borrowed child
projection contains only a disposition and permitted public nonce, excluding
host paths, grants, source references and credentials. Unknown/cancelled outcomes
carry no successful reply and settle no durable effect. Supplied/replayed kernel
authenticity is not authenticated operational provider authority. No network,
credential file or real provider is accessed. DEC-012 remains unresolved for live
provider principals and route authorization.

## Verification mechanisms and performance scope

Pure tests challenge policy/root changes, unavailable capability requirements,
evidence-class mismatch, pidfd birth mismatch, forbidden controller setup,
canonical Unknown retention and no replay after refused readiness or changed
policy. Provider tests cover exact dispatch, scope, cancellation, uncertainty,
revocation, expiry, clock regression, route refusal and redacted projections.

The purpose-built `isolated-probe-v1` invocation additionally uses only private
canaries and an operator-owned loopback listener. It requires a 64 KiB scratch
fixture and 16 inodes. It checks root EROFS, outside-canary/symlink/traversal denial,
actual scratch ENOSPC and loopback denial after verifying a reachable outside
neighbor. Its fixed public nonce still requires an exact release. Recognized
namespace prerequisite denials are reported as blocked host observations; an
unexpected later setup failure fails the test. A blocked branch is never an
isolation pass. The manager never selects the probe mode.

The explicitly ignored `native_fixture_sequential_parallel_observation` is a
four-child, at-most-two-concurrent overhead harness. It holds workload, output
oracle, controls and setup/cleanup boundary constant. It reports raw absolute
wall times and valid completions. It supplies no statistical, CPU, memory,
productivity or GPE strategy speedup claim. A representative governed comparison
still needs an approved owned resource scope, actual canonical operations,
equivalent resources, complete attempt populations and DEC-008 decisions.

Run focused source checks with the pinned toolchain and conservative job/thread
limits, then the complete [repository verification](../../README.md#verify).
Actual host tests require their explicitly authorized private fixture envelope;
never repair missing permissions with sudo, host settings or permissive fallback.
