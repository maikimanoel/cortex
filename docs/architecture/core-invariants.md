# Core invariants

These are normative product rules for future capabilities. They are not claims
that the foundation enforces every rule. Current limits and future semantic
extensions are separated in [the overview](overview.md). Capability contracts
and their acceptance cases live in [the catalog](../product/capability-catalog.md).

## INV-001

**Authority cannot amplify itself.** A consequential invocation requires a typed,
core-governed decision applicable to its principal, operation, target, candidate,
tools, budgets, and policy version, plus any required operator/external approval.
The host boundary must check and enforce the decision when the action occurs.
A proposal, skill, workflow, harness, recovered memory, or tool response cannot
extend that decision, approve itself, change its evaluator, or authorize a child
with broader effective authority. Expired, absent, ambiguous, or mismatched
authority blocks the affected operation. An external integration cannot bypass
this path because its input arrived through another protocol.

Inspection and discovery do not execute a skill or workflow. Read permission does
not imply write, network, subprocess, or data-export permission. These distinctions
must survive serialization and adapter translation, not only Rust type checking.

## INV-002

**Claims have different kinds and evidence has separate dimensions.** Keep model
proposals/statements, tool observations, inferred claims, assessments, operator
authorizations, and recorded state transitions distinguishable. A process result
is an observation about a mechanism, not automatically an acceptance judgment.

The planned unified assessment vocabulary is:

| Result | Required meaning |
| --- | --- |
| `PASS` | Relevant observations and an explicit oracle/reasoning support the scoped acceptance criterion |
| `FAIL` | Relevant observations demonstrate a scoped violation |
| `UNKNOWN` | An examination occurred but available evidence is insufficient |
| `NOT_RUN` | The verification procedure was not performed |
| `BLOCKED` | A missing or unauthorized prerequisite prevents the required operation/check |
| `N/A` | The governed object/trigger is demonstrably absent, with an applicability reason |

Applicability distinguishes applicable, not applicable, and uncertain cases.
Only demonstrated absence supports `N/A`. Uncertain applicability cannot support
`PASS` or `N/A`. Missing tools, time, access, or evidence are not absence.
Validity is independent: `CURRENT` means the dependency/candidate context remains
applicable; `STALE` means it no longer does. A stored historical `PASS` can be
`STALE`; displaying it as current success is forbidden. No current-readiness
claim can use stale evidence without new justification and verification.

Zero exit, zero relevant tests, model confidence, missing observations, and a
narrative of intended work cannot independently establish `PASS`. Required gates
with missing evidence remain unresolved. A failed command is not erased by a
later retry; retain both observations and the reason for the new assessment.

## INV-003

**A candidate identifies the assessed material.** Distinguish a task identity,
contract/requirement version, repository revision, worktree/snapshot identity,
execution identity, and assessment identity. A syntactically valid Git ID is not
proof that the object exists, is a commit, or corresponds to inspected bytes.
Resolve actual repository objects and retain staged, unstaged, and untracked
state. Dirty `HEAD` is not the complete working candidate. An incomplete or racing
inspection must declare its limits and cannot assert an identical clean snapshot.
Ignoring files for policy reasons must not silently turn partial coverage into
complete candidate identity. Snapshot and concurrency mechanisms remain to be
decided before their implementation.

For parallel work, the shared base and each execution/integrated candidate remain
distinct. Integration changes the assessed material: branch-local `PASS` does not
establish integrated `PASS`; detect semantic/interface conflicts and revalidate
affected gates on the integrated candidate. A Portable Context Package identifies
its project and snapshot. Snapshot reasoning without live repository access must
not be presented as live verification.

## INV-004

**Provenance is universal.** Material knowledge, memory, requirements, decisions,
and observations need identities, source/version references, and temporal/validity
semantics. Evidence identifies its task/criterion, candidate, mechanism and
version, inputs, configuration, environment, relevant component versions,
observation/recording time, observed outcome, evaluator when applicable, and
limitations. Skill and harness execution references are exact immutable versions
when those components participate. Absent components are explicitly inapplicable,
not invented versions. Inferences identify their supporting observations and
derivation mechanism. Hashes support byte correspondence; they do not prove
authorship, permission, semantic truth, or completeness.

## INV-005

