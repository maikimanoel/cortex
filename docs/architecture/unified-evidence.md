# Unified evidence semantics (002-C)

This is the bounded implementation contract for the library portion of
[CAP-EVD-002](../product/capability-catalog.md#cap-evd-002). The catalog owns the
capability contract/status; the [roadmap](../product/roadmap.md#002-first-complete-vertical-slice)
owns its sequence. The implementation extends [002-B binding](candidate-binding.md)
without changing [002-A inspection](repository-inspection.md).
[002-D CLI integration](cli-check.md) composes it for one explicit criterion.
Evidence grants no operational authority.

## DEC-010: explicit extension, no implicit migration

The foundation's `Evidence`, `Assessment`, `Verdict`, `ExecutionObservation` and
`Provenance` APIs retain their original construction and transition rules.
`Verdict::Inconclusive` remains a terminal, undifferentiated inconclusive judgment;
it does not imply UNKNOWN, NOT_RUN, BLOCKED, applicability or validity. There is no
implicit or explicit conversion to the new model. Callers needing richer semantics
must supply a new report with the required identities, context and temporal metadata.

The new `cortex_evidence::unified` model separates reported facts, explicit judgment
and contextual validity. Extending the old enum or inferring richer records from
legacy values was rejected: those values lack the required distinctions and times.
No storage migration, dependency graph or serialization protocol is introduced.
This resolves [DEC-010](../decisions/open-decisions.md#dec-010) for 002-C, preserving
the [accepted foundation decision](0001-foundation.md) as its original bounded API.

## Identity, provenance and time

`ReportedObservation` retains the existing complete `CandidateObservation`:
its task, single acceptance criterion, contract revision, required candidate and
observed repository/HEAD/index/working-tree state, coverage and comparison basis.
There is one authoritative candidate/state model in Core. The committed revision
never identifies dirty bytes. Detached HEAD retains its commit; unborn HEAD
retains explicit absence. Unsupported Git captures still fail in the producer.

A separate `ObservationId` identifies the verification/inference/scope report.
`AssessmentId` identifies its explicit judgment. These roles are distinct from
`TaskId`, `ContractRevision` and `CandidateId`. Verification and candidate capture
IDs must differ, including the supplied reobservation ID. Observation scopes and
sequences remain producer-managed under 002-B; assessment IDs use foundation task
identifier syntax. There is no global uniqueness registry or authentication.

`VerificationProfile` retains foundation `Provenance` (mechanism, inputs and
configuration), the exact mechanism version, environment and up to 32 uniquely
named participating components with exact versions. Components are sorted for
comparison; an empty list means none declared. Future skill/harness versions must
be supplied when participating; none are fabricated here. Equality is conservative:
any profile change invalidates reuse, even a changed description.

`ObservationTimes` retains caller-reported observed and recorded `SystemTime`.
An assessment retains its caller-reported time, original source, full supplied
candidate reobservation and expected profile, with the initial validity and cause.
Times must be at/after the Unix epoch, recording cannot precede observation, and
assessment cannot precede recording. They are not authenticated execution start/end
times. The library samples no clock and performs no process, filesystem or network
operations. Elapsed time alone never establishes freshness.

All reports, oracle interpretations, versions, context and times remain **caller
assertions**. Typed construction does not authenticate an external tool or prove
that a check ran. The integration must perform actual captures; copied values with
new IDs do not demonstrate revalidation. DEC-007's scoped in-memory references do
not freeze persisted formats, Portable Context Packages, harness traces, transport,
privacy/retention policy or future API compatibility.

## Independent dimensions and construction rules

`EvidenceRecord::new` is unassessed. Compatibility and exit zero do not create an
assessment. `assess` requires an explicit identity, time, result, applicability,
nonblank rationale and validation context; it retains at most one terminal judgment.
Fields are private, with validated constructors and inspectable getters.

| Result | Required observation and applicability |
| --- | --- |
| `Pass` | Applicable; completed exit zero; positive relevant check count; explicit `Satisfied` oracle with reasoning; CURRENT context |
| `Fail` | Applicable; completed execution (including exit zero); positive relevant checks; explicit `Violated` oracle with reasoning; CURRENT context |
| `Unknown` | Applicable or uncertain; insufficient interpretation, interrupted execution, inference or scope examination; cannot disguise NOT_RUN or unavailable execution |
| `NotRun` | Applicable or uncertain; explicit foundation `NotRun` execution report |
| `Blocked` | Applicable or uncertain; explicit `Unavailable` execution report; substantive blockage detail and assessment rationale retained |
| `NotApplicable` | Not-applicable determination; explicit scope examination finding `Absent` with its basis; CURRENT context |

Applicability is independently `Applicable`, `NotApplicable` or `Uncertain`.
Uncertain applicability permits only UNKNOWN, NOT_RUN or BLOCKED. N/A cannot mean
missing tooling, missing evidence or a skipped check. `ScopeInspection` explicitly
reports examination of the criterion's governed subject/trigger, distinct from
`Execution` and `Inference`. Inferences retain derivation detail but cannot alone
support PASS or FAIL in this model. A caller-reported scope basis is inspectable;
its substantive truth cannot be mechanically established from prose.

`VerificationOutcome` retains the original execution status, relevant completed
check count and optional `OracleEvaluation`. An unperformed/incomplete execution
cannot claim completed checks or an oracle. Zero-check completion can be reported
but cannot carry a conclusive oracle or establish PASS/FAIL. A missing or
`Insufficient` oracle cannot establish PASS/FAIL. Timeouts/cancellation remain
UNKNOWN, with their original execution status visible. No process-status conversion
creates an assessment. Result, applicability and validity are never permissions.

## Contextual validity and invalidation

`ValidationContext` supplies the expected complete binding contract, an optional
separate current candidate capture and optional expected verification profile.
`assess` and `inspect` compare these with the retained report. This does not run a
verification mechanism or reinspection. Core's existing `bind_candidate` policy
checks ordered observations, exact contract applicability, repository/commit,
coverage, consistency, typed state and opaque comparison material.

| Context | Validity and behavior |
| --- | --- |
| Profile equal, Core binding compatible | `Some(Current)` within the declared observation coverage |
| Known repository/task/commit/working-tree mismatch, stale revision/order/state or changed profile | `Some(Stale)` with the typed cause |
| Missing capture/profile, different source scopes, missing comparison basis, incomplete/indeterminate observation, or matching dirty state under `AllowChanges` | `None`: validity UNESTABLISHED; no current acceptance |
| Same task/revision reused with changed contract contents, or conflated verification/candidate IDs | Invalid context; typed assessment error, no accepted judgment |

Checks first retain an existing invalidation; otherwise they check ID collision,
profile availability/equality, then Core binding in its documented order. A check
returns one cause, not an exhaustive diagnosis. Missing information is not itself
proof of staleness. A known incompatible required commit (including unborn HEAD)
is stale relative to that requirement, even if the record was never previously
current. `Clean` rejects dirty state; `AllowChanges` cannot establish dirty byte
identity. Equal HEADs, counts or status entries do not resolve that limitation.

`inspect` returns `EvidenceView`: historical result/applicability and separately
computed validity/cause. Missing judgment is UNASSESSED, not NOT_RUN. Missing
validity is UNESTABLISHED, not CURRENT. `current_pass()` requires all of PASS,
Applicable and CURRENT. Historical getters are explicitly historical.

Observed staleness is sticky in that record. Restoring an old context cannot
promote it. `invalidate(reason)` retains the first explicit reason. Revalidation
requires a new actual observation, record and assessment; there is no stale-to-current
setter. Terminal judgments and original reports cannot be overwritten. Invalid
judgment inputs leave them unchanged. A valid context examination may record
invalidation even when the requested PASS/FAIL/N/A is rejected; this prevents
rejection from discarding a demonstrated context change. Initial assessment context
and the first invalidation reference/cause remain inspectable; this is not a
persistent event history.

A view is correspondence to the supplied captures, not permanent freshness. Ignored
files and unobserved changes remain outside 002-A/002-B coverage. Two matching Git
samples are not atomic, and later changes may escape observation. Full dirty snapshot
identity and transitive dependency invalidation remain future work.

## Inspection, redaction and errors

Typed getters expose the full retained context to an authorized caller. They may
contain private paths, descriptions or reasoning. The non-versioned library
`EvidenceView::write_redacted` summary omits **all** caller strings, IDs, paths,
provenance, oracle text and comparison bytes. It preserves dimensions, typed issue
category, HEAD kind, coverage/consistency, dirty categories and execution/check
facts. Incomplete or indeterminate candidate facts are never displayed as clean.
New evidence `Debug` implementations also omit private context. This is deliberate
omission, not credential detection or a public machine serialization contract.
Writer I/O failures are returned; no repository export file is created by the library.

`SemanticError` distinguishes invalid text/identity/chronology, duplicate or excessive
components, identity collisions, contradictory observations, invalid applicability,
unsupported results, absent oracle, nonzero exit for PASS, unresolved/stale conclusive
context, reassessment and repeated explicit invalidation. Wrapped foundation/Core
errors preserve their causes. Diagnostics omit private payloads.

Text uses the existing 4096-byte UTF-8 bound and control-character restrictions.
Component lists are capped at 32; candidate bounds remain those of 002-B. Retaining
an assessment copies its supplied bounded validation context to preserve its basis.
There is no process/RSS/performance guarantee or unbounded history in this layer.
Production Evidence still depends only on Core. With 002-D, CLI depends on
Evidence/Core, and real producer integration tests live in the CLI consumer;
Evidence's former CLI dev-dependency is removed to preserve an acyclic graph.
The evidence model itself adds no third-party package.

## Acceptance mechanisms and limits

[Semantic tests](../../crates/cortex-evidence/tests/unified.rs) exercise allowed and
forbidden combinations, zero-check/exit-zero claims, explicit oracle, N/A basis,
blocked/not-run/unknown distinctions, source/version/profile mismatch, incomplete
coverage, sticky invalidation, deterministic repetition, terminal judgments,
construction bounds, private-text omission, writer failure and legacy preservation.
A compile-fail doctest rejects implicit legacy verdict conversion.
[Real-Git tests](../../crates/cortex-cli/tests/evidence.rs) traverse
002-A -> 002-B -> Evidence using an actual revision query and explicit comparison.
They cover clean, dirty, changed/restored, detached, unborn and foreign repositories.
Repeated clean and dirty checks compare every controlled fixture file/directory,
including `.git`, for content, modification time, length and read-only permissions.
Access time and platform-specific inode/change/birth metadata are not compared there;
existing 002-A/002-B invariance and inspection regressions remain required.

These are verification mechanisms, not a claim that an arbitrary invocation ran.
Executed counts, tools, exact candidate tree and delivery evidence belong in the
external handoff. No independent review, cross-platform assurance, authenticated
execution, atomic snapshot, privacy policy or hosted CI result is claimed.
CAP-EVD-002's bounded C/D contract is implemented with the
[002-D CLI path](cli-check.md); transitive validity/interchange remains planned.

## Dependency support inspection (A-1)

`cortex_evidence::readiness::inspect_support` is an additional bounded, synchronous
consumer of unified Evidence and the existing G1 projection. It evaluates the
Evidence portion of **one exact consumer edge**. Core does not depend on Evidence;
actual capture remains in the CLI/producer. This owner-approved advisory preview
completes neither CAP-EVD-003 nor CAP-WFL-004 or Milestone 003/004. Its scoped
[DEC-007/012 decisions](../decisions/open-decisions.md#dec-012) authorize no effects.

The caller supplies a `ReadinessProjection`, immutable `EdgeEvidenceContract`,
mutable canonical `EvidenceRecord` slice, borrowed `SupportRequest` and positive
finite `SupportLimits`. `EdgeScopeSnapshot::capture` retains graph/edge identity,
producer/consumer full mission scopes/generations, artifact/revision/candidate,
dependency kind, exact readiness level and required predicates. Its notification
cursor is historical only; unrelated Signal/capacity changes do not invalidate the
same semantic scope. The artifact has its own explicit repository/worktree
requirement; its candidate need not equal either mission candidate.

Each `PredicateCorrespondence` retains the exact full verification task, revision,
single criterion and candidate requirement, complete expected `VerificationProfile`
and caller-declared `CriterionCoverage`. `ExactSingleCriterion` maps that whole
frozen criterion to the predicate; `Unestablished` cannot supply support. Existing
`OracleEvaluation` has a conclusion and reasoning, **no standalone qualified oracle
identity or coverage manifest**. The component parses no rationale for entailment
and cannot establish independent qualification, capture authenticity or that the
reported mechanism ran. A requirement for such stronger support remains unresolved.
The mapping is edge-local data, not an engineering-method registry/adoption rule.

A `WitnessBinding` names the exact artifact and compound verification-report plus
expected assessment identity. Missing assessment references are never automatically
filled. All required predicates and all distinct assigned witnesses are conjunctive;
there is no latest-wins, majority or favorable-record selection. Exact duplicate
bindings add no support but still consume admission budget. Reusing an AssessmentId
across distinct reports is legitimate; duplicate record-pool identities are ambiguous.
Alias checks cover supplied material, not a global/cross-call identity registry.

`RecordExamination` supplies a legitimate source contract, current capture and
expected source profile independently of the target correspondence. A narrow
CURRENT PASS assigned to a broader predicate remains unsupported without poisoning
its valid source record. Current producer/consumer captures are checked separately
through existing `bind_candidate`. Missing, incomplete, indeterminate or incompatible
inputs remain inspectable. Source reports, judgment identities/times/rationales and
available historical invalidation causes are retained unchanged.

The result exposes `SupportReport::evidence_part`, ordered `findings`,
`inspected_records`, `basis`, `input_usage` and `authority`. Predicate findings are
`SupportedWithinDeclaredContract` or `NotSupported`, not new Evidence verdicts.
An applicable CURRENT explicitly assessed PASS, qualifying completed execution,
positive relevant checks and matching conclusive oracle are necessary but still
require exact criterion/profile/artifact/edge coverage. Result, applicability and
validity remain the existing independent dimensions. Even complete support does
not confer CONTRACT_STABLE review, provisional authority, INTEGRATED_ACCEPTED
acceptance or whole-mission completion. Authority is always `Unavailable`. No
G1 review/attestation/event, dependency release, permission, reservation or dispatch
is constructed. Genuine coordinator/operational acceptance remains separate.

### A-1 state-retention boundary

`inspect` is **not read-only toward its record**. The consumer first validates the
complete request, identities and budgets, then inspects bounded draft copies under
their own source contexts. All fallible result preparation finishes before publication.
A malformed `Err(SupportInputError)` leaves every supplied record and G1 unchanged.
A well-formed `Ok` negative support result still atomically publishes legitimate
inspection-driven sticky invalidations. It does not roll back known staleness or
explicitly invalidate records just because a target asks a broader criterion.
Unreferenced records are never inspected/mutated. Original observations and terminal
assessments are not replaced; only existing inspection invalidation can change.
Ordinary allocation panic/abort is outside this typed-error atomicity claim.

The report borrows committed originals immutably and owns its context/correspondence
basis. Retain `SupportBasis` and use `check_scope` to reject changed scopes,
same-revision definition aliases or a new correspondence revision before reuse.
That structural comparison does not recapture sources: repeat inspection with
actual current contexts. Old invalidated records cannot revive by restoring values;
revalidation requires a new actual observation and explicit assessment. A caller
who discards history or fabricates captures cannot be detected by this bounded API.
Existing sticky state retains a first issue/reference, not a complete historical
invalidation event/profile log; missing detail must not be invented.

`SupportLimits` bounds record-pool/examination counts, witness occurrences,
required-predicate/correspondence counts and charged nested material. Each paired
collection is independently bounded; usage reports the greater count. Budgets are
positive, finite, caller-selected and checked before staging. Material includes
paths/comparison bytes, profiles/provenance, oracle/assessment rationale, retained
assessment contexts and a conservative 128-byte scope charge for opaque existing
sticky comparison state. These are logical occurrence budgets, not total RSS,
host-resource enforcement, a persisted format or performance guarantee. No history
is evicted. New Debug/Display diagnostics omit caller payloads; typed getters expose
retained private data deliberately.

### A-1 acceptance mechanisms and limits

[Public Evidence consumer tests](../../crates/cortex-evidence/tests/readiness_support.rs)
trace A-01–A-32: positive/repeated/multi-predicate support, all non-PASS dimensions,
zero-check/absent-oracle refusal, candidate/contract/artifact/profile mismatch,
coverage and sibling isolation, sticky invalidation/new records, atomic rejection,
limits/identity conflicts, redaction, scope changes and no authority. Focused unit
tests cover accounting overflow and finite-limit validation. The
[real-Git consumer tests](../../crates/cortex-cli/tests/readiness_support.rs) execute
actual capture/query/oracle/assessment-to-support sequences, compare complete
controlled fixture/Git metadata and repeat a genuine query after restoration.
Execution observations/counts belong to the exact candidate's external handoff;
source mechanisms alone are not proof they ran.

This covers advisory Evidence-checking cases within G1-12, **not** authenticated
capture, trusted coordinator release/protected-oracle approval, real authorization,
reservation/dispatch or external recovery. Those remain NOT_RUN. G1-01–10 and
G1-11's existing in-memory scope are unchanged; G1-11 effects and full transitive
Evidence validity remain separate future work. Engineering Intelligence method
qualification, adoption and selection are not implemented here.
