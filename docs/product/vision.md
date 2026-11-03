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

## Working alongside an external coding assistant

Cortex is an original local-first Rust engineering platform, not an AI agent,
LLM or replacement for an external coding assistant. The assistant proposes and
may perform separately authorized work; Cortex supplies native contracts, qualified
methods, context, state and evidence within delivered scope, with enforced gates
remaining future work. The journey below is planned except for the bounded manual
inspection/check path described under [demonstrated behavior](#ambition-and-demonstrated-behavior).

1. **Start with the target project.** Open an unrelated repository and inspect its
   actual candidate using supported Cortex operations. Manual/offline use remains
   possible without a model, provider, connector, MCP or instructional skill.
2. **Optionally prepare assisted use.** Choose an actually supported external
   executor. A future read-only [preflight](capability-catalog.md#cap-cli-004) shows
   project/candidate identity, Cortex availability, observed adapter capabilities,
   missing prerequisites and proposed file/settings changes. Review any persistent
   bridge before narrowly owned, reversible changes. Existing instructions remain
   untouched without approval; association syntax and storage are still
   [open decisions](../decisions/open-decisions.md#dec-007).
3. **Launch the external CLI normally.** The assistant may discover/query delivered
   Cortex capabilities for contextual guidance and verification. Native CLI comes
   first; an optional executor-specific instruction bridge, later optional
   Rust-native MCP and thin instructional skills are discoverability/transport
   surfaces over the same contracts. They require no copied methodology in each
   prompt and create no separate method store or authority. An instruction file
   cannot guarantee adherence or place an operator-started process under Cortex
   host enforcement. General contextual method selection remains planned; P0
   supplies bounded advisory qualification inspection only.
4. **Use governed effects only when available and authorized.** Future execution
   depends on scoped Core grants, actual host controls and required current
   evidence. Unsupported controls hold dependent effects while independent
   authorized inspection remains usable. Optional parallel work consumes exact
   reviewed artifacts under [the existing release contract](../architecture/governed-parallel-engineering.md);
   G1/A-1 previews grant no operational release or execution authority.
5. **Inspect and reconcile.** Retain the exact candidate, changed artifacts,
   current/stale findings, unsupported checks, omissions and outstanding decisions.
   Future [portable handoffs](capability-catalog.md#cap-mem-003) support manual
   snapshot transfer with source/version and integrity information; they create
   neither approval nor live verification.

These conceptual user-visible labels describe existing domain meanings; they are
not runtime enums, capability statuses or a new lifecycle:

| Label | Canonical meaning and limit |
| --- | --- |
| unattached | No optional project/executor association; the supported manual baseline remains usable |
| preflight / needs review | Candidate/capability/configuration observations and a proposal awaiting applicable operator review; no grant or effect |
| assisted-ready | Observed assisted-query prerequisites are met for the inspected scope; no task acceptance, host-isolation or effect permission claim |
| degraded / unsupported | A required capability or check is unavailable/unsupported; preserve diagnostics and NOT_RUN/BLOCKED or uncertainty as applicable, with a permitted manual path |
| stale | Evidence/context has a known relevant mismatch under INV-002/005; missing validity remains UNESTABLISHED |
| effect-blocked | An affected operation lacks current authority, enforceable controls or required evidence under INV-001/006/008; no permission inferred from text |
| reconciled | The handoff and candidate/evidence basis have been examined under CAP-WFL-003; failures, gaps, stale findings and pending approvals remain visible |

The whole journey draws on every [catalog family](capability-catalog.md): Core and
Evidence govern identity and conclusions; Knowledge and Memory supply bounded
source facts and continuity; Skills and Workflows qualify/select methods; Harness
and Security govern actual effects and privacy; Evaluation and Evolution assess
proposed improvements without self-approval; Context and CLI expose the same
contracts. All twelve invariants apply, including local operation, proportionality,
history/privacy and separately authorized distribution. No bundled model, SaaS or
third-party orchestration engine is required, and no acceleration or agent
independence is demonstrated by this journey.

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
permissions or execute agents. The first 003-A library slices add canonical temporal
history and bounded encrypted local persistence with explicit operator inputs;
general memory/recovery and the remaining 003 contracts remain incomplete.
The [roadmap](roadmap.md) schedules that work;
plans are not execution authorization or evidence of delivery.

Cortex does not promise infallibility, universal compatibility, or objectively
better outcomes without measurement. Built-in quality includes honest stops,
blocked actions, explicit anomalies, and useful partial results. The initial source
contribution policy is [resolved under DEC-003](../decisions/open-decisions.md#dec-003);
broader governance and public-launch approval remain open at their decision points.
The owner has selected the [MIT License](../../LICENSE); publication requires
separate authorization.