**Time and invalidation are explicit.** Separate when a fact applies in the domain
from when it was recorded/observed. Candidate-scoped validity intervals must not
pretend that incomparable source branches form one universal revision clock.
Historical queries state candidate, domain time, and recording-time perspective
as relevant. A later correction must not silently rewrite what was known earlier.
Changes to declared inputs, contracts, mechanisms, versions, or configuration
invalidate dependent conclusions. Unaffected material retains validity only with
a justified dependency/non-impact basis; uncertainty must remain visible.
Recovery of old memory cannot make it current by recall alone.

Canonical events/references preserve constraints across sessions. Chat compaction
or a guessed token threshold cannot determine which accepted requirements survive;
bounded retrieval must expose missing, uncertain or stale context.

## INV-006

**Transitions are guarded and auditable.** Preconditions, postconditions, actor,
candidate, decision, evidence, and resulting state must be inspectable for a
material transition. Rejection cannot leave a partially accepted state.
`proposed -> authorized`, `executed -> accepted`, and `interrupted -> completed`
are forbidden without their respective guards and supporting records. Missing
postconditions/evidence block completion. Restart/replay must preserve recorded
decisions and detect changed dependencies; it must not blindly repeat external
effects or synthesize unobserved outputs. Crash safety and effect reconciliation
require demonstrated behavior before durable workflows are claimed.

## INV-007

**One derived context index, separate authoritative records.** The knowledge
graph/context index is derived and rebuildable. Canonical decisions, requirements,
events, and evidence retain their proper logical ownership and history outside
that index. Memory and context consumers use the unified index with source links;
they do not grow a second competing index. Rebuilds must preserve source identity,
relation origin, validation disposition, and temporal meaning. Inference must
never become extraction merely because someone validates it. Conflicting sources
remain explicit until a recorded reconciliation resolves the scoped conflict.

## INV-008

**Trust is enforced at the boundary.** Files, retrieved resources, prompts,
transcripts, skills, and tool outputs are lower-trust data regardless of channel.
They cannot launder instructions into authority or trusted evaluation evidence.
Least-privilege tool/path/network controls and supported resource limits must be
effective on the host. A trait, prompt, classifier, or scanner is not isolation
or permission. Unsupported controls must be reported; refuse operations whose
required boundary cannot be enforced. Tool/plugin provenance and supply-chain
checks are part of the trust analysis. Diagnostics and traces must redact secrets
and minimize private content while preserving useful failure categories.

## INV-009

**Evolution cannot judge itself into production.** Every proposed harness, skill,
tool, memory, instruction, adapter, or topology change has a separate candidate,
proposer, justification, impact analysis, evaluation plan/results, and disposition.
The proposer cannot approve its own proposal. Recursive chains cannot expand
privileges, disable gates, or unilaterally alter trusted oracles. Promotion needs
core gates and any required independent/external/human authorization; rejection,
deferral, and rollback/recovery remain inspectable. Evaluation data and held-out
cases must be protected against reward hacking and benchmark overfitting.

An executor also cannot self-approve its own consequential integration/promotion.
An assessment or returned handoff is evidence for reconciliation, never a new grant.

## INV-010

**The local baseline remains complete.** Essential functionality cannot require
a paid provider, managed store, subscription, vector service, or hosted runtime.
Optional integrations require explicit opt-in, capability negotiation, and useful
failure diagnostics. Their absence cannot silently replace or remove the complete
operator-controlled implementation. No cost objective overrides security,
correctness, provenance, authorization, or milestone scope. Finite local capacity
and physical costs must be represented honestly.

## INV-011

**Completeness, proportionality, and portability need proof.** Each targeted
capability must satisfy its whole functional contract. Advanced properties are
separate obligations, not substitutes for missing baseline behavior. Select
trivial/standard/critical methods by auditable rules; neither automatic multi-agent
ceremony for trivial work nor bypass of qualified review for critical work is
acceptable. Compatibility is scoped to observed adapter/host configurations.
Performance and comparative claims identify actual procedures, baselines, resource
costs, and limitations. Unperformed evaluations remain `NOT_RUN`.

## INV-012

**History and privacy have explicit owners.** Canonical history is logically
append-only with corrections and supersession links. Transient and ephemeral data
have distinct retention rules. This does not prohibit authorized privacy-driven
physical deletion: deletion must propagate to derived indexes, caches, and exports
under the chosen policy. Retain only authorized non-sensitive audit information;
a tombstone must not recreate the erased payload. Missing historical data remains
an explicit limit, never a fabricated reconstruction. Defaults and backup/export
handling require a privacy decision before persistence is enabled.
