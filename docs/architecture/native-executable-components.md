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
policy-bound release frames. It verifies and seals the file copies it actually
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
output. EOF, partial input, cancellation, timeout, exit code, signal and unknown
cleanup remain distinct. This component consumes the raw byte/idle/wall subset of
Core ProcessLimits; it does not implement JSON frame/event or startup-record
validation and does not advertise those executor capabilities.

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

## Canonical integration blocker

The executable adapter reuses existing CreateRequested, BoundObserved,
ReleaseRequested and Report owners and the uniquely held preparation channel.
Its guard currently refuses **before CreateRequested or host creation** because
the existing DomainBindingObservation contract requires independently managed
runtime/stop deadlines and retained manager state. A direct cgroup plus a
controller timer cannot truthfully provide those readbacks. No true readback is
fabricated, Core/schema is unchanged, and no operational executable release is
available through this adapter. Resolving this source integration gap requires
implementing/qualifying those manager guarantees with existing ownership; merely
authorizing a delegated path would not resolve it.

The executable bootstrap, byte pins, raw transport, artifact capture and cgroup
components are implemented mechanisms. Their composition is not an operationally
qualified general executor. The strict two-independent-plus-dependent GPE host
scenario, canonical structured-output consumer, complete descendant cleanup and
durable recovery qualification remain blocked/incomplete. Existing fixed-nonce
and fixture authority/recovery regressions remain applicable to their own scope.

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
