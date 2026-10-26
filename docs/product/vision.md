# Product vision

Cortex is a software engineering platform designed to turn human intent into
constructed, verified, and continuously improved solutions by integrating
intelligence, execution, knowledge, and responsibility into one coherent
engineering process. It engineers arbitrary software with agents; the resulting
application does not need to contain AI. Work must remain authorized, inspectable,
reproducible where possible, and assessed against explicit contracts.

The name evokes the cerebral cortex as a metaphor for perception, attention,
reasoning, planning, decision-making, language, memory, and learning. This is a
product metaphor, not biological equivalence or a promise of infallible intelligence.

## Problem and audience

Software work crosses requirements, source versions, tools, people, and sessions.
A plausible proposal is easy to mistake for an authorized operation; a successful
command is easy to mistake for a correct result; remembered context is easy to
mistake for current knowledge. These distinctions become harder to maintain when
agents and tools act across many steps.

Cortex serves individual developers operating locally, maintainers coordinating
changes across sessions, engineering teams requiring inspectable decisions, and
operators evaluating agents under controlled conditions. Critical work still
requires the qualified reviews and external approvals required by its contract.
Automation does not manufacture reviewer independence or human authorization.

## Operational model

An operator supplies a task, its acceptance conditions, and the authority under
which work may proceed. Probabilistic agents may interpret, plan, and propose.
A deterministic Rust control plane governs candidate identity, policy decisions,
state transitions, tool boundaries, resource limits, evidence, and completion.
Effective host controls must enforce those decisions at the point of action.

An optional Plan–Execute–Reconcile method may connect free-form intent to a
reviewed typed contract, authorized execution, and a candidate-bound handoff for
evidence-based reconciliation. It may use one agent or heterogeneous agents.
Optional Portable Context Packages support manual snapshot exchange without a
particular chat service or repository connector. A recipient without live access
can reason about the included snapshot but cannot claim live verification.
These remain planned contracts in [the catalog](capability-catalog.md#cap-wfl-003).

Three commitments bind the product together:

1. **Authority has an enforcement boundary.** Typed contracts and actual host
   controls govern consequential actions. Instructions and recovered context
   cannot grant themselves tools, privileges, or budgets.
2. **Claims carry evidence.** Observations, inferences, evaluations, and approvals
   retain separate identities and meanings. Missing execution never becomes pass.
3. **Context has time and provenance.** Knowledge, decisions, observations, and
   memory identify their sources and relevant versions, state when they apply,
   and become stale explicitly when their dependencies change.

The [core invariants](../architecture/core-invariants.md) make these obligations
precise. The [capability catalog](capability-catalog.md) is the authoritative
functional inventory; this vision does not add an alternative feature register.

## Scope and value

The intended platform combines task contracts, unified evidence, temporal code
intelligence and memory, typed skills and workflows, isolated execution, governed
evolution, empirical evaluation, and a coherent local CLI. Its value comes from
their shared authority, candidate, evidence, and temporal semantics. A graph,
prompt, scanner, or execution adapter contributes to that system; none defines
the whole product by itself.

Essential capabilities must work with free, locally operated or operator-controlled
resources. Optional providers and accelerators require explicit opt-in, report
their limitations, and fail diagnostically without displacing the complete local
baseline. Hardware, energy, storage, computation, and operator time remain costs.

Deliver useful verified slices progressively. As capabilities mature, Cortex may
help build Cortex within their demonstrated limits and actual authority. This
does not make an incomplete milestone complete or let an executor approve its own
consequential integration. Parallel work, when authorized, needs shared contracts,
bounded independent workspaces, and revalidation of the integrated candidate.

## Ambition and demonstrated behavior

The long-term outcome is continuity from an engineering requirement to an
authorized action, an observed result, and an inspectable conclusion tied to an
actual candidate. Improvements must be proposed and evaluated against explicit
oracles and baselines. Compatibility, reliability, retrieval quality, security,
and performance claims require observations in the stated environments.

The current implementation retains the three-crate foundation described in the
[accepted foundation decision](../architecture/0001-foundation.md) and adds
[002-A bounded local Git inspection](../architecture/repository-inspection.md) and
[002-B typed candidate/task binding](../architecture/candidate-binding.md), plus
[002-C unified evidence](../architecture/unified-evidence.md).
It validates in-memory task/evidence values, observes supported repositories,
compares candidate observations against explicit task revisions and independently
represents assessment result, applicability and contextual validity with reported
times. Binding does not create an assessment. The CLI prints help/version/inspection;
`cortex check` now composes task binding and unified evidence for one bounded
committed-HEAD equality criterion, with human/JSON output. It does not enforce task
permissions, execute agents or persist temporal history.
The [roadmap](roadmap.md) schedules that work;
plans are not execution authorization or evidence of delivery.

Cortex does not promise infallibility, universal compatibility, or objectively
better outcomes without measurement. Built-in quality includes honest stops,
blocked actions, explicit anomalies, and useful partial results. Public release,
licensing, and contribution governance remain [open decisions](../decisions/open-decisions.md).
