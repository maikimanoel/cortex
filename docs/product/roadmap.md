# Architecture roadmap

This document sequences the [canonical capability register](capability-catalog.md).
It does not authorize implementation or restate each functional contract. Milestone
exits require the complete scoped contracts and separately demonstrated advanced
properties, not merely similarly named modules or a successful command. The
[invariants](../architecture/core-invariants.md) apply throughout. Numeric limits,
host coverage, formats, and policies remain [open decisions](../decisions/open-decisions.md)
until their decision point.

## Readiness vocabulary and current position

`DELIVERED` describes an inspected milestone artifact, not universal readiness.
`PLANNED` describes future approved scope; it grants no authority to execute it.
An increment can be implemented while required verification is still `NOT_RUN`,
`BLOCKED`, or insufficient. Assess result and validity independently; record exact
candidate/environment and missing gates in each handoff. A milestone is complete
only when all its required increments and exit evidence are satisfied.

| Milestone | Current readiness | Dependency | Exit evidence |
| --- | --- | --- | --- |
| 001 — Repository Foundation | DELIVERED: Cortex source foundation; local verification and private delivery require candidate-scoped handoff evidence; remote CI remains NOT_RUN until authorized execution | None | Actual three-crate workspace, CLI behavior, minimal contract tests, private remote, configured manual CI |
| 001A — Product Blueprint | Documentation artifact defined here; checkpoint acceptance/commit/push must be established in its handoff | 001 | Complete catalog, consistent architecture/roadmap/decisions, link and scope audits, local checks, verified private documentation commit/push |
| 002 — First Vertical Slice | IN_PROGRESS: 002-A/B delivered; 002-C/D remain PLANNED | 001A | All 002-A/B/C/D exits; real task-to-candidate-to-evidence-to-inspection end-to-end tests and honest failure/unknown cases |
| 003 — Temporal Knowledge & Memory | PLANNED | 002; privacy/storage/coverage decisions | Graph extraction/query/conflict/invalidation, memory supersession/time/staleness, retrieval/context and portable handoff correctness |
| 004 — Typed Skills, Workflows & Harness | PLANNED | 003; authority, profile, adapter and budget decisions | Typed composition/guards, durable continuation, effective host isolation, real authorized/denied invocations, traces and recovery |
| 005 — Governed Evolution & Evaluation | PLANNED | 004; evaluation decisions | Reproducible evaluation and regression evidence, audited proposal dispositions, no self-approval or recursive privilege expansion |
| 006 — Unified Integration | PLANNED | 005 and all scheduled subsystem contracts | End-to-end evidence/authority/temporal coherence, exports, actual interoperability/portability and security validation with no bypass paths |
| 007 — Public Readiness | PLANNED; publication is separately gated | 006 plus owner decisions | Reviewed release candidate, licensing/history/privacy/distribution decisions, preparation evidence; separate explicit owner launch authorization |

## 001 and 001A: preserve the foundation

