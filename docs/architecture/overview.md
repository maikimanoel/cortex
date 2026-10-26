# Architecture overview

Cortex separates probabilistic interpretation from deterministic operational
control. The [vision](../product/vision.md) owns product intent, the
[catalog](../product/capability-catalog.md) owns capability contracts/status, the
[invariants](core-invariants.md) own shared semantics, and the
[roadmap](../product/roadmap.md) owns sequencing. This overview describes flow and
boundaries; it does not define additional capabilities or freeze public APIs.

## Observed implementation

The Cortex foundation contains these actual crates:

| Crate | Current responsibility | Dependency |
| --- | --- | --- |
| `cortex-core` | Foundation contracts, contract revisions, typed repository state and candidate/task binding | Standard library only |
| `cortex-evidence` | Descriptive provenance, caller-reported execution status, optional terminal assessment | `cortex-core` |
| `cortex-cli` | Help/version, bounded read-only Git inspection, typed Core adapter and errors | `cortex-core`; local Git and util-linux runtime tools |

The accepted [0001 foundation decision](0001-foundation.md) is authoritative for
those bounded design choices. Rust 1.99.1, edition 2024,
resolver 3, unpublished packages, and the shared lockfile remain as configured.
The CLI resolves actual Git commits in [002-A inspection](repository-inspection.md)
and supplies typed state to [002-B Core binding](candidate-binding.md). Shared
state types have one definition in Core; Git capture and presentation remain in
the CLI package, exposed through its library adapter as well as the executable.
Core performs no filesystem/process/clock access. Evidence semantics are unchanged.
Policy enforcement, durable workflows, persistence, knowledge indexing, agent
harnesses and temporal provenance remain planned.

## Logical areas and dependency direction

The future areas in [subsystems](subsystems.md) are responsibilities, not promises
of one crate each. Introduce a crate, adapter, or abstraction only when an actual
consumer and testable boundary justify it. Preserve these directions:

- Foundational contracts and deterministic policy must not depend on the CLI,
  model/agent infrastructure, graph storage, or an external execution provider.
- Evidence may depend on core contracts. Infrastructure supplies observations;
  the domain governs whether those observations support transitions/assessments.
- Repository/index, memory, skill, workflow, evaluation, and harness adapters
  consume shared identity/authority/provenance semantics. They cannot redefine
  those semantics to accept otherwise forbidden work.
- CLI and context compilers are presentation/integration consumers. Neither
  translated prompts nor protocol adapters become a new authority source.

These are logical dependencies, not a mandate for synchronous calls or cyclic
crate references. Evaluation can observe the harness while the promotion control
plane consumes evaluation evidence through contracts; it need not introduce a
mutual implementation dependency.

## Intended control and data flow

```text
operator task + applicable policy/approvals
                  |
         core contract + actual candidate
                  |
      context assembly -> agent proposal (data)
                  |
      guarded workflow / policy decision
                  |
         governed execution boundary -> host controls -> tool/agent
                  |                                      |
         recorded transitions <- assessment <- captured observations
                  |
      canonical history / evidence -> derived context index -> inspection
```

This is the planned agentic path, not an implemented pipeline. Read-only local
inspection is the first smaller path: repository input -> bounded Git observation
-> candidate/contract binding -> evidence -> CLI inspection. It does not require
an agent, network service, or persistent store. `cortex inspect` alone closes only
002-A; the complete first slice also requires 002-B, 002-C, and 002-D.

Before a consequential tool invocation, core policy must check identity, scope,
target, candidate, tools, and budgets plus external/operator authorization where
required. The host boundary enforces the resulting limits and captures violations.
All Cortex-initiated agent execution enters one governed harness boundary;
different adapters/backends cannot create an alternate route around it.

Observations return separately from evaluations. An assessment references the
criterion, actual candidate, inputs, mechanism, environment, time, relevant
versions, and limitations. Required evidence gates govern completion. Failures,
interruptions, missing dependencies, and insufficient evidence remain typed and
visible, including when useful partial observations exist.

