# Bounded engineering evaluation foundation

This owner-authorized increment implements an in-memory foundation under
[CAP-EVL-001/002](../product/capability-catalog.md#cap-evl-001). It does not complete
either capability or Milestone 005. [DEC-008](../decisions/open-decisions.md#dec-008)
retains case selection, oracle protection, reviewer governance, comparison policy,
numerical targets and publication decisions. The
[controlled protocol](../evaluation/controlled-comparison-protocol.md) describes
the subsequent decision points, not an approved experiment or observed improvement.

## Observable acceptance and ownership

Given a frozen finite specification and complete declared trial population, a
trusted caller can register releases/attempts, retain original observations,
inspect compatibility, and report every failed, interrupted, skipped, blocked,
unresolved or incomplete attempt. A retry cannot replace earlier observations or
create another accepted work item in the same trial. A completed local branch or
functional PASS cannot automatically establish accepted integrated throughput.
Changed source/oracle/specification/profile inputs invalidate comparison use.

Core's `evaluation` module owns immutable specification, case/trial/attempt
identities, strategy definitions, explicit resource/configuration declarations and
separate source-tree syntax. Evidence's `evaluation` module owns bounded in-memory
attempt accounting, exact unified-Evidence/oracle correspondence, endpoint inspection
and reporting. CLI supplies actual Git acquisition in its development consumer.
Existing Core, Evidence, Storage, Harness and CLI dependency directions remain.
There is no new crate, dependency, workflow journal, storage codec, allocation
authority, permission mechanism, executor adapter or public command.

The integrated [functional two-task path](functional-two-task-fixtures.md) retains
its existing coordination, Evidence and effect owners. The composition regression
below observes only B's verification criterion. Whole-program measurement still
requires its own measurement contract and applicable acceptance/policy decisions;
it must not duplicate coordination machinery or treat an unmerged branch as main.

## Input contracts and populations

`EvaluationSpecification` retains an exact `SourceReference`, complete cases, arms,
trials and finite `EvaluationLimits`. Case and task identities are distinct. Cases
retain full task/criterion revision, starting `CandidateRequirement`, separate
`SourceTreeId`, oracle identity/version, workload family and disclosure class.
This first profile requires clean committed source. A commit and tree ID are
syntax-checked assertions until an adapter resolves actual objects. Dirty snapshot
identity and source authentication are not supplied.

`RunConfiguration` retains exact model/settings or explicit absence/uncertainty,
tool versions, harness, environment, information access and resource envelope.
The three proposed strategy arms are SequentialExecution, DirectParallelExecution
and GovernedParallelEngineering. Names do not implement or qualify their runtimes.
Harness/strategy are the declared treatment; model/tool/environment/information/
resource differences are reported as confounding. Matching opaque references do
not authenticate equivalent physical conditions or establish causal identification.

Every `TrialDefinition` names one case, strategy and positive repetition identity.
The case/repetition populations must match for structural comparison eligibility.
Repetitions do not assert statistical independence. Cases cannot duplicate an
engineering task under a new case name. Omitted trials remain visible; they are
never silently excluded from denominators. Failed/blocked/unperformed observations
are not exclusion criteria. An unobserved trial is missing, rather than an invented
NOT_RUN report. An explicitly unperformed observation retains NOT_RUN or BLOCKED.

Register each attempt before performing its measured operation. A registered
attempt with no report is incomplete. Distinct retry identities form one linear
trial chain and require the preceding report; unknown outcomes can be recorded
without authorizing another effect. The registry never schedules or retries work.
It refuses duplicate attempt/observation identities, changed terminal reports,
forked chains, missing references and malformed correspondence atomically.

The logical maxima are 32 cases, 256 trials, 256 attempts, three arms, 16 tools,
ten declared resource dimensions, eight cumulative metric dimensions and 32 phase
observations per report. Caller-selected limits can be smaller. Existing nested
text/source/Evidence constructors retain their bounds. These limits are finite
logical input bounds, not allocator/RSS or physical host guarantees.

## Observation and assessment integrity

`AttemptExecution` distinguishes existing execution observations, an unresolved
outcome and an explicit N/A scope examination. Canonical unified Evidence retains
PASS, FAIL, UNKNOWN, NOT_RUN, BLOCKED and NotApplicable with separate applicability
and validity. No conversion from legacy Inconclusive is introduced.

`OracleContract` freezes the exact case/revision/oracle and full verification
profile. `EndpointEvidence` consumes the existing original EvidenceRecord and a
separate candidate reobservation. The full task/criterion revision, result candidate,
profile and execution must match. An explicitly accepted endpoint requires CURRENT
applicable PASS. A stale historical PASS remains separately visible as stale; it
cannot acquire acceptance merely from inspection or successful exit.

`AcceptanceClaim` is separate from the semantic assessment. It identifies assessor,
authority reference, result scope, review separation and optional explicit
acceptance-event clock offset. The assessor must differ from every retained trial
executor and cannot claim SameAuthor separation for a primary endpoint. Local
completion is insufficient. SeparateAuthorshipSharedContext remains distinct from
an OrganizationallyIndependent assertion. References/separation are caller claims,
not authentication, qualified reviewer governance or protected integration approval.

The eventual evaluated executor must have no write access to reserved cases,
oracles or accounting. This module grants no such access, but it also implements
no hostile-worker protection. ReservedReference stores classification/reference
only, not held-out payloads. Public development test cases are not a qualified
held-out corpus. Effective protection/access/retention policy remains BLOCKED at
DEC-008/004/012 before operational evaluation; it is not inferred from Rust privacy.

## Primary timing and resource observations

The planned engineering primary measure is elapsed time from authorized task
release through independently accepted, appropriately integrated outcome, including
applicable preparation, coordination, implementation, verification, integration,
review, repair, requalification and required CI. Model generation time, branch
completion, agent count, lines of code and test count are not substitutes.

`TrialRelease` identifies the original release/authority reference, audit timestamp
and monotonic clock lifetime. `ElapsedInterval` offsets are nanoseconds from that
same release, retaining measured versus estimated basis. Retries retain original
release and gaps; their starts cannot precede any earlier known start, observed end
or acceptance event. Endpoint report/assessment timestamps cannot postdate the
terminal measurement; acceptance cannot predate the release. Wall time is not
silently converted into monotonic elapsed time.

The primary field uses the explicit `accepted_elapsed_ns`, rather than inferring
acceptance time from the last implementation interval. It requires a corresponding
monotonic development-fixture observation and a separate current integrated
acceptance claim. Synthetic timing cannot be labelled monotonic measurement and
estimates never supply measured primary latency. Missing acceptance or timing
remains absent. Phase intervals retain only actually supplied coverage and are not
summed, because they can overlap. There is no automatic clock reconstruction after
controller loss or cross-host clock correspondence.

MetricObservations keeps optional CPU nanoseconds, bytes written, model invocations,
input/output tokens, CI job-nanoseconds, operator interventions and review findings.
None is unmeasured; Some(0) is a reported measured zero. Totals expose a known
subtotal and missing-attempt count, with a complete total only when the declared
population is covered. Failed and interrupted attempts still consume their
reported quantities. Overflow refuses append without erasing the registered
attempt. Memory uses existing MemoryObservation basis/scope; process/cgroup samples
are never summed into AccountingPool allocations or inferred from elapsed time.
These quantities settle no resource reservation or historical debt.

Monetary cost/currency policy, full review/repair effort, disk occupancy, final
whole-domain consumption and operational resource enforcement are not implemented.
No missing dimension is reported as free or zero. Retry count is not automatically
a count of required repairs. Resource efficiency remains unestablished when its
required measurements are missing, even if elapsed time exists.

## Inspection and limitations

`EvaluationLedger::inspect` takes the complete current specification and oracle
contracts. A malformed mapping refuses without partial invalidation. A material
change retains its first complete invalidating specification/profile context;
restoring old references cannot revive that comparison. Original measurements and
terminal assessments remain unchanged. This is explicit contextual examination,
not automatic source reacquisition or a durable event history.

ComparisonEligibility answers structural correspondence under declared controls.
It is separate from MeasurementScope and primary_outcome_available. Synthetic
contract checks, actual development fixtures, unperformed work and mixed scopes
remain distinct. A structurally matching population without accepted endpoints
does not establish primary throughput. Planned live-provider performed observations
are refused; NOT_RUN/BLOCKED records remain available for unavailable arms.

`write_redacted` follows the existing library inspection convention using
std::io::Write. It prints scope, issues, full populations, metric missingness and
endpoint availability while omitting caller IDs, paths, oracle/configuration text
and raw reports. Writer errors propagate. It is an advisory library view, not a
new stable interchange/storage schema or a CLI compatibility change. The result
always withholds improvement, causal attribution and publication authority.

## Discriminating mechanisms and remaining gates

Core tests reject aliases, invalid membership/source and resource profiles.
Evidence tests preserve all outcome populations and retry history, distinguish
missing/zero/overflow, reject stale correspondence and inconsistent chronology,
retain oracle/source/profile invalidation, and separate structural comparison from
primary availability. Explicit hypothetical endpoint inputs test acceptance-offset
accounting across a retry gap; they are not independent real acceptance evidence.

The CLI consumer resolves a real Git commit/tree and measures a finite in-process
Rust calculation against an independent fixed literal. Its interval includes
registration/source acquisition and calculation, rather than claiming pure model
or arithmetic time. Two executions reproduce the same output/result/status scope;
actual times are retained rather than forced equal. Direct parallel remains NOT_RUN
and GPE remains BLOCKED as planned strategy runtimes. The actual fixture has no
integrated acceptance claim and reports no primary engineering throughput.

Actual command counts, failures, repairs, candidate identity and review/CI outcomes
belong in the private handoff. No GPE productivity improvement, acceleration,
statistical significance, general safety, real provider operation or full
CAP-EVL-001/002 status follows from these mechanisms. Durable records, protected
evaluation execution and any actual controlled strategy comparison require their
remaining decisions and separately authorized integration.

The combined [two-task fixture](../../crates/cortex-cli/tests/gpe_functional.rs)
also registers one consumer-verification evaluation attempt before B starts,
using its actual binding, oracle profile, process specification and reserved demand.
It imports the same canonical B Evidence into the advisory consumer after the
workflow, while the journal retains dependency invalidation and a synthetic Unknown.
Rejected oracle/origin changes leave the registered observation absent; a valid
functional PASS supplies no engineering acceptance, timing or missing metrics.
Byte/metadata comparisons ensure evaluation leaves the operational journal intact.
This case covers only B's verification criterion; the workflow retains its separate
complete effect population. It is a composition regression, not a controlled
strategy comparison or whole-program productivity measurement.
