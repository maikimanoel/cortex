# GPE-B Stage 2A operational foundations

Stage 2A implements operational contracts and controlled fixture projections under
the existing CAP-CORE-003, CAP-WFL-001/004, CAP-HRN-001/002, CAP-CTX-002,
CAP-MEM-001, CAP-SEC-002 and CAP-CLI-004 owners. It does not authorize real
executor dispatch, provider disclosure, host qualification experiments or protected
integration. These foundations complete neither a capability nor a milestone.

## Ownership and acceptance contract

Core owns pure policy, scope, resource and effect transition guards. Workflow's
existing `CoordinationKernel` composes them with program claims, instruction
packages and Evidence-backed dependency decisions. Storage persists the same
complete input journal and reconstructs projections through those guards. CLI
only inspects the resulting canonical state. No runtime crate or second effect
authority is needed until a real host consumer is authorized.

A controlled fixture can retain an exact intent and reservation, record a dispatch
boundary as uncertain, reopen the encrypted journal, and reconcile that same
operation without silently dispatching it again. Unknown outcomes retain their
reservations. Cancellation or grant revocation alone cannot prove termination or
release resources. Changed identity reuse, stale attempts, unknown required
capacity, insufficient verification reserves and duplicate execution are refused.
Independent eligible fixture work can proceed when another outcome is unknown.

`OperationalProjection` binds proposed grants to the exact program, baseline,
mission candidate, attempt/generation, instruction package, policy and principal.
Logical ticks are monotonic caller assertions with inclusive start and exclusive
expiry, not an authenticated clock. There is one effect retry chain per metadata
attempt. Reissuing a grant/package/effect identity cannot restart that chain;
changed identity reuse and forking a predecessor are refused. A fresh metadata
attempt still needs an explicit claim and new exact proposed grant. The program's
finite journal and effect bounds remain in force across policy replacements.

Policies name every required resource dimension and classify cumulative usage
separately from reusable capacity. Unknown quantities or undeclared enforcement
hold reservations. Pending reservations and settled cumulative usage remain
accounted across policy versions. `preview_schedule` subtracts that accounting
and delegates ordering to the existing Stage 1 simulator; it creates no reservation.
Known insufficient verification reserves report contention, including a zero-demand
proposal when the reserve itself cannot fit. Required dimensions cannot be omitted.
G1's advisory free-capacity signal does not charge an already owned Active attempt
again; the fixture ledger owns quantity admission. Other mission preconditions,
context and dependency blockers remain effective.
The complete instruction package must also remain structurally current. Stage 1
packages include advisory blockers and the plan revision, so changing those inputs
after assembly can hold dispatch on package correspondence even when resource
admission would fit. This is not a semantic non-impact proof. Assemble against the
intended current state; a stale reserved fixture can be cancelled before dispatch
and requalified through an explicit fresh claim/package/grant. No stale package
is silently relabeled current.

An intent enters `Reserved`; a separate durable fixture input enters
`DispatchUncertain` before a hypothetical interaction. Running, unknown, report
and cancellation observations remain distinct. A report retains its original
outcome and still needs explicit usage/outcome reconciliation. Known running or
reported effects cannot later be declared absent. Positive running observations
after cancellation retain the cancellation request. Revoked or stale attempts
cannot dispatch, while exact historical outcomes can still be reconciled after
baseline changes. Outstanding effects prevent claim retirement or handoff in the
canonical kernel. None of these transitions runs a process.

Policy and operational grant inputs are explicit local assertions, not approved
runtime grants. A matching fixture policy is not effective host permission.
Fixture outcomes do not create Evidence assessments, release consequential
dependencies, complete missions or establish real process/resource guarantees.
The accepted Stage 1 dependency engine remains the sole release path.

## Versioned persistence extension

The existing metadata-only program format 1 and history format 1 retain their
contracts. An explicitly selected program fixture format 2 uses the existing
SQLCipher lifecycle, transaction and journal ownership with a distinct policy
identifier. It admits fixture effect inputs; format 1 refuses them. Creation is
explicit, with no automatic conversion or migration of existing stores.

Both formats retain complete ordered inputs rather than trusted serialized
projections. Exact replay returns the original receipt. Refusal changes no
canonical state. Missing, replaced or corrupt storage cannot establish absence.
The trusted local Linux operator, responsive filesystem, finite encoded limits,
encryption, source-root exclusion and uncertain-commit limitations of the
[program kernel](program-kernel.md) continue to apply. No raw executor traces,
provider sessions or credentials are collected by this extension.

This is an intentionally bounded format for operational **fixtures**. Selecting
and qualifying a real dispatch protocol remains a DEC-007/006 decision. Storage
commit and an external effect cannot be promised as one transaction.

## Executor boundary

Native contracts distinguish advertised controls, controlled fixture observations
and caller-supplied host qualification reports. Negotiation exposes missing,
unsupported and insufficiently supported controls. Exact descriptor and package
correspondence is required for prepared input. Instruction bytes confer no
authority. No trait, constructor or reported qualification authenticates the
underlying host guarantee; no process launch API is supplied here.

`PrepareExecutorInput` retains the full descriptor, mandatory-control profile and
exact package reference in the same journal. Reconstruction repeats validated
construction, negotiation and package checks. Reusing a descriptor or requirement
reference for different contents is refused; a new exact version is a distinct
input. Preparation preserves every package byte without wrapping or submitting it.
Concrete allowed tool/path/network policy, authenticated principals, actual
transport parsing and host enforcement remain later runtime obligations.

The existing `cortex.program.v1` JSON adds `fixture_projection`, with nested schema
`cortex.fixture-effects.v1`: proposed policy/grant counts, logical tick, revoked
grants, exact effect scopes and statuses, pending accounting and prepared-input
correspondence. Real reservations remain empty, execution authority remains
unavailable, and preparations explicitly report `submitted: false`. Human output
labels fixture effects and pending reconciliation. No raw package or trace is
printed. The existing explicit-key, private-output and read-only refusal contract
continues unchanged. A passive journal can still require explicit recovery access;
read-only inspection never removes it to obtain a successful view.

## Verification and remaining gates

Verification must discriminate changed identity reuse, stale scope, capacity
contention, arithmetic overflow, revocation, cancellation, lost replies, replay,
recovery and legitimate unaffected work. Real-command CLI tests must retain
read-only behavior and label all effect records as fixtures. Storage tests must
exercise the selected format and prove the old profile cannot silently accept
new effect inputs. Test source is not evidence that a check ran.

DEC-004 still requires prompt/session/trace disclosure and retention policy;
DEC-005 requires actual numeric operation budgets and enforceable verification
reserves; DEC-006 requires an approved, executed host/executor qualification plan;
DEC-007 requires the real effect/reconciliation protocol; DEC-012 requires real
principals, grant/revocation, method/reviewer and integration policy. Read-only
installed-tool observations establish only their observed metadata. Stage 2B and
protected merges require separate owner authority.
