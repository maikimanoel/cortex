# Capability catalog

This is Cortex's authoritative capability register. The [vision](vision.md)
explains purpose; the [roadmap](roadmap.md) assigns increments; the
[core invariants](../architecture/core-invariants.md) define shared semantics.
Logical owners are described in [subsystems](../architecture/subsystems.md).
All entries inherit INV-001 through INV-012 as target product obligations,
especially enforced authority, mandatory evidence, and temporal provenance.
The implemented foundation subsets do not yet satisfy the full platform
invariants. Entry-specific properties emphasize particular obligations; they do
not exempt any subsystem from the others.

## Reading status and acceptance

| Feature status | Meaning |
| --- | --- |
| `PLANNED` | Approved requirement, no delivered implementation of this contract |
| `IN_PROGRESS` | Authorized implementation started; contract is not complete |
| `IMPLEMENTED` | Inspected code implements the stated bounded contract; this is not a verification result |
| `VERIFIED` | Complete contract has candidate/environment-scoped executed acceptance evidence linked here |
| `DEFERRED` | Explicit postponement with a decision, reason, and revisit point |

Feature status is separate from assessment result and validity in
[INV-002](../architecture/core-invariants.md#inv-002). No capability is promoted
to `VERIFIED` merely because types compile. The `IMPLEMENTED` entries below
refer to the current Cortex source and behavioral test mechanisms. Links to test
source are not run logs. Re-establish observations against each actual candidate;
candidate-specific execution results belong in its handoff/evidence. A result
from another candidate cannot verify the current tree or establish remote CI pass.
No future subsystem has execution evidence; its intended acceptance procedures
are `NOT_RUN`. No capability is silently deferred.

Functional contracts below are the complete required behavior for the named
capability. Advanced properties extend that baseline and need separate proof.
Inputs/outputs are conceptual contracts, not frozen Rust signatures or formats.
Failures use structured categories and the common result/validity rules where
applicable; exact schemas, thresholds, and numeric exit codes need their scheduled
decision. Plans and dependencies do not authorize execution or spending.

## A. Core, task contracts, and operational authority

### CAP-CORE-001

**Name:** Foundation task contracts and source-revision syntax.
**Status:** IMPLEMENTED.
**Owner:** Core. **Target:** 001. **Dependencies:** None.

**Functional contract:** Construct a validated task name, one objective/acceptance
pair, and an independent full Git revision identifier. Reject blank, malformed,
control-bearing, or oversized inputs using typed errors without echoing payloads.
Preserve accepted values. Bounds and syntax are defined in the accepted
[foundation decision](../architecture/0001-foundation.md#domain-contracts).
No object existence, uniqueness registry, permission, or execution is implied.

**Advanced properties:** Immutable values support candidate/contract separation
(INV-003). Actual source binding and operational authority belong to the next
two capabilities; they are not delivered by these types.

**Acceptance:** Valid names and full 40/64-digit lowercase revisions survive
construction; abbreviations, invalid characters, blank criteria, and byte-limit
overflows fail with the expected category. Unicode descriptions count bytes.
**Verification:** [Source](../../crates/cortex-core/src/lib.rs) and
[behavioral tests](../../crates/cortex-core/tests/contracts.rs); current run
results require a candidate-scoped handoff. No open foundation API promise.

### CAP-CORE-002

**Name:** Actual candidate and task binding.
**Status:** IMPLEMENTED.
**Owner:** Core with repository inspection. **Target:** 002-B; snapshot extensions
in 003-A. **Dependencies:** CAP-CORE-001, CAP-CLI-002.

**Functional contract:** Given a task contract and an actual repository observation,
bind the task/criterion version to resolved revision and inspected worktree state.
Keep task, revision, and candidate/snapshot identities distinct. Reject missing
objects, wrong object types, mismatched sources, or assertions of completeness
that the inspection cannot support. A dirty or incomplete observation must not
be represented by `HEAD` alone as a complete working candidate.

**Advanced properties:** Dependency/version references and subsequent snapshot
identity must support temporal invalidation and historical correspondence
(INV-003/004/005). Snapshot coverage and concurrent-change guarantees are explicit.

**Acceptance:** Bind a real clean checkout to its commit and task; a staged-only,
unstaged-only, or untracked change remains distinguishable from that clean
candidate. Reject stale/mismatched observations and nonexistent commit claims.
**Bounded implementation:** Core validates explicit contract revisions, local
repository/worktree identities and typed observations, then compares a baseline
with a separately ordered reobservation. Results distinguish scoped compatibility,
incompatibility, staleness, insufficient information and invalid input. The
[002-B contract](../architecture/candidate-binding.md) defines exact applicability,
producer trust, dirty-state handling, comparison material, coverage and limits.
No dirty snapshot or permanent freshness guarantee is supplied.
**Verification:** [Core tests](../../crates/cortex-core/tests/binding.rs) and
[real-Git cross-crate tests](../../crates/cortex-cli/tests/binding.rs), alongside
002-A/foundation regressions. Test source is a mechanism; executed outcomes belong
to the delivered candidate's external handoff. No evidence assessment is created.
**Decisions:** DEC-011 resolved for bounded 002-B binding only; dirty snapshot
identity remains open for 003-A. DEC-010 remains at its pre-002-C evidence-extension
decision point and was not resolved by binding-domain results.

### CAP-CORE-003

**Name:** Typed policy, identities, permissions, and budgets.
**Status:** PLANNED.
**Owner:** Core. **Target:** 004-A, integrated in 006-A.
**Dependencies:** CAP-CORE-002, CAP-EVD-002; enforcement through CAP-HRN-001.

**Functional contract:** Represent requirements, decisions, principals, tool
permissions, and resource budgets with typed identities/contracts. Evaluate an
operation's preconditions, target/candidate scope, applicable policy and required
approvals; return a structured allow/block decision. Guard auditable state changes
and postconditions. An allowed decision is enforceable only through the actual
tool/environment boundary; missing external/human approval blocks the action.

**Advanced properties:** Non-amplifying delegation, versioned policy, temporal
decision provenance, and auditable consumption at invocation (INV-001/004/006/008).
Model proposals, operator authorizations, results, evaluations, and transitions
remain different domain objects.

**Acceptance:** A permitted operation stays inside its scope; a child requesting
another tool, larger budget, changed candidate, or broader path/network access is
blocked before effects. Wrong/absent approval cannot be supplied by tool text.
**Verification:** NOT_RUN; policy boundary/property tests and real host-denial
integration tests. **Decisions:** DEC-005/006/012; no numeric budgets selected.

## B. Unified evidence, assessment, and provenance

### CAP-EVD-001

**Name:** Foundation in-memory evidence consistency.
**Status:** IMPLEMENTED.
**Owner:** Evidence. **Target:** 001. **Dependencies:** CAP-CORE-001.

**Functional contract:** Retain a full task contract, syntactic candidate identity,
bounded nonblank mechanism/input/configuration descriptions, reported execution
status, and at most one explicit assessment with rationale. Construction is
unassessed. Pass requires a completed zero-exit report; fail requires a completed
report; inconclusive accepts any report. Reject reassessment or invalid text
without changing the retained record. Reports remain caller assertions.

**Advanced properties:** Observation and judgment are separate, terminal judgments
cannot be relabeled, and diagnostics omit payloads (INV-002/003/006). Structured
time, result/validity, authenticated capture, and invalidation are absent.

**Acceptance:** Exit zero alone leaves no assessment; it can explicitly receive
fail. All 18 observation/verdict pairs obey execution preconditions. Rejected
pass, rationale, or replacement preserves the original value and provenance.
**Verification:** [Source](../../crates/cortex-evidence/src/lib.rs) and
[behavioral tests](../../crates/cortex-evidence/tests/evidence.rs); run evidence
is separate. The [002-C extension](../architecture/unified-evidence.md) is a
separate model; legacy values and `Inconclusive` are not implicitly converted.

### CAP-EVD-002

**Name:** Unified observations and inspectable assessments.
**Status:** IN_PROGRESS; 002-C library contract implemented, 002-D CLI integration planned.
**Owner:** Evidence. **Target:** 002-C/002-D.
**Dependencies:** CAP-CORE-002; extends CAP-EVD-001 without erasing its meaning.

**Functional contract:** Capture and inspect candidate/criterion-bound observations
with mechanism/version, inputs, configuration, environment, timestamp, component
versions where applicable, outcomes, and limitations. Distinguish executable
observations, inferred claims, assessments, and approvals. Represent result,
applicability, and validity separately using INV-002. Export/inspect the basis for
a conclusion without exposing credentials or unnecessary private content.
Missing execution/evidence cannot support pass; unavailable prerequisites and
unperformed checks remain distinguishable.

**Advanced properties:** Independently inspectable provenance and explicit semantic
reasoning (INV-002/004/008). Add exact skill/harness references when those components
participate; do not invent them for local inspection. No authentication claim is
implicit in structured fields or export.

**Acceptance:** A real inspection binds to the correct task/candidate and tool
version/time; absent execution is not `PASS`, an insufficient oracle is `UNKNOWN`,
and a missing prerequisite is `BLOCKED`. A zero-test invocation cannot establish
behavioral acceptance. Redacted exports preserve the useful result and limits.
**Implemented 002-C scope:** The [evidence contract](../architecture/unified-evidence.md)
defines validated caller reports, exact task/candidate/observation/assessment
references, versioned mechanism context and reported observation/recording/assessment
times. Independent result/applicability/contextual-validity checks reject unsupported
success, zero-check acceptance and missing applicability. Known invalidation is
sticky without rewriting terminal results. Full typed inspection and a redacted,
non-versioned library writer expose the basis and limits. No legacy conversion,
runner, authenticated capture, persistence or new CLI command is provided.

**Verification mechanisms:** [Semantic and negative tests](../../crates/cortex-evidence/tests/unified.rs),
[real-Git cross-crate fixtures](../../crates/cortex-evidence/tests/repository_evidence.rs),
redaction/I/O tests and a legacy-conversion compile-fail doctest. Executed results
are candidate-scoped external evidence, not these source links. Full CLI output/exit
integration remains NOT_RUN until 002-D. **Decisions:** DEC-010 resolved for 002-C;
DEC-007 scoped identity/time relationships resolved, CLI output/formats deferred.

### CAP-EVD-003

**Name:** Dependency-aware validity and evidence interchange.
**Status:** PLANNED.
**Owner:** Evidence. **Target:** 003-C, cross-subsystem exits in 006-A.
**Dependencies:** CAP-EVD-002, CAP-KNW-001, CAP-MEM-001.

**Functional contract:** Given a dependency/version change, identify affected
observations and conclusions, mark their applicability to the requested context
stale, retain original results, and explain why. Preserve unaffected validity
only with a justified dependency basis. Import/export and historical inspection
retain identities, dependencies, times, and limitations; malformed or unsupported
versions fail explicitly rather than discarding provenance.

**Advanced properties:** Transitive invalidation, scoped non-impact reasoning,
and independently inspectable history (INV-004/005/007/012).

**Acceptance:** Changing a criterion, tool version, or depended-on file invalidates
its consumers; unrelated material stays current with justification. A historical
pass survives as historical/stale, never as current acceptance. Round-trip export
preserves the result/validity distinction; missing dependencies stay unknown.
**Verification:** NOT_RUN; dependency/property tests and interchange/historical
query integration. **Decisions:** DEC-004/007/010.

## C. Knowledge graph and code intelligence

### CAP-KNW-001

**Name:** Deterministic multi-language extraction and incremental indexing.
**Status:** PLANNED.
**Owner:** Knowledge/index. **Target:** 003-B.
**Dependencies:** CAP-CORE-002, CAP-EVD-002, canonical sources from CAP-MEM-001.

**Functional contract:** Parse multiple declared source languages locally and
deterministically into typed symbols, references, imports, call/dependency
relations, and source spans. Appropriate adapters also handle documentation,
schemas, and analyzable binary artifacts. Material nodes/edges carry source,
candidate, adapter/version, and extraction provenance. Update deterministically
from declared dependencies; remove obsolete relations and invalidate affected
material. Unsupported formats, syntax, unresolved references, and partial coverage
must be explicit, not invented structure or a silent complete index.

**Advanced properties:** One derived, rebuildable context index; relation origin
(`extracted`/`inferred`) stays separate from validation disposition
(`unverified`/`validated`/`rejected`/`conflicted`) (INV-004/005/007).

**Acceptance:** Declared-language fixtures yield correct symbols, spans, imports,
and calls; unsupported constructs report coverage limits. Incremental and clean
rebuilds agree for the same inputs/configuration. A dependency edit invalidates
consumers while justified unaffected entries retain validity. Validating an
inferred edge never changes its origin to extracted.
**Verification:** NOT_RUN; parser fixtures, deterministic rebuild comparisons,
negative coverage and invalidation tests. **Decisions:** DEC-007/011.

### CAP-KNW-002

**Name:** Temporal queries, conflicts, impact, and graph interchange.
**Status:** PLANNED.
**Owner:** Knowledge/index. **Target:** 003-C.
**Dependencies:** CAP-KNW-001, CAP-MEM-001; coordinates with CAP-EVD-003.

**Functional contract:** Provide native composable Rust queries for exact symbols,
graph traversals, and filters by provenance, confidence where meaningful,
candidate, and validity. Support candidate-scoped validity intervals and historical
`as-of` queries. Expose conflicting relations and recorded reconciliation without
silent source preference. Impact output distinguishes observed structure from
inferred hypotheses. Import/export retains typed relations and temporal meaning;
invalid intervals, missing dependencies, and incompatible exports are explicit.

**Advanced properties:** Separate domain and recording time, branch-aware history,
explainable query paths, and stable origin/disposition meaning (INV-004/005/007).
Confidence does not grant truth or authority. No query language/API is frozen.

**Acceptance:** An old-candidate query returns its applicable relations after a
new candidate changes them. A late correction changes the appropriate temporal
view without erasing the earlier recorded view. Conflicting extracted/inferred
claims remain visible until reconciled; export/import preserves both. Impact
results identify hypothesis edges and their supports.
**Verification:** NOT_RUN; temporal/conflict query fixtures, traversal property
tests, and interchange checks. **Decisions:** DEC-007/011.

### CAP-KNW-003

**Name:** Local full-text and hybrid retrieval.
**Status:** PLANNED.
**Owner:** Knowledge/index. **Target:** 003-D.
**Dependencies:** CAP-KNW-001/002, CAP-SEC-002.

**Functional contract:** Retrieve authorized local code/document/history context
using full-text, exact-symbol, and graph signals where useful. Return explainable
source spans, candidate, provenance, and validity with bounded results. Missing,
inaccessible, or stale material is explicit; absence of a match is not proof that
a fact does not exist. No remote graph/vector service is needed.

**Advanced properties:** Reproducible retrieval inputs/ranking configuration and
privacy-aware filtering using the unified index (INV-004/005/007/010).

**Acceptance:** Known symbol/text cases recover relevant authorized spans; removed
or denied material does not appear as current accessible context. Empty queries,
no matches, stale candidates, and result-budget limits have deterministic outcomes.
**Verification:** NOT_RUN; relevance/negative fixtures and bounded retrieval
measurements with declared oracles, not invented accuracy targets.
**Decisions:** DEC-004/007/008; ranking and limits remain open.

### CAP-KNW-004

**Name:** Optional embedding acceleration.
**Status:** PLANNED.
**Owner:** Knowledge/index. **Target:** Explicit future backlog after 006.
**Dependencies:** CAP-KNW-003, CAP-EVL-001/002, CAP-CTX-001.

**Functional contract:** Allow an operator to opt into an embedding-based retrieval
accelerator without requiring a vector-store service or displacing the complete
local retrieval baseline. Declare model/index versions, input handling, resources,
privacy and adapter capabilities; missing/failed integrations produce useful
diagnostics and an explicit permitted baseline path.

**Advanced properties:** Candidate-aware invalidation and measured benefit/cost
with provenance (INV-004/005/010/011).

**Acceptance:** Baseline retrieval works with the accelerator absent. Changed
embedding inputs/model invalidate dependent material; a failed provider neither
leaks private inputs nor fabricates results. A benefit claim needs reproducible
baseline comparisons. **Verification:** NOT_RUN; optional-adapter failure tests
and retrieval/resource evaluations. **Decisions:** DEC-004/005/006/008; no vendor,
store, model, performance claim, or activation is selected.

## D. Temporal memory, continuity, and handoffs

### CAP-MEM-001

**Name:** Canonical history and memory layers.
**Status:** PLANNED.
**Owner:** Memory/continuity. **Target:** 003-A.
**Dependencies:** CAP-CORE-002, CAP-EVD-002, policy decisions DEC-004/007.

**Functional contract:** Retain logically append-only canonical requirements,
decisions, and learning history with identities, provenance, version references,
explicit corrections/supersession, and domain versus recording times. Distinguish
canonical, transient, and ephemeral layers with chosen retention rules. Reject
ambiguous identities/temporal contradictions and expose unavailable history.

**Advanced properties:** Bitemporal queries and privacy-compatible deletion without
silently rewriting retained history (INV-004/005/007/012). Canonical memory is an
authority for its records, never a source of new operational permission.

**Acceptance:** Superseding a decision preserves both identities and applicable
times; querying before/after a late recording gives the appropriate perspective.
Transient/ephemeral retention does not silently delete required canonical facts.
**Verification:** NOT_RUN; supersession, temporal-boundary, invalid-event and
retention tests. **Decisions:** DEC-004/007 before enabling persistence.

### CAP-MEM-002

**Name:** Version-aware recovery and obsolete-context detection.
**Status:** PLANNED.
**Owner:** Memory/continuity. **Target:** 003-C/003-D; event-driven continuity in 003-F.
**Dependencies:** CAP-MEM-001, CAP-KNW-002/003, CAP-EVD-003.

**Functional contract:** Recover the decisions, task context, and dependencies
applicable to a historical or current candidate. Detect superseded decisions,
stale dependencies, conflicts, and missing records; expose reasons and unresolved
issues. Use the unified context index with canonical source links, not redundant
memory indexing. Recovered text remains data/evidence and cannot grant tools or
approvals. Preserve continuity through canonical decision, requirement, handoff,
and evidence events/references rather than chat-window lifetime or a guessed token
threshold. Compaction and retrieval must retain material constraints and expose
uncertainty, missing context, and staleness; canonical evidence retains its owner.

**Advanced properties:** Explicit historical validity and safe cross-session
continuity (INV-001/004/005/007/012).

**Acceptance:** Resuming against a changed dependency marks old context stale;
recovering an old candidate finds its applicable decision instead of the latest
one. An injected remembered instruction cannot expand authority. Authorized
deletion produces an honest history gap rather than fabricated memory. Closing or
compacting a chat session cannot lose accepted constraints or make stale context
current; missing material must be reported when a bounded recovery cannot include it.
**Verification:** NOT_RUN; historical recovery, staleness, conflict and trust-boundary
integration tests. **Decisions:** DEC-004/007.

### CAP-MEM-003

**Name:** Portable, integrity-checked handoffs.
**Status:** PLANNED.
**Owner:** Memory/continuity. **Target:** 003-E; interoperability validation in 006-C.
**Dependencies:** CAP-MEM-001/002, CAP-EVD-003, CAP-SEC-002.

**Functional contract:** Export a self-contained handoff with project identity,
active requirements/current decisions, task state, candidate identity, evidence,
source map, unresolved issues, and integrity information. Apply disclosure policy
and declare redactions/omissions.
On receipt, verify integrity and candidate correspondence before context use.
Malformed/tampered packages are rejected; candidate/dependency mismatch yields
`STALE`, not silently trusted current context. No package grants execution rights.

**Advanced properties:** Portable temporal meaning, provenance-preserving export,
and inspectability without the producing agent (INV-001/004/005/008/012).

**Acceptance:** A permitted offline transfer reconstructs the included task context;
modified payloads fail integrity checks. A valid package for another candidate
remains stale. Redaction retains declared limits and removes secret payloads.
**Verification:** NOT_RUN; round-trip, tampering, mismatch, missing-source and
privacy cases. **Decisions:** DEC-004/007; no package format is frozen.

## E. Typed skills and capability registry

### CAP-SKL-001

**Name:** Typed skills and inspectable registry.
**Status:** PLANNED.
**Owner:** Skills/registry. **Target:** 004-B.
**Dependencies:** CAP-CORE-003, CAP-EVD-002, CAP-SEC-003.

**Functional contract:** Declare Rust-native typed inputs/outputs, verifiable
pre/postconditions, allowed tools, limits, and required evidence. Discover and
inspect definitions without execution. Resolve exact semantic versions to
immutable execution references with source/provenance. Reject missing versions,
unverifiable contracts, or mutable substitution under an existing reference.

**Advanced properties:** Shared authority and temporal version semantics across
all invocations (INV-001/004/006). A registry entry is a declaration of requested
capabilities, not an operator grant.

**Acceptance:** Listing a skill invokes no tools; an exact version yields the
same definition or an integrity error. A changed implementation cannot replace
that reference silently. Missing evidence requirements prevent stable promotion.
**Verification:** NOT_RUN; registry/discovery integrity and contract tests.
**Decisions:** DEC-003/007; registry representation and public API remain open.

### CAP-SKL-002

**Name:** Guarded skill composition, imports, and promotion.
**Status:** PLANNED.
**Owner:** Skills/registry. **Target:** 004-B, governed promotion in 005-C.
**Dependencies:** CAP-SKL-001, CAP-CORE-003, CAP-SEC-001/003.

**Functional contract:** Compose type-compatible skills at construction where
possible and validate remaining runtime guards. Admit internal, operator, and
authorized external definitions with provenance; adapt external representations
without treating text as authority. Reject unauthorized tools, budget expansion,
absent postconditions/evidence, and incompatible composition. Require behavioral
tests before stable promotion. Skills cannot independently change core policy,
the harness, permissions, or registry.

**Advanced properties:** Non-amplifying composition and immutable versioned traces
(INV-001/004/006/008/009). Promotion uses CAP-EVO-002, not self-certification.

**Acceptance:** Compatible composition carries outputs and evidence correctly;
incompatible types fail construction or a declared runtime guard before effects.
Imported instructions requesting extra permissions are rejected. Untested skills
cannot become stable. **Verification:** NOT_RUN; construction/runtime negative
cases and real unauthorized-tool tests. **Decisions:** DEC-003/005/007.

## F. Typed workflows and engineering methods

### CAP-WFL-001

**Name:** Durable guarded workflows and safe continuation.
**Status:** PLANNED.
**Owner:** Workflows/methods. **Target:** 004-C.
**Dependencies:** CAP-CORE-003, CAP-EVD-003, CAP-MEM-001, CAP-SKL-001/002.

**Functional contract:** Define durable Rust state machines with inspectable steps,
transitions, guards, composition, candidate-bound evidence, and typed blocked
reasons. Validate expected evidence before advancing. Resume after interruption
against applicable context; deterministic replay reproduces recorded decisions
and guarded transitions where supported, without pretending to rerun or observe
external effects. Crash-safe continuation reconciles pending effects or blocks
explicitly rather than blindly duplicating them.

**Advanced properties:** Auditable state history, dependency-aware invalidation,
and executor-independent workflow contracts (INV-004/005/006).

**Acceptance:** Invalid transitions and missing evidence cannot advance state;
interrupt/restart at persistence/effect boundaries neither loses an accepted
transition nor repeats an unreconciled side effect. Changed candidates block
unsafe resumption. **Verification:** NOT_RUN; transition/property tests,
deterministic replay and crash/fault injection with declared budgets.
**Decisions:** DEC-005/007; durability/effect protocol not selected.

### CAP-WFL-002

**Name:** Proportional engineering methods and specialization profiles.
**Status:** PLANNED.
**Owner:** Workflows/methods. **Target:** 004-C; integrated evaluation in 005/006.
**Dependencies:** CAP-WFL-001, CAP-CORE-003, DEC-012.

**Functional contract:** Select trivial/standard/critical profiles by auditable
rules. Define typed agent specializations with scope, tools, authority, and limits,
independent of executor implementation. Provide methods spanning discovery,
specification, planning, implementation, debugging, testing, review, security,
reliability, and delivery. Specification-first and test-driven methods are available
when suitable. Inspect states, guards, and evidence expectations before running.
Unsupported classification or missing qualified review blocks the affected path.

**Advanced properties:** Proportionality with no bypass of contract-required review,
explicit specialization boundaries, and evidence-backed method selection
(INV-001/004/006/011).

**Acceptance:** A trivial task need not spawn agents or formal analysis; a critical
task cannot skip its required qualified review. A specialist cannot invoke tools
outside its grant. Workflow inspection has no execution effects.
**Verification:** NOT_RUN; profile/guard cases and complete representative methods.
**Decisions:** DEC-006/012; no risk thresholds or reviewer policy invented here.

### CAP-WFL-003

**Name:** Optional Plan–Execute–Reconcile method.
**Status:** PLANNED.
**Owner:** Workflows/methods. **Target:** 004-C method, 004-E execution/adapters,
complete cross-subsystem validation in 006-C.
**Dependencies:** CAP-WFL-001/002, CAP-CORE-002/003, CAP-EVD-002/003,
CAP-MEM-003, CAP-CTX-002, CAP-HRN-001/002.

**Functional contract:** Turn freely expressed intent into a proposed typed task
contract for review, with explicit acceptance, candidate, dependencies, authority
requirements, and limits. The reviewed contract governs execution; neither a
planner's narrative nor a handoff grants permission. Execute only through actual
authority, then return a candidate-bound handoff containing changes, observations,
evidence, limitations and unresolved issues for evidence-based reconciliation.
Support one agent or multiple heterogeneous agents without requiring a particular
chat service, direct repository connector, or mandatory delegation.

**Advanced properties:** Traceable planning/review/execution/reconciliation states,
portable snapshot context, and separation of observation, assessment and approval
(INV-001/003/004/006/009/011). An executor cannot approve its own consequential
promotion. Integration uses CAP-WFL-004 and fresh integrated-candidate evidence.

**Acceptance:** Free-form intent cannot execute before contract review and
authority checks. A single-agent case works without delegation; heterogeneous
execution preserves the same contract and limits. A returned handoff for a changed
candidate becomes stale. Reconciliation records missing evidence or semantic
conflicts instead of treating the executor's completion statement as acceptance.
**Verification:** NOT_RUN; method-state, manual-package, cross-adapter and
approval/candidate mismatch integration cases. **Decisions:** DEC-005/006/007/012;
no service, schema, planner policy, or executor implementation is selected.

### CAP-WFL-004

**Name:** Governed parallel work and integrated-candidate revalidation.
**Status:** PLANNED.
**Owner:** Workflows/methods with core and harness enforcement.
**Target:** 004-C decomposition contracts, 004-D execution controls, 006-A/006-C
guarded integration and complete interoperability validation.
**Dependencies:** CAP-WFL-001/002, CAP-CORE-002/003, CAP-EVD-003,
CAP-HRN-001/002, CAP-EVL-002 for measured evaluation.

**Functional contract:** When parallel work is explicitly authorized, decompose
tasks by declared dependencies under a common base candidate and shared contracts.
Assign independent execution workspaces/branches, bounded mutation scopes,
resource budgets, and integration responsibilities. Detect overlapping changes,
semantic/interface conflicts, stale bases, and missing dependency results.
Integrate in a separately authorized guarded step and rerun affected checks on
the integrated candidate. Preserve branch-local and integrated evidence distinctly;
a branch-local `PASS` does not establish integrated `PASS`.

**Advanced properties:** Non-amplifying delegation, reproducible candidate
correspondence, effective isolation, and evidence-based integration gates
(INV-001/003/004/005/006/008/009). Git worktrees may supply independent trees;
they are not sandboxes or guarantees of semantic compatibility. No executor may
self-approve its own consequential promotion. Parallel execution is optional,
not mandatory ceremony for trivial work.

**Acceptance:** Dependent tasks wait for their prerequisites; out-of-scope writes
are denied by actual controls. Individually passing branches with an incompatible
shared interface fail/block integration until reconciled. A changed base
invalidates affected evidence, and required integrated checks cannot be replaced
by local passes. Missing approval or partial integration remains explicit.
**Verification:** NOT_RUN; dependency/overlap/semantic-conflict fixtures, real
isolated workspace tests, guarded integration and revalidation regressions.
**Decisions:** DEC-005/006/007/012; workspace mechanics and approval policy remain
open, and no parallel executor is implemented by this blueprint.

## G. Isolated agent harness and controlled tool execution

### CAP-HRN-001

**Name:** Governed execution boundary and effective isolation.
**Status:** PLANNED.
**Owner:** Harness/execution. **Target:** 004-D, backend/portability validation in 006-B.
**Dependencies:** CAP-CORE-003, CAP-EVD-003, CAP-SEC-001, approved budgets/host policy.

**Functional contract:** Route every agent execution initiated by Cortex through
one governed entry point. Enforce applicable tools, filesystem/path/network
restrictions, least privilege, and supported CPU, memory, wall-time, operation,
and data-volume limits using real host controls. Capture candidate-bound inputs,
proposals, tools, outputs, decisions, and evidence with redacted export. Timeouts,
resource exhaustion, permission violations, and tool failures produce recoverable
typed interruption with useful partial trace and explicit cleanup limits.

**Advanced properties:** Pluggable isolation backends with demonstrable guarantees;
appropriately constrained component-style runtimes may be used when justified by
threat model/host capability (INV-001/004/006/008). No Rust trait alone is isolation.

**Acceptance:** Attempted path escape, denied network/tool calls, limit overruns,
and child-process escape attempts are actually contained for each supported
backend. Unsupported mandatory controls block execution. Cancellation and cleanup
leave inspectable terminal/interrupted state without secret trace leakage.
**Verification:** NOT_RUN; adversarial real-host tests, resource/fault tests, and
trace privacy checks. **Decisions:** DEC-004/005/006; no backend guarantee assumed.

### CAP-HRN-002

**Name:** Agent adapters and optional protocol interoperability.
**Status:** PLANNED.
**Owner:** Harness/execution. **Target:** 004-E; optional extensions in 006-D/backlog.
**Dependencies:** CAP-HRN-001, CAP-CTX-002, CAP-SEC-001.

**Functional contract:** Provide typed adapters for selected local/external agents,
models, and CLIs. Negotiate/report real capabilities and limitations; translate
requests/results without inventing interchangeable behavior. Unsupported features,
missing optional integrations, and provider failures produce typed diagnostics.
Optional MCP tool/resource/client/server interoperability must pass the same
authority boundary; MCP is not required for core functionality.

**Advanced properties:** Exact adapter/configuration provenance and observed
compatibility across the chosen host matrix (INV-001/004/008/010/011).

**Acceptance:** Real selected adapters satisfy their declared contracts; an adapter
without a required control is refused. A protocol tool cannot bypass policy or
export unauthorized data. The local baseline operates with optional integrations
absent. **Verification:** NOT_RUN; actual adapter conformance, denial and
unavailable-integration cases. **Decisions:** DEC-005/006/007; no agent/provider or
MCP dependency selected, and no paid integration authorized.

## H. Governed recursive harness evolution

### CAP-EVO-001

**Name:** Identified evolution proposals and retained disposition.
**Status:** PLANNED.
**Owner:** Evolution governance. **Target:** 005-B.
**Dependencies:** CAP-CORE-003, CAP-EVD-003, CAP-MEM-001, CAP-EVL-001.

**Functional contract:** Accept proposals to change harness instructions, memory,
tools, skills, adapter configuration, or subagent topology as separately identified
candidates. Retain proposer, justification, source provenance, impact analysis,
evaluation plan/results, and disposition. Support rejection, deferral, and
rollback/recovery plans without applying the proposal during submission.
Malformed, unscoped, or unauthorized proposals fail diagnostically.

**Advanced properties:** Traceable recursive proposal chains and temporal candidate
history (INV-004/005/009). Submission is data, not promotion or execution approval.

**Acceptance:** A proposal is inspectable without altering active configuration;
rejected/deferred candidates retain reasons. Missing rollback/recovery analysis
blocks promotion where its contract requires it. **Verification:** NOT_RUN;
proposal lifecycle, provenance and no-side-effect tests. **Decisions:** DEC-007/008.

### CAP-EVO-002

**Name:** Controlled promotion and recursive privilege preservation.
**Status:** PLANNED.
**Owner:** Evolution governance with core gates. **Target:** 005-C; integration 006-A.
**Dependencies:** CAP-EVO-001, CAP-EVL-001/002, CAP-CORE-003, CAP-HRN-001.

**Functional contract:** Evaluate a proposal against deterministic gates and
sufficient results, separate proposer from approver, and require external/human
authorization where applicable before promotion. Reject chains that enlarge
grants, disable mandatory gates, or unilaterally change trusted oracles. Record
promotion/rejection/deferral and perform authorized rollback/recovery with evidence;
failure cannot silently leave an unrecorded active candidate.

**Advanced properties:** Non-amplifying recursion and experiment-governed change
with auditable active-version history (INV-001/004/006/009/011).

**Acceptance:** Self-approval, indirect privilege expansion through a subproposal,
changed evaluation oracle, missing regression evidence, and unauthorized promotion
are blocked. An authorized rollback identifies the restored candidate and retains
the failed candidate's evidence. **Verification:** NOT_RUN; recursive-chain,
separation-of-duty, promotion-fault and rollback integration tests.
**Decisions:** DEC-005/008/012; qualified approval policy is not invented.

## I. Security, privacy, and trust boundaries

### CAP-SEC-001

**Name:** Trust-boundary enforcement and threat analysis.
**Status:** PLANNED.
**Owner:** Security/privacy, enforcement owned by core/harness.
**Target:** 004-A/004-D; whole-system validation in 006-A/006-B.
**Dependencies:** CAP-CORE-002, CAP-EVD-002; pairs with CAP-CORE-003/CAP-HRN-001.

**Functional contract:** Threat-model lower-trust files, retrieved data, prompts,
transcripts, tool/plugin responses, and skills. Control prompt injection,
tool-output laundering, plugin/skill poisoning, exfiltration, filesystem/permission
violations, and trust changes through policy-enforced invocation and actual
isolation. Classification/detection can add defense in depth but cannot grant
authority. Record denied actions and useful redacted failure diagnostics.

**Advanced properties:** No bypass through retrieval, recovered memory, protocol,
or export path; resource-bounded least privilege (INV-001/004/008).

**Acceptance:** Malicious instructions in each input channel cannot invoke a
denied tool or cross a path/network boundary. A benign-looking classified response
still cannot authorize itself. Unsupported isolation fails closed for the required
operation. **Verification:** NOT_RUN; threat/control map and adversarial host
integration cases. **Decisions:** DEC-005/006/012; no certification is claimed.

### CAP-SEC-002

**Name:** Privacy-aware retention, deletion, and audit.
**Status:** PLANNED.
**Owner:** Security/privacy with canonical record owners.
**Target:** 003-A policy gate/003-E behavior; integration in 006-A.
**Dependencies:** DEC-004, CAP-EVD-002; memory/index owners implement propagation.

**Functional contract:** Apply authorized handling of secrets and personally
sensitive data to capture, diagnostics, retrieval, storage, handoffs, and export.
Separate retention by data layer. Inspect/correct records and, when authorized,
physically delete protected data and dependent copies under declared backup/export
limits. Preserve only permitted non-sensitive audit facts; report incomplete
deletion rather than declaring success. Missing policy blocks persistence/disclosure.

**Advanced properties:** Privacy-compatible logical history and provenance-preserving
redaction without reconstructing erased payloads (INV-004/007/008/012).

**Acceptance:** Secret-bearing fixtures never appear in prohibited output. An
authorized deletion removes canonical payload and derived/cache copies under
scope; a retained tombstone does not disclose it. An inaccessible backup is an
explicit remaining limitation, not a completed deletion claim.
**Verification:** NOT_RUN; retention, correction, deletion-propagation and redaction
integration tests. **Decisions:** DEC-004/007; no retention defaults selected.

### CAP-SEC-003

**Name:** Dependency and executable-definition provenance.
**Status:** PLANNED.
**Owner:** Security/privacy with delivery and skill owners.
**Target:** 004-B, integrated supply-chain review in 006-B/007.
**Dependencies:** CAP-EVD-002, CAP-CORE-003; informed by actual dependency inventory.

**Functional contract:** Inspect origins, versions, integrity, permissions, and
license obligations of introduced dependencies, tools, plugins, and skills.
Reject unauthorized or substituted executable inputs. Optional scanners/detectors
provide evidence with coverage and failure semantics; an unavailable scan is not
a pass and a clean classification is not permission or comprehensive security.

**Advanced properties:** Immutable execution references, temporal invalidation,
and auditable trust decisions (INV-001/004/005/008).

**Acceptance:** A changed skill/tool under a recorded immutable reference is
rejected; an unavailable required check blocks its gate. Scanner output retains
source/version/scope and cannot approve an operation by itself.
**Verification:** NOT_RUN; integrity/substitution, dependency review and scanner
failure cases. **Decisions:** DEC-003/006/007; no scanner/service requirement added.

## J. Evaluation, reproducibility, and measured improvement

### CAP-EVL-001

**Name:** Reproducible evaluation contracts and observations.
**Status:** PLANNED.
**Owner:** Evaluation. **Target:** 005-A.
**Dependencies:** CAP-CORE-003, CAP-EVD-003, CAP-HRN-001, DEC-008.

**Functional contract:** Identify cases, datasets, candidates, explicit oracles,
baselines, hidden/held-out sets, and negative examples. Execute within authorized
budgets and capture versions/configuration, structured traces, cost/resource
records, outcomes and limitations. Reproduction states relevant stochastic and
environmental conditions; unavailable prerequisites or unperformed procedures
remain `BLOCKED`/`NOT_RUN`, not fabricated results.

**Advanced properties:** Independently inspectable evaluation inputs and temporal
validity, with controlled access to reserved cases (INV-004/005/009/011).

**Acceptance:** Reproduce a deterministic case's result from its recorded inputs;
changed candidates/oracles invalidate comparisons. Negative cases discriminate
incorrect behavior. Hidden cases are not exposed to a proposing agent.
**Verification:** NOT_RUN; evaluation reproducibility, oracle-failure, negative
and held-out isolation cases. **Decisions:** DEC-005/008; no score targets selected.

### CAP-EVL-002

**Name:** Regression coverage and defensible improvement claims.
**Status:** PLANNED.
**Owner:** Evaluation. **Target:** 005-C; integrated coverage in 006.
**Dependencies:** CAP-EVL-001; observes capability/harness candidates for CAP-EVO-002.

**Functional contract:** Evaluate capability behavior, security boundaries, replay,
retrieval quality, and task outcomes against explicit baselines. Separate harness
contributions from model contributions where identifiable and disclose confounding
otherwise. Use reserved/held-out and negative cases to detect reward hacking and
benchmark overfitting. Publish comparisons only when actual procedure/results and
the approved comparison policy support the scoped claim. Missing evaluations
remain `NOT_RUN`; regression failures cannot be averaged away. Evaluate optional
delegation/parallel methods under fair declared baselines using time to verified
integrated outcome, task quality, token/resource expenditure, retries, human
coordination cost, and regressions. Local branch completion is not the endpoint
when integration and revalidation remain outstanding.

**Advanced properties:** Candidate-bound, resource-aware empirical promotion and
honest uncertainty (INV-004/005/009/011).

**Acceptance:** An oracle exploit or held-out regression blocks a required gate
despite improved aggregate score. A study changing both model and harness cannot
claim isolated harness benefit without evidence. A parallel run with fast local
completion but failed integrated checks cannot count as a faster verified outcome.
**Verification:** NOT_RUN; adversarial oracle/reward cases and reproducible
controlled comparisons.
**Decisions:** DEC-008/012; no superiority or publication claim exists.

## K. Context, prompting, and interoperability

### CAP-CTX-001

**Name:** Versioned, bounded context assembly.
**Status:** PLANNED.
**Owner:** Context/interoperability. **Target:** 003-F; integrated validation 006-C.
**Dependencies:** CAP-KNW-003, CAP-MEM-002, CAP-SEC-002, CAP-EVD-003.

**Functional contract:** Assemble context from the unified index and authorized
memory sources using explicit task/candidate, relevance, provenance, staleness,
and resource/token limits. Return selected material, source map, and declared
omissions/limits. Reject unauthorized sources; stale, conflicting, missing, or
budget-truncated material is visible. Context never grants operational authority.
Compaction/retrieval preserve material constraints from canonical events and
references, independent of chat-window limits or any guessed token threshold.

**Advanced properties:** Reproducible assembly configuration, temporal dependencies,
and privacy-aware source selection (INV-001/004/005/007/008).

**Acceptance:** Repeated deterministic assembly for fixed inputs is inspectably
consistent; candidate changes invalidate affected context. Over-budget content
has declared truncation, and a retrieved instruction cannot elevate permissions.
**Verification:** NOT_RUN; assembly/source-map, budget, staleness and injection
cases. **Decisions:** DEC-004/005/007; token accounting is adapter-aware.

### CAP-CTX-002

**Name:** Portable context and executor-specific contract translation.
**Status:** PLANNED.
**Owner:** Context/interoperability. **Target:** Package transport in 003-E/003-F,
executor translation in 004-E; cross-adapter exits 006-C.
**Dependencies:** Package transport uses CAP-MEM-003 and CAP-EVD-003; context
assembly integrates CAP-CTX-001 in 003-F. Executor translation additionally
requires CAP-CORE-003 and CAP-SKL-001 in 004.

**Functional contract:** Translate typed task/context contracts into instructions
for heterogeneous executors and protocols. An optional Portable Context Package
provides task-scoped, self-contained, model-agnostic transport with project and
candidate identity, active requirements/decisions, relevant evidence/provenance,
source map, integrity, privacy controls, and freshness validation. Manual file
exchange must work without a direct repository connector or particular chat
service. A recipient without live access may reason about the included snapshot
but cannot assert live repository verification. Negotiate required/optional
capabilities before execution. Preserve limits and evidence requirements outside
the prompt at the real enforcement
boundary. Unsupported translation or absent optional integrations must fail
diagnostically or use an explicitly permitted complete baseline path.

**Advanced properties:** Observable compatibility without prompt-based authority
and exact compiler/adapter version references (INV-001/004/008/010/011).

**Acceptance:** Selected adapters preserve task identity and constraints through
translation; one lacking required semantics is rejected before invocation.
An exported prompt/package cannot authorize extra tools. A manually transferred
package supports snapshot reasoning without a connector; changed project/candidate
or freshness dependencies prevent current/live verification claims. Tampered or
privacy-incompatible packages are rejected. **Verification:** NOT_RUN;
translation conformance, package integrity and actual adapter compatibility cases.
**Decisions:** DEC-006/007; no universal instruction format promised.

## L. CLI, developer experience, and delivery

### CAP-CLI-001

**Name:** Foundation terminal interface.
**Status:** IMPLEMENTED.
**Owner:** CLI/delivery. **Target:** 001. **Dependencies:** None beyond Rust/std.

**Functional contract:** `cortex --help`/`-h` and no arguments print help;
`--version`/`-V` print the package version. Success exits zero, unsupported/extra
arguments exit 2 without echoing their values, and output failures propagate with
exit 1. These foundation forms remain compatible; repository inspection is
specified separately by CAP-CLI-002. No agent or network operation is introduced.

**Advanced properties:** A bounded argument dispatch and private-input-safe
diagnostics (INV-008/011). Stream writes can block; no stream timeout is claimed.

**Acceptance:** Real subprocess help/version outputs and exit statuses match the
interface; extra/non-UTF-8 arguments are rejected without panic where supported.
An output failure preserves its I/O cause. **Verification:**
[Source/unit test](../../crates/cortex-cli/src/main.rs) and
[integration tests](../../crates/cortex-cli/tests/interface.rs); run results are
candidate/environment-scoped. No stable future subcommand API is implied.

### CAP-CLI-002

**Name:** Bounded read-only actual Git inspection.
**Status:** IMPLEMENTED.
**Owner:** Repository inspection with CLI presentation. **Target:** 002-A.
**Dependencies:** CAP-CORE-001; DEC-005/011 resolved for the supported scope.

**Functional contract:** Given a local path and the chosen revision-selection
contract, resolve an actual Git repository and HEAD/commit, and report staged,
unstaged, and untracked worktree state separately. Preserve detached/unborn/missing
revision distinctions. Operate read-only with bounded time/output/operations;
never fetch, checkout, stage, mutate Git metadata, or run user hooks. Report
unsupported forms, permissions, missing Git/object, malformed input, truncation,
or concurrent-change uncertainty explicitly. Dirty HEAD is never labeled as the
complete working candidate. The interface must be inspectable without an agent.

**Advanced properties:** Provenance of the inspection mechanism/configuration and
host, honest candidate coverage, and repeatable observation where inputs remain
stable (INV-003/004/008/011). Atomic snapshots are not assumed.

**Acceptance:** Temporary real repositories distinguish clean, staged-only,
unstaged-only, untracked-only, mixed, detached, and unborn cases. Invalid paths,
missing revisions, denied access, nonstandard filenames, and configured limit
failures have documented outcomes. Before/after comparison shows no repository
mutation. Concurrent edits cannot produce an unsupported clean-candidate claim.
**Bounded implementation:** `cortex inspect [--repo <PATH>]` on Linux, with local
Git and util-linux `prlimit`; HEAD selection, typed private observations, two-sample
comparison, finite process/capture/entry/address-space budgets, and explicit
coverage/error semantics. See the [inspection contract](../architecture/repository-inspection.md)
for supported forms, exclusions, read-only controls and concurrency limitations.
**Verification:** [Implementation](../../crates/cortex-cli/src/inspection/mod.rs),
[real-Git CLI tests](../../crates/cortex-cli/tests/inspection.rs), and bounded
parser/process tests in the inspection module. Executed acceptance is recorded
against the delivered candidate in its external handoff; test source alone is not
run evidence. No remote CI result or full snapshot/candidate identity is claimed.
**Decisions:** DEC-005/011 resolved for 002-A only in the inspection contract;
runner budgets, snapshot identity and later extraction remain open.

### CAP-CLI-003

**Name:** First complete task-to-evidence inspection path.
**Status:** PLANNED.
**Owner:** CLI/delivery integrating core/evidence. **Target:** 002-D.
**Dependencies:** CAP-CLI-002, CAP-CORE-002, CAP-EVD-002.

**Functional contract:** Present a task contract, deterministic actual-candidate
processing, unified observations/assessments, and inspection of their basis through
the CLI. Supply human-usable and machine-consumable output with reliable exit
behavior, explicit errors/unknowns, and provenance. Missing execution or a dirty
candidate cannot be hidden by a successful display command. No agent, database,
or persistent workflow is required to complete this slice.

**Advanced properties:** One coherent identity/evidence path with redaction and
candidate correspondence (INV-002/003/004/008). Display success is separate from
the assessed task result.

**Acceptance:** End-to-end fixtures begin with a real repository and task, produce
candidate-bound evidence, and inspect it without relabeling. Mismatch, absent
prerequisite, insufficient evidence, and invalid inputs remain distinguishable.
**Verification:** NOT_RUN; full CLI/core/evidence integration, output and exit
contract tests. **Decisions:** DEC-007/010; 002-A alone does not satisfy this entry.

### CAP-CLI-004

**Name:** Coherent developer operations and portable configuration.
**Status:** PLANNED.
**Owner:** CLI/delivery. **Target:** 003-E/003-F and 004 inspection surfaces;
complete integration in 006-C. **Dependencies:** CAP-CLI-003 and the exposed
memory/context/skill/workflow/harness contracts.

**Functional contract:** Expose task/evidence viewing, diagnostics, context and
handoff operations, pre-execution skill/workflow inspection, and later authorized
workflow execution through a coherent local-first CLI. Provide human/machine
outputs, typed error semantics, bounded operations, portable explicit configuration,
and reliable exits. Invalid or incompatible configuration cannot silently expand
authority or select an optional provider.

**Advanced properties:** Consistent candidate/time/validity semantics across all
surfaces and observed host/adapter compatibility (INV-001/004/005/010/011).

**Acceptance:** Export/import context and handoffs retain meaning; diagnostics
show stale/blocked state and limitations. Inspecting workflow/skill definitions
has no effects. Execution uses the same policy boundary as all other callers.
**Verification:** NOT_RUN; CLI integration and actual supported-host/configuration
tests. **Decisions:** DEC-005/006/007; no complete command tree selected now.

### CAP-CLI-005

**Name:** Foundation development and manual CI configuration.
**Status:** IMPLEMENTED.
**Owner:** CLI/delivery. **Target:** 001, maintained in every increment.
**Dependencies:** The three workspace crates and installed pinned toolchain.

**Functional contract:** Provide a shared Cargo workspace/lockfile, pinned Rust
toolchain, unpublished packages, documented local verification commands, and a
manual-only read-only CI definition with a bounded job and pinned checkout.
No automatic job, package publication, license, or release is enabled.

**Advanced properties:** Controlled build inputs and least-privilege CI
(INV-008/010/011). This is neither byte-identical artifact proof nor remote CI pass.

**Acceptance:** Workspace metadata agrees with the crate graph; local formatting,
compilation, lint and behavioral checks actually run against the candidate. CI
configuration has only `workflow_dispatch`, read-only permissions, and no persisted
checkout credentials. Remote success requires a separately authorized actual run.
**Verification:** [Workspace](../../Cargo.toml), [toolchain](../../rust-toolchain.toml),
[workflow](../../.github/workflows/ci.yml), and [commands](../../README.md#verify).
Remote CI is `NOT_RUN` in the observed starting state. **Decision:** DEC-005 before
dispatch; the checkpoint does not authorize changing this configuration.

### CAP-CLI-006

**Name:** Public-readiness preparation and controlled distribution.
**Status:** PLANNED.
**Owner:** CLI/delivery with owner decisions. **Target:** 007.
**Dependencies:** Full 006 exits, CAP-SEC-002/003, DEC-001/002/003/009.

**Functional contract:** Prepare a reviewed release candidate with license decision,
repository/history/privacy review, distribution/versioning and operation/support
documentation, and reproducible checks appropriate to the delivered platform.
Record unresolved limits and readiness evidence. Separate technical preparation
from explicit owner authorization for public access, packages, releases, or launch.

**Advanced properties:** Candidate-bound delivery provenance and auditable approval
without unsupported readiness claims (INV-001/004/008/011/012).

**Acceptance:** An unlicensed, privacy-unreviewed, or unresolved-gate candidate
cannot be declared ready for its proposed distribution. Completing preparation
does not publish anything; absent owner authorization blocks launch.
**Verification:** NOT_RUN; release-candidate/history review and distribution checks
after decisions. No public repository, tag, release, or package is created here.
