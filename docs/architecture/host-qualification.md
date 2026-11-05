# Provider-free host qualification and Harness hardening

This bounded GPE-B extension belongs to CAP-CORE-003, CAP-WFL-001/004,
CAP-HRN-001/002, CAP-CTX-002, CAP-SEC-001/002 and CAP-CLI-004. It reuses the
[supervised-fixture contract](supervised-fixtures.md), its single encrypted
Workflow journal and existing authority guards. It adds no capability identifier,
model, orchestration framework or competing permission service.

The owner authorizes source implementation, read-only host inspection, independent
review and bounded provider-free qualification with owned synthetic fixtures.
Each effectful probe still requires its actual containment prerequisites. Missing
delegation or mandatory controls holds that probe while independent Rust work
continues. This authorization does not permit host-wide policy changes, privilege
escalation, permission bypass, live providers or protected integration.

## Acceptance and ownership

An operator can inspect why the current host has not established an execution
boundary without activating one. A trusted fixture can select distinct transport
limits in its durable process specification; an exceeded limit produces its
original typed interruption, never a successful engineering assessment. Replay
retains the exact limits and observations and never launches another process.

Core owns the optional transport contract and structural outcome guards. Evidence
retains the same canonical inputs and observations. Storage encodes those inputs
without a second journal. Harness owns metadata acquisition and actual trusted
fixture I/O enforcement. CLI depends on Harness for read-only presentation; the
dependency remains acyclic. No infrastructure dependency enters Core.

The host observer belongs in Harness because its interface describes actual host
prerequisites consumed by execution engineering. Implementing it in CLI would
duplicate acquisition for other consumers. A new backend crate or generic launcher
is unnecessary while no usable isolated execution profile has been established.

## Read-only host preflight

`cortex_harness::host_preflight::inspect_host` inspects fixed Linux proc/sysfs
metadata and the fixed bubblewrap executable location. It performs no subprocess,
network, policy write, namespace activation or credential/session lookup. Reads
have finite file, aggregate and operation bounds and a cooperative deadline;
blocking kernel I/O is not promised preemptible. Missing, denied, malformed,
oversized, changed and unsupported metadata remain distinct redacted causes.

The observer reports configured CPU rate, memory, swap and process/thread ceilings,
controller availability, cgroup membership consistency, namespace configuration,
and executable file metadata. It does not infer executable version or byte identity
from file metadata. Before/after correspondence is not an atomic snapshot.

These are observations of the observer's potentially shared scope. File writability,
a finite shared limit, a controller name or a nonzero namespace switch does not
establish an owned delegated attempt subtree or successful denial behavior. Even a
fully populated metadata fixture leaves delegation unestablished and behavioral
qualification unperformed. The report is not a grant and cannot activate dispatch.

```text
cortex host inspect
cortex host inspect --json
```

JSON schema `cortex.host-preflight.v1` separates `OBSERVED_METADATA`/`UNAVAILABLE`
fields from `BLOCKED` qualification and `NOT_RUN` behavioral tests. It reports
causal gaps for filesystem, network, CPU rate and time, memory/swap, processes,
scratch, verification reserves, cancellation and credentials. It omits cgroup
names, raw contents, UID and device/inode identities. The Rust report retains
explicit metadata for programmatic inspection, without authenticity guarantees.

Exit 0 means inspection produced a report, including unavailable metadata and
blocked qualification. Exit 2 rejects unsupported arguments without echoing them;
exit 1 reports output failure. The command offers no activation, recovery, grant,
qualification-assessment or execution subcommand.

## Exact optional transport limits

`ProcessLimits::transport` selects explicit `TransportLimits`, rather than universal
runtime defaults. The optional contract is included in `ProcessSpec` equality,
prepared-fixture pinning, journal identity and current dispatch checks. Changing a
limit cannot silently reuse the original immutable intent.

- Startup is measured from fresh dispatch admission to the first complete valid
  stdout JSON record. A child that exits without that record cannot satisfy the
  selected transport contract.
- Idle time measures the interval without actual input/output byte progress.
  Startup and idle deadlines remain subordinate to the execution wall deadline.
  Their initial clock starts before the durable dispatch commit, so journal and
  initial observation latency consume the same allowance as process startup.
- Captured stdout and stderr each have their own ceiling, in addition to the
  existing combined-output ceiling. The record ceiling counts valid stdout JSON
  records, including a final record without a newline at EOF.
- The transfer-buffer bound covers the supervisor's input allocation capacity,
  frame allocation capacity and read scratch. It excludes reconstructed canonical
  state, executable pins, allocator metadata, parser stack/nesting scratch and kernel queues. It
  is a narrower bound than total transient runtime memory, not an RSS quota.

Deadline checks remain cooperative, including around canonical state inspection.
An interruption retains its cause even if cleanup later succeeds. Observed counts
can exceed a ceiling by the bounded detection observation; actual consumption is
not rewritten to the configured allowance. These controls do not implement staged
process-tree termination, hard CPU/memory quotas or filesystem/network isolation.

The original nine-field limit encoding remains unchanged when the extension is
absent. Explicit extended limits use a tenth structured field. Older readers
reject that shape rather than ignore mandatory limits. The same format 3 and
effect mode remain trusted-fixture-only; formats 1/2 do not gain process authority.
There is no automatic migration or claim of general stable Rust API compatibility.
Rust struct-literal callers explicitly add `transport: None` to retain the old
profile, or supply complete extended limits. Historical JSON parsing behavior
remains selected for the old profile; the extended profile validates borrowed raw
JSON syntax without constructing a semantic value tree. Neither parser establishes
the truth of a worker's output.

## Qualification procedure and limits

Actual qualification needs a separately established owned backend, exact binaries,
configuration, operation identity, current grant and enforceable reservations before
launch. Only synthetic canaries, credential substitutes and owned endpoints may be
used. Do not use genuine private files, credentials or sessions as denial targets.

Probes cover allowed neighbors and denied boundaries for paths, symlinks, mounts,
working directories, inherited handles, parent/descendant network, resource ceilings,
timeout/output exhaustion, revocation, cancellation and whole-process-set cleanup.
Controller-loss and journal crash cuts retain uncertainty; neither parent exit nor
closed output streams proves a hostile descendant set is gone. Unknown effects
retain reservations and cannot be blindly retried.

Fork, resource-exhaustion, network and filesystem stress probes wait for their
required containment. Existing finite audited development fixtures test their
declared cooperative contract; they do not qualify hostile containment. Configured
budgets, measured consumption, enforced limits and protected reserves are separate.
An unavailable control is a blocker, not an invitation to weaken the oracle or use
another credential. Proposed alternate hosts require owner selection and their own
qualification; a VM or namespace label alone is insufficient.

Candidate-scoped evidence records PASS, FAIL, BLOCKED, NOT_RUN and UNKNOWN using
the existing Evidence semantics, with immutable source/configuration/oracle
references and original failures. Private evidence and resource/recovery handoffs
remain outside Git. Independent technical review requires authorship separation
and actual examination; shared operator, model and host limitations remain visible.

Full host qualification, live executor compatibility, provider disclosure,
credential mediation, live budgets, protected integration and Stage 3/4 remain
separate gates. No capability or milestone status advances from this slice.

The [owned-scope continuation](owned-scope-fixtures.md) extends the same Harness
with explicitly selected scope and synthetic namespace profiles. Its separate
bounded grant permits temporary owned scopes under existing legitimate delegation,
without shared-policy mutation. Inspection remains non-effectful. Neither the
source adapter nor a successful narrow primitive qualifies the full required
resource, denial or recovery profile.
