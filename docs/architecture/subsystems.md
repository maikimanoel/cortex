# Subsystem ownership and interactions

These are logical responsibilities, not existing crates, APIs, storage schemas,
or service deployments. Actual crate ownership is in [the overview](overview.md).
The [catalog](../product/capability-catalog.md) owns functional contracts and
acceptance, while [core invariants](core-invariants.md) apply to every interaction.
All planned record flows retain identity, candidate/version references, provenance,
domain/recording times, validity, and explicit limitations as applicable.

## Core and operational authority

Owns task/candidate/requirement/decision/principal identities, contract validation,
policy decisions, tool grants, resource budgets, and guarded domain transitions
([CORE](../product/capability-catalog.md#cap-core-001)). Inputs are declared intent,
actual candidate observations, policy and required external/operator approvals.
Outputs are validated contracts and scoped allow/block decisions, never authority
derived from model text. Rejections identify violated conditions. Enforcement
consumers must check the applicable decision at the actual invocation boundary.
Core remains independent of storage, CLI, agents, and providers. It owns the
applicable grant; harness/tool consumers enforce it at the actual effect boundary
under CAP-CORE-003, with no second policy authority.

## Evidence and assessment

Owns observation/claim/assessment distinctions, provenance requirements, result,
applicability, validity and inspection/interchange semantics
([EVD](../product/capability-catalog.md#cap-evd-001)). Receives captured reports and
their mechanisms/inputs/configuration/versions/time, not claims of success as
trusted commands. Produces scoped judgments with reasons and limitations; it
cannot approve an operation. Dependency changes invalidate consumers without
rewriting observations. Actual capture adapters and evaluators remain identifiable.
Producers declare verification context/coverage under INV-004; Evidence owns validity interpretation,
while knowledge owns the extraction dependency declarations in CAP-KNW-001.
The existing [002-C in-memory model](unified-evidence.md) adds independent evidence
dimensions and direct contextual invalidation alongside the preserved foundation
API. Transitive dependency invalidation, evidence persistence and interchange remain
planned. [002-D](cli-check.md) integrates the bounded task/evidence path in the CLI
without transferring assessment or binding ownership out of their crates.

## Repository intelligence and unified knowledge index

Owns actual repository observation adapters and the derived code/document/schema/
artifact graph, deterministic incremental extraction, queries, conflicts, impact,
and retrieval ([KNW](../product/capability-catalog.md#cap-knw-001), with initial Git
inspection in [CAP-CLI-002](../product/capability-catalog.md#cap-cli-002)). Repository
observation reports source identity/state; core binds it to a contract. Extraction
produces typed spans/relations with separate origin and validation disposition.
The [first reference slice](knowledge-references.md) implements one committed JSON
source adapter and immutable Core model; full extraction/indexing remains incomplete.
Canonical sources own their records; this subsystem owns only rebuildable
projections and dependency indexes. It supplies shared context to memory,
retrieval and prompting rather than proliferating independent indexes.

## Memory and continuity

Owns canonical requirement/decision/learning history, supersession, memory-layer
boundaries, historical recovery and portable handoff composition
([MEM](../product/capability-catalog.md#cap-mem-001)). Uses canonical sources and
the shared index to recover applicable context; does not own a competing search
store. A handoff includes evidence and source maps under disclosure policy.
Consumers verify integrity/candidate correspondence; stale or missing context
does not grant privileges. Record owners and security/privacy jointly handle
correction, retention, and authorized deletion propagation.
The first [canonical-history adapter](0002-durable-history.md) persists requirements,
decisions and their exact candidate context without refreshing observations or
storing assessments. It provides whole-collection removal; broader recovery and
selective lifecycle operations remain planned.

## Skills and registry

Owns typed skill declarations, introspection, exact semantic versions/immutable
references, composition guards and definition provenance
([SKL](../product/capability-catalog.md#cap-skl-001)). Receives internal/operator/
authorized external definitions as lower-trust inputs. Discovery returns
definitions without execution. Invocation requires core authority and harness
enforcement; a skill's requested tools/budgets are not grants. Stable promotion
requires behavioral evidence and evolution gates. The registry cannot enlarge
permissions through an import or metadata update.

## Workflows and engineering methods

Owns inspectable durable state machines, transitions, guards, typed blocked
reasons, specialization/method profiles, composition and safe continuation
([WFL](../product/capability-catalog.md#cap-wfl-001)). Receives a task/candidate,
applicable policy, selected method and evidence; emits guarded step decisions and
recorded state changes. Executor contracts are replaceable only within declared
capabilities. Persistence/effect reconciliation must make crash/replay limits
explicit. The workflow consumes evidence and permission; it manufactures neither.
CAP-WFL-001 owns durable external-effect intent, uncertainty and reconciliation state. The
harness reports actual dispatch/outcomes against those references, rather than
maintaining competing authoritative effect state.

The planned optional [Plan–Execute–Reconcile method](../product/capability-catalog.md#cap-wfl-003)
owns review and reconciliation around typed contracts and returned handoffs.
[Governed parallel work](../product/capability-catalog.md#cap-wfl-004) owns
dependency-aware decomposition, common base/shared contracts, bounded independent
workspaces, task ownership/prerequisites, stable shared contracts and the guarded
integration schedule. Its integrated candidate has distinct identity and affected
revalidation obligations under CAP-WFL-004. Core/harness still enforce
authority/isolation; integration needs separate approval and affected checks
on the integrated candidate. Branch-local success is not integrated acceptance.

## Harness and controlled execution

Owns the single boundary for Cortex-initiated agent execution, actual host controls,
agent/tool adapters, resource enforcement, interruption/cleanup and trace capture
([HRN](../product/capability-catalog.md#cap-hrn-001)). Receives core-authorized scoped
requests and versioned context/definitions. It must reject unsupported mandatory
controls and preserve evidence on tool failure or exhaustion. Backends report
their real guarantees. Local/external agents and optional MCP enter through this
same boundary. Output is lower-trust observation/proposal data, not a new grant.

## Evolution governance

Owns separately identified change proposals, impact/evaluation plans, dispositions,
active-version changes and authorized rollback/recovery
([EVO](../product/capability-catalog.md#cap-evo-001)). Agents may propose instruction,
memory, tool, skill, adapter or topology changes; submission has no activation
effect. Promotion consumes core gates, sufficient evaluation, and approval by a
party distinct from the proposer plus required external authorization. Recursive
chains retain provenance and cannot change privilege or trusted oracles on their
own. Rejected/deferred history remains inspectable.

## Security and privacy

Owns cross-boundary threat analysis, privacy/retention/deletion requirements,
redaction, executable-input provenance review, and adversarial validation
([SEC](../product/capability-catalog.md#cap-sec-001)). Core and harness own effective
enforcement; each record owner implements relevant retention/deletion. Security
classification and optional scanners provide evidence, not permission. Data can
remain private even in debug output, source maps, errors, and exports. Required
unsupported controls block actions; deletion/scan coverage limits remain visible.

## Evaluation

Owns cases/dataset identities, explicit oracles/baselines, protected held-out
examples, reproducible procedures, resource/cost observations and regression/
comparison reasoning ([EVL](../product/capability-catalog.md#cap-evl-001)). It observes
candidate behavior through authorized execution without redefining authority.
Evolution consumes its evidence through domain contracts; proposers cannot replace
trusted oracles or view protected cases. Model/harness effects are separated where
identifiable. Unrun evaluations and confounding are explicit, never optimistic
improvement claims. CAP-EVL-001 owns complete attempt/population accounting and protected
judgment material outside the evaluated agent/harness's authority; CAP-EVL-002
owns controlled comparisons, without changing Core's authority role.

## Context and interoperability

Owns versioned context assembly, relevance/limit accounting, source maps, portable
context packages and executor/protocol translation
([CTX](../product/capability-catalog.md#cap-ctx-001)). It consumes the unified index
and authorized memory sources, retaining temporal provenance and visible omissions.
It negotiates capabilities with actual adapters and cannot turn translated text
into authorization. Missing optional integrations have explicit failure/baseline
behavior. Memory owns the broader task handoff; context packages preserve that
ownership instead of creating a second history.

## CLI and delivery

Owns human/machine presentation, input/configuration diagnostics, reliable exits,
local development checks, minimal-permission CI configuration and eventual release
preparation ([CLI](../product/capability-catalog.md#cap-cli-001)). It integrates
subsystems without redefining their statuses or granting execution authority.
Inspection must reveal dirty candidates, stale evidence and blocked prerequisites.
Display success is distinct from task acceptance. Delivery preparation does not
authorize hosted jobs, public access, packages, or release actions.

## Shared interaction obligations

Before an effect, the consumer checks identity/candidate, policy/approval scope,
required capabilities and limits. After an attempt, it preserves the mechanism,
inputs/configuration/version/time, actual observation, completion/interruption
state and permitted, redacted diagnostics under the applicable capture/omission policy.
Assessments and state transitions reference those records; history/index consumers retain dependencies and temporal meaning.

For example, a recovered decision can help assemble context for a task, but a
stale dependency prevents treating it as current evidence. A workflow may propose
a tool step; core must authorize it and the harness must enforce it. A zero-exit
tool result remains an observation until a relevant acceptance assessment exists.
If any prerequisite is absent, the affected operation stops honestly while
independent authorized inspection may continue. Serialization formats, backend
choices, transaction protocols and broader public APIs remain
[open](../decisions/open-decisions.md).

## Maintainability ownership

Each subsystem owns the cohesion, public invariants, tests and documentation of
its changes under the canonical [engineering requirements](../engineering-principles.md#maintainability-and-contributor-experience).
CLI/delivery coordinates setup and verification instructions and future contributor
walkthrough acceptance in CAP-CLI-006. It also coordinates proportional progressive
dogfooding with current CAP-CLI-005 tooling; workflow automation and benefit
measurement remain with CAP-WFL-002 and CAP-EVL-002. These are engineering/acceptance
responsibilities, not a new subsystem, contribution policy or publication grant.
