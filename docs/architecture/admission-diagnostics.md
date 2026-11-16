# Bounded admission diagnostics

Storage owns this opt-in diagnostic extension under CAP-MEM-001, CAP-SEC-002
and CAP-WFL-001. The [program kernel](program-kernel.md), canonical Evidence
journal and [supervised fixture](supervised-fixtures.md) contracts retain their
authority. Capability and roadmap statuses are unchanged.

## Interface and correspondence

A caller creates an AdmissionDiagnostics buffer and passes it to
ProgramStore::admit_with_diagnostics. This additive method is available to
normal library consumers, including integration tests; it does not depend on
the library being built with cfg(test). Existing admit and dispatch methods
use a zero-sized disabled recorder.

The report contains the exact store incarnation, operation identifier and
expected predecessor from this call. Validation can reject that input; recording
its identity does not validate its grant, attempt, generation or payload.
The caller correlates those fields with its retained canonical input. Commands
without an attempt or generation acquire no invented identity. Reports are
separate invocation objects; repeated operation IDs do not make calls identical.

The caller-owned report resets at entry. Each record contains a typed stage and
a monotonic elapsed offset from that diagnostic invocation. Adjacent offsets
bound inclusive intervals; they do not separate CPU, I/O, scheduling or encryption
cost. No wall-clock timestamp is captured.

## Actual admission boundaries

The existing protocol checks the location and acquires one lifecycle lock,
checks readiness and budget, connects to SQLCipher and starts an IMMEDIATE
transaction. It reconstructs the journal, validates the profile and complete
operation, serializes bounded bytes, admits a draft and validates source
exclusion. A duplicate rolls back the read transaction and returns its original
receipt. A new operation inserts one row, checks the budget, commits, checks the
budget again and only then permits the existing trusted dispatch callback.

Diagnostics record these actual boundaries, including the validated ordinal.
Connection setup and SQL statements include encryption work; there is no
fictional separately measured encryption phase. Admission has no cancellation
input, so this interface adds no cancellation event or policy.

The report distinguishes:

- Lock requested/acquired. Lock exhaustion remains Storage(Busy), not Deadline.
- Insert acknowledgement from commit acknowledgement.
- Explicit rollback requested, acknowledged or failed.
- An already-ended SQLite transaction on the existing write-error short circuit,
  without claiming an explicit rollback ran.
- Post-commit deadline with a recorded commit acknowledgement.
- Final appended, duplicate or existing failed completion after lock release.

CommitAcknowledged means the existing commit call returned under SQLCipher's
configured DELETE/EXTRA contract. It is not independent power-loss proof.
A failed rollback does not prove absence; implicit cleanup on Drop is not an
explicit rollback receipt. Diagnostics cannot settle, refund or relaunch work.

## Bounds, privacy and observer effects

The fixed array retains at most 32 typed observations. Overflow increments a
saturating dropped-event count and makes complete() false. A caught unwind
leaves an incomplete prefix in the caller's buffer. Abort or process loss may
lose the entire in-memory report. A completed report means collection completed,
not that admission succeeded or that uncertain durable state was reconciled.

The allowlist is store ID, operation ID, predecessor, ordinal, typed stage/error/
completion and monotonic offset. No key, credential, path, operation payload,
private instruction, database bytes or raw engine error is retained.
Debug redacts the operation identifier. Explicit identifier access is intended
for caller-controlled private correspondence, not automatic public logging.

Enabled setup clones one bounded canonical identifier before admission's
budget/lock. Recording performs fixed-slot writes and monotonic clock reads.
It performs no formatting, I/O, additional locks, arbitrary callbacks or
per-event allocation. The disabled recorder initializes no report or clock.
This is a static cost assessment, not measured negligible overhead or identical
machine code. Observer cost can change whether a tight deadline is crossed.
Representative enabled/disabled overhead measurement is NOT_RUN.

The trusted CLI integration fixture opts into this method for its generic
admission helper and writes only fixed synthetic operation IDs, predecessor,
dispatch-slot count and redacted diagnostic events after admission returns.
The original admission outcome is retained even when the fixture separately
confirms exact newly owned metadata through canonical read-only reconciliation.
Dispatch and other effect transitions cannot use that fixture-only confirmation.
A dispatch slot is an attempted dispatch count, not proof of physical launch.
Native process observations remain the launch/termination evidence.

## Deterministic verification contract

The new unit cases use fresh encrypted fixture stores and no physical worker.
Existing read-only load/reconcile and exact operation/resource comparisons supply
oracles independently of the diagnostic event labels. A private cfg(test) seam
expires a budget at a selected existing check; it adds no public fault control
and does not change production deadlines. The same pinned rusqlite dependency's
hooks feature is enabled only as a development dependency for actual rollback
statement denial; no new package or persistence format is introduced.

| Case | Controlled boundary | Independent oracle | Prohibited conclusion |
| --- | --- | --- | --- |
| A | Stale expected predecessor | Rejected; unchanged bytes/revision; no write | Rejection authorizes work |
| B | Budget expiry after real commit | Indeterminate; exact dispatch retained; reservation retained; duplicate/changed replay cannot launch | Unknown is absent or retryable |
| C | Budget expiry after insert, before commit | RolledBack; original operations and reservations retained; exact input absent | Reservation refunded |
| D | SQLite authorizer denies actual ROLLBACK after insert | Indeterminate; RollbackFailed, no acknowledgement; later load preserves reservations | Connection teardown proves acknowledged rollback |
| E | Budget expiry immediately after lock acquisition | Rejected; no transaction/write/rollback; unchanged bytes | Deadline implies durable mutation |

Case D exercises a real SQLite API failure, not disk corruption, power loss or
OS rollback failure. Connection close may clean up afterward; a later observation
does not relabel the original Indeterminate return. Other tests cover duplicate
receipts, incomplete unwind, explicit overflow, identity correspondence, redacted
formatting and monotonic ordering. Test source is a mechanism; actual results
belong to candidate-scoped private verification evidence.

The historical main-CI deadline remains failed and its operation, admission stage,
commit disposition and root cause remain unknown. Passing deterministic cases
does not retrospectively explain or reconcile it.