The optional [Plan–Execute–Reconcile method](../product/capability-catalog.md#cap-wfl-003)
adds review between free-form intent and the typed task, then returns a
candidate-bound execution handoff for reconciliation. It supports single or
heterogeneous agents without service-specific authority. Optional
[parallel work](../product/capability-catalog.md#cap-wfl-004) decomposes dependencies
under a common candidate/shared contracts and separate bounded workspaces. A
guarded, separately authorized integration step detects semantic/interface
conflicts and reruns affected checks on the integrated candidate. Worktrees may
separate source trees; actual isolation still belongs to host controls. No
executor self-approves its consequential promotion, and branch-local results
cannot establish integrated acceptance. These paths are planned, not implemented.

## Knowledge, memory, and time

Canonical requirements, decisions, learning events, and evidence have their own
logical authority and history. The unified context index derives code structure,
document/schema/artifact relations, and searchable projections of authorized
canonical data. It is rebuildable and shared by memory, retrieval, impact
analysis, context assembly, and handoff tooling. It does not replace authoritative
events or create a second memory-owned search index.

Continuity follows canonical decision, requirement, handoff, and evidence events
and references, not chat-window lifetime or guessed token thresholds. Compaction
and retrieval must preserve material constraints and expose uncertainty/staleness.
An optional Portable Context Package carries task-scoped project/candidate,
requirements/decisions, provenance/evidence, source map and integrity under privacy
and freshness controls. Manual file transfer remains possible without a repository
connector. A recipient with snapshot-only access cannot assert live verification.

Material records carry identity, source provenance, version references, and
domain/recording-time semantics. Candidate-scoped graph intervals, conflict
dispositions, supersession, and dependency invalidation allow explicit historical
queries. Corrections preserve what was previously recorded; authorized physical
deletion is a separate privacy operation whose downstream effects must be handled.
Backend, format, transaction, and compatibility decisions remain
[open](../decisions/open-decisions.md); these responsibilities do not mandate a
database, graph service, vector store, or event-storage schema.

## Foundation reconciliation and extension seams

The current Rust `Verdict` variants are `Pass`, `Fail`, and `Inconclusive`, and the
current `ExecutionObservation` variants describe execution status. There are no
result/applicability/validity dimensions, timestamps, or dependency invalidation.
The planned vocabulary in [INV-002](core-invariants.md#inv-002) is a requirement
for future work, not an alias for today's API. In particular, `Inconclusive` does
not encode why a check was unknown, not run, or blocked. Model extensions must
preserve the original observation and cannot infer a missing distinction or timestamp.
Resolve [DEC-010](../decisions/open-decisions.md#dec-010) before extending that
model in 002-C; do not rewrite the accepted foundation decision retroactively.

Similarly, `CandidateId` still checks object-ID syntax only. 002-A resolves real
commits; 002-B retains contract revisions, local worktree identities, scoped
observations and correspondence decisions. It does not identify dirty bytes or
produce an atomic snapshot. Snapshot identity remains planned for 003-A. Typed
permissions, principals, resource budgets, requirement/decision identities,
skills, and workflows remain planned extensions, not existing core types.

## Trust, deployment, and evolution

Lower-trust inputs stay data across retrieval, memory recovery, agent prompts,
tool responses, skill imports, and optional MCP boundaries. A classifier may
provide defense in depth; core decisions and effective isolation remain the
source of enforcement. Unsupported isolation or adapter capabilities cannot be
hidden by a common Rust interface. Secrets and unnecessary private content must
not flow into traces, diagnostics, context exports, or handoffs.

Essential behavior remains local and operator-controlled. Optional providers,
embedding accelerators, protocols, and isolation backends must declare actual
capabilities and fail diagnostically when absent. Compatibility and improvement
claims are scoped to observed tests/evaluations. Proposed harness changes are
separate candidates; promotion requires gates and an approver distinct from the
proposer, with external authorization where required.

The repository remains private; license and public launch are undecided. CI is
manual-only and its configuration is not evidence of a remote run. Completion of
an engineering milestone does not authorize deployment, paid resources, package
publication, or public release.
