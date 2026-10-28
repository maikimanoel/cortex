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
| `cortex-core` | Foundation contracts, candidate/task binding, canonical temporal records and immutable knowledge projections | Standard library only |
| `cortex-evidence` | Legacy evidence plus unified result/applicability/validity, versioned context and redacted inspection | `cortex-core` |
| `cortex-cli` | Existing commands, Git/JSON knowledge acquisition and explicit encrypted-history library adapter | Core/Evidence, JSON/codec and SQLCipher binding; local Git/util-linux and OpenSSL 3 build prerequisites |

The accepted [0001 foundation decision](0001-foundation.md) is authoritative for
those bounded design choices. Rust 1.99.1, edition 2024,
resolver 3, unpublished packages, and the shared lockfile remain as configured.
The CLI resolves actual Git commits in [002-A inspection](repository-inspection.md)
and supplies typed state to [002-B Core binding](candidate-binding.md). Shared
state types have one definition in Core; Git capture and presentation remain in
the CLI package, exposed through its library adapter as well as the executable.
Core performs no filesystem/process/clock access. [002-C unified evidence](unified-evidence.md)
consumes those observations programmatically, preserving legacy semantics. It adds
independent result/applicability/contextual-validity and caller-reported time;
[002-D CLI integration](cli-check.md) now composes one committed-HEAD criterion.
Policy enforcement, durable workflows,
full knowledge indexing and agent harnesses remain planned. The first
[003-A slice](temporal-records.md) adds in-memory requirement/decision history
without changing the binding or evidence models.
The [first durable slice](0002-durable-history.md) adds a Linux operator-controlled
SQLCipher adapter that reconstructs validated Core history; broader persistence,
privacy lifecycle and recovery features remain planned.
The [first 003-B reference adapter](knowledge-references.md) adds committed JSON
source nodes/spans/local references, borrowed queries and contextual validity.
Core owns the immutable model; CLI owns parsing/Git acquisition. It adds no
knowledge database, inferred/validated relations or agent behavior.
Its [additive source-set interface](knowledge-references.md#additive-source-set-contract)
captures explicitly selected files as one bound batch and projects cross-document
references without changing original per-source targets. Single-source APIs and
all authority/evidence boundaries retain their meanings.

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

The [planned native engineering methods](engineering-methods.md) compose existing
SKL/WFL/Core/Evidence/knowledge owners. Common properties, qualified specializations,
candidate-aware contract proposals and governed method evolution require no second
instruction registry, index or authority. They are not implemented by today's CLI.

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
002-A; `cortex check` completes the bounded first slice by composing 002-B/C/D.
It checks only committed-HEAD equality; it provides no general task execution.

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
Backend, format, transaction and compatibility decisions beyond the
[first canonical store](0002-durable-history.md) remain
[open](../decisions/open-decisions.md). These planned graph responsibilities do
not select another database, graph service, vector store or event-storage schema.

The [003-A temporal-record contract](temporal-records.md) defines the implemented
first in-memory requirement/decision history slice and its separate privacy,
persistence and snapshot gates. Logical memory ownership is implemented cohesively
in Core, with the first qualified adapter in the CLI library. Existing evidence
semantics remain unchanged, and full 003-A is not delivered.

## Foundation reconciliation and extension seams

The current Rust `Verdict` variants are `Pass`, `Fail`, and `Inconclusive`, and the
current `ExecutionObservation` variants describe execution status. There are no
result/applicability/validity dimensions, timestamps, or dependency invalidation
in that legacy model. `Inconclusive` does not encode why a check was unknown,
not run, or blocked. The separate [002-C model](unified-evidence.md) implements
[INV-002](core-invariants.md#inv-002)'s bounded evidence dimensions, caller-reported
times and direct contextual invalidation without inferring legacy metadata.
[DEC-010](../decisions/open-decisions.md#dec-010) records this scoped resolution;
the accepted foundation decision is not rewritten. Transitive dependency tracking,
cross-candidate recovery and interchange remain planned.

Similarly, `CandidateId` still checks object-ID syntax only. 002-A resolves real
commits; 002-B retains contract revisions, local worktree identities, scoped
observations and correspondence decisions. It does not identify dirty bytes or
produce an atomic snapshot. Snapshot identity remains planned for 003-A. Typed
permissions, principals, resource budgets, skills and workflows remain planned
extensions. Logical requirement/decision and immutable version identities are now
provided by the first in-memory temporal-record slice; durable admission and
privacy lifecycle extensions remain scoped by the [temporal contract](temporal-records.md#durable-history-extension-contract-and-first-implementation).

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

The repository remains private under the owner-selected [MIT License](../../LICENSE).
DEC-009 permits preparation for an early pre-alpha source launch; actual visibility
requires separate owner approval after 002-P3 preparation. The bounded CI workflow checks PR/main
and manual requests using included standard Linux capacity under DEC-005. Its
configuration is not evidence of a remote run. Completion of an engineering
milestone does not authorize deployment, paid resources, package publication or
public release.
