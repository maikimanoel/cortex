# Controlled engineering comparison protocol

This is a proposed protocol under CAP-EVL-001/002, not an approved benchmark policy
or a record that a study ran. The implemented
[measurement foundation](../architecture/evaluation-foundation.md) supports frozen
inputs, complete populations and explicit missingness. DEC-008/005/012 must select
the experimental/protection/acceptance policy before an effectiveness claim.

## Question, hypotheses and endpoint

Question: under matched tasks, information, model/tools, host conditions and
resource envelopes, does GPE change time from authorized task release to an
independently accepted, appropriately integrated engineering result, while meeting
the selected quality, safety, reliability and resource-efficiency constraints?

The null and alternative must be preregistered against the selected estimand and
owner-approved practically meaningful difference. The directional hypothesis is
that GPE can reduce accepted-outcome elapsed time on suitable workloads without
violating those constraints. This document chooses neither a superiority margin
nor a noninferiority margin, sample size, confidence level or publication decision.
The aspirations of 2x throughput on highly parallelizable work and 30% less time on
mixed work are provisional ambitions, not acceptance thresholds or observations.

The primary elapsed endpoint includes applicable preparation, scheduling,
coordination, implementation, verification, integration, review, repair,
requalification and required CI. Failed or not-yet-accepted work has no fabricated
successful latency. Record its censoring/status and consumed resources. Define
the throughput denominator and observation window before execution; attempts,
accepted increments and repeated cases are distinct units.

## Arms and matching

| Proposed arm | Treatment and prerequisites |
| --- | --- |
| Sequential Execution | One Codex engineering path executing sequentially; no such live runtime is qualified by this foundation |
| Direct Parallel Execution | Multiple paths coordinated without GPE, using the same selected task/information/model/resource conditions; independent operational authority and observation required |
| Governed Parallel Engineering | Cortex admission, dependencies, Evidence, coordination, supervision and recovery; applicable integrated runtime/host prerequisites required |

Freeze task difficulty/acceptance, starting source commit/tree, case/oracle versions,
input/context access, model/settings, toolchain, available compute and cumulative
budgets, environment and required acceptance/CI for each matched case. Harness and
coordination strategy form the declared treatment; changing model and harness
together is confounded. Disclose materially different resource, host, information
or tool conditions; exact labels are not physical matching evidence. Unqualified
arms remain NOT_RUN or BLOCKED and cannot be replaced by a fast local fixture.

## Workload population and protected inputs

Select and freeze representative workloads before outcomes are observed:

| Family | Candidate case pattern and independent negative oracle |
| --- | --- |
| Independent | Disjoint well-specified changes; reject shared acceptance regressions despite local success |
| Dependency-heavy | Intermediate contract/artifact fan-out/fan-in; reject stale revision or omitted dependent checks |
| Integration conflict | Overlapping interfaces/invariants; reject independently passing branches that fail integrated acceptance |
| Interruption/recovery | Finite interruption boundaries and uncertain effects; reject blind retry, erased attempts and invented settlement |
| Real roadmap | Separately authorized bounded repository task; use owner-selected accepted criteria and unaffected regression checks |

Inclusion/exclusion rules, task-family weights and case/repetition identities must
be registered before execution. Exclusions require a predeclared rule and a
retained disposition, not removal of a failure. Preserve all execution errors,
skips, blocks, interruptions, retries, Unknown and open attempts. Register attempts
before work, so incomplete work is observable; no constructor proves that an
external actor disclosed every attempt.

Keep reserved/held-out cases, independent oracles and accounting outside evaluated
executor write authority. Select the actual access-control boundary and evaluator
governance under DEC-008; this source increment does not enforce them. Public
development fixtures test instrumentation only and are not a protected held-out
evaluation corpus. Preregister negative/adversarial cases for oracle manipulation,
stale source, hidden failures, shifted input access, review self-approval and
missing-resource reporting. Never expose protected expected results as worker input.

## Assignment, repetition and independence

After instrumentation qualification, propose matched-case randomized arm order or
a documented balanced/counterbalanced alternative. Freeze seed/assignment procedure
and disclosure before executing the selected study. Reset permissible caches and
state consistently, or measure and disclose their effects. Isolate contamination,
operator learning, shared host contention and carryover where the approved design
permits; labels/worktrees alone do not establish physical isolation.

Choose repetitions from an approved design with variance/precision rationale.
Do not choose sample size after seeing a favorable result. Repeated attempts of
one trial are retry/repair history, not independent replications. Repeated cases
or shared operators/models/hosts require explicit dependence assumptions and
appropriate clustered/paired analysis. Separate source authorship review from
organizationally independent engineering acceptance. This foundation authenticates
neither and supplies no default reviewer threshold.

## Observation, missingness and analysis

Retain exact specification, source/tree, candidate, attempt, model/harness/tool/
environment/oracle versions, declared limits, actual observation sources and
acceptance/integration references. Use monotonic release-to-acceptance observations
where available; retain wall/audit timestamps and clock lifetimes separately.
Observe phase intervals only where mechanisms actually capture them. Include
review/repair/requalification/CI time and retry gaps in the primary endpoint.

CPU consumption, memory basis/scope, disk use, model invocations/tokens/cost,
CI consumption and human interventions/effort need actual measurement definitions.
Declared limits, measured quantities and physical enforcement are distinct.
Unknown consumption is not zero; retain known subtotals and incomplete populations.
Do not use complete-case filtering to turn missing failures into apparent savings.
Preregister censoring, missing-data treatment and sensitivity analyses. An Unknown
effect cannot be automatically retried or refunded by a measurement consumer.

Report individual trial/attempt populations and status, observed effect sizes and
appropriate uncertainty under the selected paired/dependent design. Statistical
and practical significance are different questions. Select interval/test methods,
confidence levels, multiplicity handling and decision thresholds before claims;
this document provides no invented sample size or achieved acceleration. A better
elapsed aggregate cannot average away a required quality/security/reliability
failure or justify an unmeasured resource-efficiency claim.

## Invalidation and no-conclusion conditions

A material specification, task, source/tree, oracle, model/tool/environment/input
or budget change requires a new exact context and affected requalification.
Do not overwrite terminal observations or relabel an old result current. Missing
trial/attempt population, inconsistent timing, confounded treatment, insufficient
independence, unauthorized assessment-input changes or execution outside the
approved profile invalidate the affected comparison or leave it non-comparable.

No effectiveness conclusion is justified without qualified arm implementations,
matched conditions, protected and independently accepted endpoints, complete
failure/resource accounting, adequate selected uncertainty analysis and applicable
owner publication authority. The present fixture establishes instrumentation and
contract behavior only. A future provider-free comparison can qualify additional
instrumentation before any separate live provider or host-experiment authorization.