001 delivers [CAP-CORE-001](capability-catalog.md#cap-core-001),
[CAP-EVD-001](capability-catalog.md#cap-evd-001),
[CAP-CLI-001](capability-catalog.md#cap-cli-001), and
[CAP-CLI-005](capability-catalog.md#cap-cli-005). The
[accepted decision](../architecture/0001-foundation.md) retains its original scope.
Each candidate must be inspected and checked with fresh observations; a result
from another tree cannot establish current execution. Test sources are mechanisms,
not run logs. Workflow configuration is not remote CI evidence and does not
authorize dispatch.

001A establishes product intent, capability IDs, shared invariants, logical
ownership, sequence, and unresolved decisions. It must change documentation only,
preserve accepted choices, and make no behavioral implementation or new readiness
claim. The exact audit, local verification, and Git outcomes belong in its handoff.

Deliver genuinely useful, verifiable increments as their contracts mature. Cortex
may help build Cortex only within demonstrated capabilities and actual grants.
Neither progressive use nor an optional workflow makes unfinished milestone
exits complete. 002-A supplies bounded repository inspection and 002-B adds typed
candidate/task binding. The next implementation is 002-C under separate authority,
after resolving DEC-010 and the applicable output/provenance contract.

## 002: first complete vertical slice

Execute only after separate authorization. The slice is task contract ->
deterministic actual-candidate processing -> unified evidence -> inspection.

| Increment | Scope and capability | Prerequisites | Exit evidence |
| --- | --- | --- | --- |
| 002-A — DELIVERED | Read-only actual Git inspection, [CAP-CLI-002](capability-catalog.md#cap-cli-002) | DEC-005/011 for bounded supported scope | Real repositories resolve HEAD/revision and separate staged/unstaged/untracked state; clean/dirty/detached/unborn and invalid/unsupported/limited cases; no repository mutation |
| 002-B — DELIVERED | Bind inspected candidate to task, [CAP-CORE-002](capability-catalog.md#cap-core-002) | 002-A, foundation contracts | Actual object/candidate correspondence, contract identity, explicit incomplete/dirty state, mismatch/stale-observation rejection |
| 002-C | Unified observation/assessment, [CAP-EVD-002](capability-catalog.md#cap-evd-002) | 002-B, DEC-010 and relevant DEC-007 output semantics | Provenance/time/environment/component versions, separate result/applicability/validity, no pass from missing evidence, honest unknown/blocked/not-run cases |
| 002-D | CLI integration, [CAP-CLI-003](capability-catalog.md#cap-cli-003) | 002-A/B/C, minimum output contract in DEC-007 | Complete real-candidate task/evidence/inspection path, human/machine outputs, reliable exits, failure and privacy tests |

`cortex inspect` alone completes at most 002-A. Do not declare 002 complete while
task binding, semantic evidence, or end-to-end inspection is missing. No agents,
indexing, persistent memory, database, or workflow runner is needed in this slice.
The implemented [002-A contract](../architecture/repository-inspection.md) settles
bounds, supported repository forms, HEAD selection, path handling and observed
concurrent-change reporting. The [002-B contract](../architecture/candidate-binding.md)
adds explicit task revisions, typed observation comparison and qualified binding,
without changing evidence semantics or creating a binding CLI command. Their local
acceptance and private delivery observations belong to candidate-scoped external
handoffs; remote CI remains NOT_RUN.

## 003: temporal knowledge and memory

These increments combine into one coherent subsystem milestone; neither a parser
demo nor an append-only log alone satisfies the exit.

| Increment | Scope/capabilities | Dependency and exit evidence |
| --- | --- | --- |
| 003-A | Canonical history/layers [CAP-MEM-001](capability-catalog.md#cap-mem-001), privacy gate [CAP-SEC-002](capability-catalog.md#cap-sec-002), snapshot extension [CAP-CORE-002](capability-catalog.md#cap-core-002) | 002, DEC-004/007/011; identity, supersession, bitemporal boundary and layer-retention tests before persistence |
| 003-B | Unified derived index/extractors [CAP-KNW-001](capability-catalog.md#cap-knw-001) | 003-A, declared adapter coverage; multi-language/code/document/schema/artifact fixtures, spans/relations, incremental versus rebuild agreement, explicit unsupported coverage |
| 003-C | Temporal queries/conflicts/impact [CAP-KNW-002](capability-catalog.md#cap-knw-002), invalidation [CAP-EVD-003](capability-catalog.md#cap-evd-003), historical recovery [CAP-MEM-002](capability-catalog.md#cap-mem-002) | 003-A/B; origin/disposition separation, historical and late-correction queries, conflict reconciliation, transitive invalidation and justified non-impact |
| 003-D | Local retrieval [CAP-KNW-003](capability-catalog.md#cap-knw-003), recovery integration CAP-MEM-002 | 003-C; declared relevance/negative cases, bounded results, no unauthorized/stale-as-current material, one shared index |
| 003-E | Handoffs [CAP-MEM-003](capability-catalog.md#cap-mem-003), privacy behavior CAP-SEC-002, CLI surfaces [CAP-CLI-004](capability-catalog.md#cap-cli-004) | 003-C/D; portable round-trip, source map, integrity rejection, candidate mismatch -> STALE, redaction and authorized deletion propagation |
| 003-F | Context assembly [CAP-CTX-001](capability-catalog.md#cap-ctx-001), CLI surfaces CAP-CLI-004 | 003-D/E; versioned source selection, provenance, resource/token limits, visible omissions/staleness, no authority from recovered context |

Exit requires all these contracts, including historical validity, conflicts,
memory corrections, privacy limits, and independent inspection of exports. Storage
choices must support the semantics; they are not prerequisites imposed by this
blueprint. Optional embedding acceleration is separately retained in the backlog.

003-E/003-F also establish model-agnostic Portable Context Package transport and
event-driven continuity under [CAP-MEM-002](capability-catalog.md#cap-mem-002),
[CAP-MEM-003](capability-catalog.md#cap-mem-003), and
[CAP-CTX-002](capability-catalog.md#cap-ctx-002). Manual exchange must work without
a repository connector. Verify project/candidate identity, integrity, privacy and
freshness; a snapshot-only recipient cannot assert live repository verification.
Canonical events survive chat-window limits, and compaction/retrieval must retain
material constraints or declare missing/stale context without guessed token thresholds.

## 004: typed skills, workflows, and effective execution

| Increment | Scope/capabilities | Dependency and exit evidence |
| --- | --- | --- |
| 004-A | Policy/typed identities [CAP-CORE-003](capability-catalog.md#cap-core-003), trust analysis [CAP-SEC-001](capability-catalog.md#cap-sec-001) | 003 and DEC-005/006/012 as relevant; explicit authority decisions, threat/control cases, no self-grant or candidate mismatch |
| 004-B | Skills/registry [CAP-SKL-001](capability-catalog.md#cap-skl-001), composition/imports [CAP-SKL-002](capability-catalog.md#cap-skl-002), executable provenance [CAP-SEC-003](capability-catalog.md#cap-sec-003) | 004-A; exact immutable versions, discovery without effects, type/runtime guard rejection, behavioral promotion prerequisites |
| 004-C | Durable workflow [CAP-WFL-001](capability-catalog.md#cap-wfl-001), profiles/methods [CAP-WFL-002](capability-catalog.md#cap-wfl-002) | 004-A/B, DEC-007/012; guarded transitions, blocked reasons, crash/replay/resume and effect-reconciliation tests, inspectable executor-independent profiles |
| 004-D | Effective harness [CAP-HRN-001](capability-catalog.md#cap-hrn-001), enforcement CAP-SEC-001 | 004-A/C, selected host/backend and approved budgets; actual isolation/denial/limit/cleanup tests, candidate traces and recoverable typed interruptions |
| 004-E | Real adapters [CAP-HRN-002](capability-catalog.md#cap-hrn-002), translation [CAP-CTX-002](capability-catalog.md#cap-ctx-002), CLI CAP-CLI-004 | 004-D plus 003 context/handoffs; actual negotiated adapter conformance, absent optional integration diagnostics, portable source/limit/evidence meaning |

The milestone exits only when guards work at real boundaries, not merely in a
trait or prompt. Required qualified review is not replaced by another agent.
Unapproved definitions can be inspected/tested only within separately granted
development authority; registration never grants execution. Stable promotion's
full evolution/evaluation gates arrive in 005-C. Optional protocols/backends do
not weaken the delivered boundary or become mandatory services.

Optional [CAP-WFL-003](capability-catalog.md#cap-wfl-003) adds reviewed
Plan–Execute–Reconcile in 004-C/004-E, with complete cross-system validation in
006-C. It supports one agent or heterogeneous executors without a chat-service
dependency. [CAP-WFL-004](capability-catalog.md#cap-wfl-004) adds dependency-aware
parallel work contracts in 004-C, real bounded workspace execution in 004-D, and
guarded integration/revalidation in 006-A/006-C. Shared base/contracts, denied
out-of-scope mutations, semantic/interface conflict cases, and separate promotion
approval are required; worktrees alone prove neither isolation nor compatibility.
Branch-local evidence cannot substitute for checks against the integrated candidate.

## 005: governed evolution and evaluation

| Increment | Scope/capabilities | Dependency and exit evidence |
| --- | --- | --- |
| 005-A | Evaluation contracts [CAP-EVL-001](capability-catalog.md#cap-evl-001) | 004, DEC-008 and authorized budgets; identified datasets/oracles/baselines, hidden/held-out/negative cases, reproducible traces and resource/cost records |
| 005-B | Proposed changes [CAP-EVO-001](capability-catalog.md#cap-evo-001) | 005-A; distinct candidates for all approved proposal categories, impact/evaluation/rollback plans, inspectable rejected/deferred dispositions without activation |
| 005-C | Promotion [CAP-EVO-002](capability-catalog.md#cap-evo-002), regressions [CAP-EVL-002](capability-catalog.md#cap-evl-002), skill promotion CAP-SKL-002 | 005-A/B; proposer/approver separation, external authorization where needed, no recursive privilege/oracle/gate bypass, rollback/recovery, reward-hacking and held-out regression cases |

Exit requires retained audited decisions and real evaluation evidence. Model and
harness contributions are measured separately where identifiable, with limits
reported otherwise. A higher aggregate score cannot override a required failed
security or functional gate. Missing comparisons remain `NOT_RUN`.

005-A/005-C evaluate the optional methods using fair declared baselines, including
time to verified integrated outcome, task quality, token/resource expenditure,
retries, human coordination cost and regressions under
[CAP-EVL-002](capability-catalog.md#cap-evl-002). No universal cost, speed, quality
or superiority claim is justified before actual measurement.

## 006: unified integration

| Increment | Scope/capabilities | Exit evidence |
| --- | --- | --- |
| 006-A | Authority/evidence/temporal coherence across CORE, EVD, MEM, WFL, EVO, SEC | Whole-path candidate and policy correspondence, stale-input invalidation, no approval from data, no self-promotion, privacy propagation and retained audit |
| 006-B | Real host/backend/security/portability coverage for HRN, SEC, EVL | Observed supported matrix, actual path/network/tool/resource denial, replay/recovery and supply-chain checks; unsupported environments explicit |
| 006-C | Unified query/context/handoff/CLI integration for KNW, MEM, CTX, CLI | Composable/history query meaning, portable provenance-preserving exports, context limits, actual cross-adapter compatibility, coherent human/machine diagnostics |
| 006-D | Optional integration boundary validation under HRN/CTX | Tested absence and failure of optional integrations; no bypass, silent authority change, lost evidence, or essential paid dependency. Test selected optional protocols/backends when actually enabled |

All mandatory contracts scheduled through 006 must satisfy their full functional
baseline and the advanced properties claimed for the supported configuration.
Tests must exercise subsystem crossings, including hostile input and failure
paths. Optional integrations not selected remain visibly planned in the backlog;
their behavior cannot be claimed as demonstrated. The local baseline must be
complete without them. No throughput, language-count, compatibility, or superiority
claim follows from milestone numbering.

## 007: public readiness, separate launch authority

Deliver [CAP-CLI-006](capability-catalog.md#cap-cli-006): resolve license, brand,
contribution/IP and release-scope decisions; inspect repository/history/privacy;
prepare distribution, versioning, operation/support documentation and an assessed
release candidate. Complete applicable security/supply-chain and environment
verification, recording limitations. Exit evidence is a reviewed candidate and
the actual required decisions. Public access, tag/release creation, package
publication, and launch each remain subject to explicit separate owner authority.
This roadmap grants none of them.

## Explicit future backlog

These approved options remain `PLANNED`, not silently dropped or represented as
delivered. Scheduling a concrete consumer/experiment requires separate scope and
authorization; no fixed vendor, backend, budget, or public interface is chosen.

| Item | Canonical capability | Dependency/decision point |
| --- | --- | --- |
| Optional embedding acceleration | [CAP-KNW-004](capability-catalog.md#cap-knw-004) | Complete local retrieval and evaluation; after 006; privacy/budget/model decisions and measured benefit |
| Additional real agents, operating systems, and isolation backends, including constrained component runtimes when justified | [CAP-HRN-001](capability-catalog.md#cap-hrn-001), [CAP-HRN-002](capability-catalog.md#cap-hrn-002) | Declared 004/006 baseline first; DEC-006 and threat/host proof for each extension |
| Optional MCP tool/resource/client/server adapters | [CAP-HRN-002](capability-catalog.md#cap-hrn-002), [CAP-CTX-002](capability-catalog.md#cap-ctx-002) | 006-D absence/authority contract; actual interoperability need, version and security tests before enabling |
| Expanded language/document/schema/binary coverage | [CAP-KNW-001](capability-catalog.md#cap-knw-001) | Complete declared multi-language baseline in 003; DEC-011 coverage and incremental-correctness evidence per adapter |
| Broader public API/format compatibility commitments | [CAP-CLI-006](capability-catalog.md#cap-cli-006) and affected owners | DEC-007, actual consumers and migration evidence; never inferred from version numbering |

These extensions cannot excuse missing mandatory baseline coverage in 002–006.
Revisit catalog targets and this backlog together whenever approved scope changes.
