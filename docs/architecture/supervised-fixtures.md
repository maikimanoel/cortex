# GPE-B Stage 2B native supervised fixtures

The owner authorizes native runtime implementation and provider-free development
verification under CAP-CORE-003, CAP-WFL-001/004, CAP-HRN-001/002, CAP-CTX-002,
CAP-MEM-001, CAP-SEC-002 and CAP-CLI-004. This bounded slice supplies a native
process consumer of the [Stage 2A contracts](operational-foundations.md). It does
not qualify an AI executor, activate an isolation backend or complete a capability
or milestone. Actual verification remains candidate-scoped delivery evidence.

## Observable acceptance

An explicitly selected trusted local fixture may be launched only after its exact
intent, resource reservation, prepared input and immutable process specification
are retained, and a new dispatch-uncertainty input commits. Competing or replayed
dispatch requests cannot launch again. The launch boundary checks the current
contract and executable correspondence. Expired, revoked, mismatched, unsupported
or incomplete inputs refuse. The recorded candidate is not refreshed by replay.

The supervisor pumps bounded input/output, observes direct-child exit, and responds
to cancellation, elapsed deadlines, malformed output and authority loss. Process
observations remain distinct from mission acceptance and dependency releases.
Lost replies, interrupted controllers and incomplete cleanup retain uncertainty;
neither reopening nor an ordinary inspection invokes a process. Original reports
and failed observations survive later reconciliation. No blind retry is supplied.

## Owners and dependency direction

Core owns process contracts and the existing operational projection's transition
guards. Evidence's `CoordinationKernel` remains the single Workflow input journal
and composition point. Storage owns encrypted encoding, locking and commit
reconstruction. CLI projects that same state without performing effects.

`cortex-harness` owns actual trusted-fixture process capture and supervision. Its
consumers are the provider-free integration fixtures and Rust library caller. It
depends on Core, Evidence and Storage; none depends on Harness. The crate has a
concrete process-resource lifetime and I/O responsibility, rather than mirroring
a conceptual subsystem without a consumer. It does not own another permission
model, resource ledger, workflow database or acceptance decision.

Keeping process execution inside Core would break its infrastructure independence.
Putting it in Storage would mix engine durability with host policy. CLI-only
execution would make the canonical library path depend on presentation. The
separate Harness boundary preserves these directions and permits explicit refusal
of unsupported future executors. No third-party orchestrator is introduced.

## Explicit profile and serialized dispatch

Program format 3, `local-operator-program-supervised-fixtures-v3`, is selected
through a new explicit creation API. Original metadata format 1, pure fixture
format 2 and history format 1 retain their contracts. There is no migration or
automatic activation. A pure fixture event cannot acquire process authority by
changing a format tag, and external-executor mode remains unavailable.

The same canonical input journal retains the process specification and observations.
It reconstructs through validated Core/Evidence transitions, never from serialized
accepted projections. Preparing a process does not launch it. Distinct process and
journal operation identities prevent changed reuse and distinguish one launch
from repeated delivery of an observation.

`ProgramStore::dispatch_fixture` accepts only a supervised dispatch input. It
validates and commits under the existing lifecycle lock, then invokes the trusted
supervisor's short launch callback before releasing that lock. Cooperating clients
cannot revoke or amend between this check and the callback. Exact duplicate input
returns its retained receipt without invoking the callback. Rejected or uncertain
commits never invoke it. The callback must not reenter the store or wait for the
child to finish. A crash after commit preserves dispatch uncertainty, including
when no process was ultimately started.

There is no transaction spanning SQLite and process creation. Callback success,
process exit and durable observation admission are separate boundaries. A lost
spawn reply cannot be resolved by replaying the callback. An unknown process is
not adopted or killed using a persisted PID alone; exact launch/incarnation and
applicable ownership evidence are required for any later reconciliation.

## Supported development boundary

Only explicitly selected, audited, owned provider-free fixture binaries are
supported. The operator supplies the exact executable, source/profile references,
working directory, finite arguments, input, limits and approval basis. Executable
bytes are pinned within an explicit memory bound and compared before launch.
Linux launches through the retained executable descriptor. The final boundary
also rechecks elapsed time and cancellation after correspondence inspection.
This is byte correspondence under a cooperating non-mutating filesystem, not
cryptographic authentication or protection against hostile same-user replacement.

The native supervisor controls its retained direct `Child`, clears the environment
and uses bounded nonblocking local stream endpoints. It does not detach reader
threads that can outlive an output deadline. Output syntax checking is not semantic
acceptance. Private output is not automatically copied into journal traces or
diagnostics; the retained observation contract contains typed outcomes and counts.

This is not a filesystem, network, credential or descendant sandbox. An audited
fixture's promise not to use those effects is a trusted development precondition,
not a demonstrated denial. Parent exit does not prove descendant cleanup. Retained
streams or uncertain cleanup remain explicit. A mandatory unsupported control
refuses; descriptor advertisements and caller-supplied support cannot override the
native backend's actual support matrix.

The process contract selects positive finite bounds instead of default agent
budgets. Unit-test choices are test configuration, not live executor allowances.
CPU capacity, CPU time, memory, disk, elapsed time and output are distinct; existing
resource units are not silently reinterpreted. Unsupported hard quotas and reserves
remain unavailable for live execution.

Inspection distinguishes original reserved demand from canonical accounted usage.
Observed overruns remain charged while uncertain and after explicit quiescent
failure settlement. A proven pre-spawn refusal retains measured controller time
without inventing child use. Cumulative time/output remain charged after settlement;
the resource ledger does not claim hard host quotas or reservations.

Supervised-fixture logical ticks are controller-reported milliseconds. A newly
committed dispatch anchors a monotonic elapsed interval; grant expiry is rechecked
against that interval and current canonical state. Restart does not create a new
clock origin for an old dispatch. The older pure fixture tick contract is unchanged.
Authority loss requests direct-child termination but does not itself prove cleanup
or release reservations. Store re-observation and OS I/O remain cooperative on the
supported responsive local filesystem, not hard real-time guarantees.

## Verification and remaining gates

Required tests discriminate changed specification/executable/package correspondence,
stale or revoked authority, concurrent dispatch, replay without launch, commit and
controller crash cuts, unknown outcome, cancellation, terminal report preservation,
bounded output/time, malformed frames and legitimate unaffected work. Storage
tests cover encryption, profile separation and recovery; actual CLI subprocess
tests preserve read-only bytes/metadata and private-input-safe diagnostics.
Independent review must examine the code, test oracles and actual observations;
authorship separation alone is not proof of comprehensive independence.

Live Codex invocation remains unavailable. It requires an explicitly approved
numeric operation envelope and verification/recovery reserve, provider disclosure,
credential mediation and retention selections, applicable real host isolation and
denial evidence, and independent acceptance of that exact runtime candidate.
Installed Codex or OS utilities do not establish those guarantees. Protected
integration has no Stage 2B delegation. Stage 3/4 and unrelated roadmap capabilities
retain their separate dependencies and grants.
