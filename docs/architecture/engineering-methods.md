# Native engineering methods

Status: **PLANNED**. This is the shared architectural contract for native method
knowledge, qualification and selection, not an implemented Rust API or execution
grant. The [catalog](../product/capability-catalog.md) owns capability requirements
and status; the [roadmap](../product/roadmap.md#native-engineering-delivery-sequence)
owns sequencing. Existing [invariants](core-invariants.md) and
[evidence semantics](unified-evidence.md) remain authoritative.

CORTEX must eventually supply a useful, inspectable local engineering-method
baseline without requiring separately installed instructional packages, repeated
user checklists, a particular model or a hosted service. Optional imported methods
use the same qualification and authority boundaries. This is a requirement for
future delivery, not a claim that the present CLI replaces external development aids.

## Ownership and reconciliation

Engineering intelligence composes existing subsystems; it is not another registry,
index, evidence vocabulary, permission authority or workflow backend.

| Responsibility | Canonical owner and target |
| --- | --- |
| Exact method/rule definitions and inspectable provenance | [CAP-SKL-001](../product/capability-catalog.md#cap-skl-001), 004-B |
| Qualification, composition and admission of definitions | [CAP-SKL-002](../product/capability-catalog.md#cap-skl-002), 004-B; governed promotion 005-C |
| Contextual selection and engineering contract composition | [CAP-WFL-002](../product/capability-catalog.md#cap-wfl-002), 004-C |
| Protected acceptance, exceptions and operational decisions | [CAP-CORE-003](../product/capability-catalog.md#cap-core-003), 004-A |
| Actual observations, assessments and dependency-sensitive validity | [CAP-EVD-002](../product/capability-catalog.md#cap-evd-002), existing bounded semantics; [CAP-EVD-003](../product/capability-catalog.md#cap-evd-003), 003-C/006-A |
| Candidate-relevant facts and grounding | [CAP-KNW-001](../product/capability-catalog.md#cap-knw-001)/[002](../product/capability-catalog.md#cap-knw-002), 003-B/C; [CAP-CTX-001](../product/capability-catalog.md#cap-ctx-001), 003-F |
| Correction/defect/repair history and recovery | [CAP-MEM-001](../product/capability-catalog.md#cap-mem-001)/[002](../product/capability-catalog.md#cap-mem-002), 003-A/C/D |
| Effect lifecycle and actual dispatch enforcement | [CAP-WFL-001](../product/capability-catalog.md#cap-wfl-001), 004-C; [CAP-HRN-001](../product/capability-catalog.md#cap-hrn-001), 004-D |
| Optional planning/reconciliation and parallel integration | [CAP-WFL-003](../product/capability-catalog.md#cap-wfl-003)/[004](../product/capability-catalog.md#cap-wfl-004), 004/006 |
| Evaluation, accounting and promotion | [CAP-EVL-001](../product/capability-catalog.md#cap-evl-001)/[002](../product/capability-catalog.md#cap-evl-002), 005-A/C; [CAP-EVO-001](../product/capability-catalog.md#cap-evo-001)/[002](../product/capability-catalog.md#cap-evo-002), 005-B/C |

The following classifications concern specification coverage, not runtime enums or
implementation status. No new capability family is needed.

| Obligation | Reconciliation against existing contracts | Necessary refinement |
| --- | --- | --- |
| Qualified knowledge | REFINEMENT_REQUIRED: SKL-001/002 already own immutable definitions and positive/near-neighbor qualification | Inspectable knowledge categories, full qualification dependencies and retirement lifecycle |
| Common properties and specialization | REFINEMENT_REQUIRED: WFL-002 already selects target-language/ecosystem methods | Native baseline, component/interface applicability, inheritance without weakening and explicit profile conflicts |
| Protected change contracts | REFINEMENT_REQUIRED: CORE-003/WFL-002 already protect behavior and test baselines | Explicit pre/postconditions, error/effect ordering, data/schema obligations and versioned exceptions |
| Verification of verifiers | ALREADY_SPECIFIED: SKL-002 qualification, EVL-001 protected cases and EVL-002 false-positive/regression evaluation | Apply their existing gates; examples alone are not execution evidence |
| Technical grounding | ALREADY_SPECIFIED: KNW-001/002 and CTX-001 own source/version correspondence | No additional technical index or authority from retrieved documentation |
| Effects and recovery | REFINEMENT_REQUIRED: WFL-001/HRN-001/CORE-003 already own unknown effects and guarded retry | Cancellation is not rollback; data-use constraints and descendant/retry budget accounting remain explicit |
| Broader lifecycle domains | CONDITIONAL: WFL-002 owns applicable methods, with domain owners below | Maintain an honest operational-versus-research coverage inventory; no automatic subsystem proliferation |
| Defect learning and evaluation | ALREADY_SPECIFIED: MEM-001/002, EVL-001/002 and EVO-002 | Preserve the complete confirmation/qualification/promotion chain below |
| Optional Plan–Execute–Reconcile and parallel work | ALREADY_SPECIFIED: WFL-003/004 and INV-003/009 | Retain challenge/reconciliation and combined-candidate gates; no mandatory topology |
| Governed self-development | ALREADY_SPECIFIED: EVO-001/002, EVL-001/002 and engineering principles | Evaluate under applicable prior policy; no self-approval or independent-review claim from version comparison |
| Separate prompt/rule stores, quality evidence enums, permission engines or universal compiler/IR | REJECTED_OR_DUPLICATE | Existing owners suffice; these alternatives add competing state or unsupported scope |
| Unqualified specialized techniques | RESEARCH_PENDING until their applicability and oracle are established | Record the question and limits; do not turn a topic label into an operational method |

GENUINELY_NEW independent subsystem responsibilities: none. New normative details
refine the existing owners above; no IMPLEMENTED contract is enlarged by this document.

## Qualified engineering knowledge

A definition must identify, as applicable: stable identity/exact revision; property
or objective; language/ecosystem/domain and supported versions; applicability
conditions and their supporting sources; legitimate exceptions and counterexamples;
mechanism/oracle, required observations and coverage exclusions; qualification
evidence and disposition; applicable canonical requirements; provenance and temporal
dependencies; and the authority required for adoption, revision or retirement.
These are conceptual obligations, not a chosen struct, serialization or storage schema.

Keep information kinds inspectably distinct:

- Mandatory project requirements, with the applicable policy/approval reference.
- Mechanically checked properties, with actual mechanism and evidence references.
- Contextual heuristics and design recommendations, with applicability and limits.
- Unqualified hypotheses and demonstrative examples.
- Reproduced defects, separately from verified repairs and retained regressions.
- Imported observations or reported incidents, separately from CORTEX-captured execution.

A pattern match is a lead, not a confirmed defect. One failure in one candidate
does not establish a global prohibition. A definition may reference canonical
requirements but cannot rewrite them, declare itself mandatory or grant authority.
Memory retains authorized canonical records; the unified knowledge index derives
rebuildable relationships; SKL owns method definitions and qualification references.

### Qualification and revision

Propose an exact immutable definition; establish its applicability and expected
observations; exercise a defective case and a valid near-neighbor; inspect actual
discovery/execution, oracle adequacy and exclusions; then retain the scoped
qualification findings. Use relevant mutations/counterexamples where they can
discriminate an inadequate verifier. A fixture, scanner result, role label or zero
exit cannot replace the required observations.

Qualification is scoped evidence, not operational adoption. Adoption requires its
applicable authority; authoritative blocking also requires DEC-012's qualified-rule
policy. Changed rule/oracle/configuration inputs reopen affected qualification.
Supersession and retirement preserve retained history and the reason for changed
eligibility, subject to privacy-governed omission/deletion. Retirement must not
silently erase an inherited mandatory requirement. Missing history remains a gap.
No mutable replacement under an old exact reference is allowed.

Qualification/adoption lifecycle, feature status, result, applicability and validity
answer different questions. Reuse the existing Evidence dimensions for assessments.
This document creates no new result enum or implicit migration of Inconclusive.

## Common properties and contextual selection

Shared properties govern behavior, identity, compatibility, errors, effects,
security/privacy, resources, recovery and maintainability. Language/ecosystem and
domain profiles refine them or add obligations. Rust is one specialization of these
properties, not another quality authority or CORTEX's assumed target language.

Selection consumes explicit task intent/change scope, candidate-bound component
facts, applicable project requirements, qualified method versions and declared
capabilities/limits. It returns selected obligations and rationale, conditional or
uncertain applicability, required observations, conflicts and visible unsupported
coverage. No required obligation disappears because a tool or fact is unavailable.
A preview is a proposed engineering contract, not an assessed outcome or execution
authorization. Determinism is scoped to identical declared inputs and versions.

Several profiles may apply to a component, and interface properties may span
components/languages. Composition retains inherited requirements. Explicitly
reconcile contradictory methods or exceptions; neither order, language preference
nor a favorable result may silently weaken the shared baseline. Unknown profile
applicability stays visible. A genuinely absent trigger requires supporting
inspection, not a missing tool. Risk selects proportionate methods without
omitting required qualified review; no universal risk threshold is selected here.

### Lifecycle coverage inventory

All native methods below remain PLANNED/NOT_RUN. A future inventory must identify
the exact developed/qualified operational methods separately from research topics.
Each specialized family requires a concrete use case and declared verification
scope before implementation priority or execution is assigned.

| Coverage domain | Applicability trigger and owning integration |
| --- | --- |
| Problem discovery, feasibility and stakeholder requirements | Product/task decisions need validation; WFL-002 with CORE-003 requirements |
| Architecture evaluation/drift and mixed-component compatibility | An architectural/interface change; WFL-002 with KNW/CTX source facts |
| Data integrity, persistence, migration and recovery | Relevant state/schema/effect changes; WFL-001/002, MEM and Evidence |
| Security, privacy and data lifecycle | Applicable threat, data-use or retention obligation; SEC-001/002 and CORE/HRN enforcement; directly relevant to CORTEX |
| Operations, deployment, incidents and decommissioning | An authorized operational lifecycle task; WFL/HRN with explicit effect and recovery contracts |
| Usability, accessibility and internationalization | Affected users/interfaces/locales; WFL-002 qualified domain profiles |
| AI/agent and engineering-process evaluation | Model/harness/method claims; EVL-001/002; directly relevant to CORTEX |
| Scientific/numerical behavior | Relevant numeric model, units, tolerance or scientific oracle; WFL-002 domain qualification |
| Distributed systems | Actual distribution/ordering/consistency requirement; CONDITIONAL, not an essential-service or consensus implementation mandate |
| Hardware, real-time and safety-critical systems | A demonstrated timing/physical/hazard obligation; CONDITIONAL with qualified review; unsupported assurance remains unresolved |
| Maintenance, governance, contributors and lifecycle cost | Applicable change/review/operation; existing engineering principles, WFL-002 and EVL-002 |

## Change, observation and effect boundaries

An engineering contract identifies its task/revision, actual candidate and source
coverage, intended behavior delta, allowed mutation scope, protected pre/postconditions,
error semantics, effects/order, interface/data/schema invariants, acceptance baseline,
required evidence, exception/revision approval requirements and resource/privacy
constraints. A valid proposal can still await human approval or unavailable facts.

CORE-003 owns baseline identity and amendment authority. Altered tests, fixtures,
assertions, policies, configuration or discovery cannot silently improve acceptance.
Authorized requirement evolution records a new revision, approval and fresh evidence;
it need not make every ordinary test immutable. Compilation or unrelated green tests
do not prove the protected behavior. Executors may challenge an assumption with a
candidate-bound counterexample; they must not expand scope while awaiting resolution.

Preserve distinct source claims, imported reports, actual capture by a supported
mechanism, deterministic assessments and human authority. An import may retain its
claimed result and provenance without being upgraded to CORTEX-observed execution.
Matching IDs or successful parsing does not authenticate a report. Current 002-C
reports remain caller assertions; future producer/import distinctions require their
own validated boundary, not retrospective relabeling of those values.

KNW/CTX ground relevant APIs, consumers, dependency/protocol/schema versions and
configuration in actual candidate sources; availability is not proof of semantic
suitability. Evidence validity depends on the conclusion's actual sources, method,
oracle, fixtures, configuration, toolchain/environment and coverage. Explain affected
invalidation; preserve unrelated validity only with a justified non-impact basis.
An old record is not made current by recall, a new label or a successful import.

Requests, attempts, observations and confirmed effects remain distinct under WFL-001.
Cancellation/timeout is not rollback or proof of absent effects. Reconcile unknown
completion before retry, using actual operation identity/idempotency and any applicable
compensation/irreversibility contract. CORE/HRN enforce current operation and data-use
constraints at the effect boundary. Descendants, retries and adapter switches cannot
reset an applicable resource budget or bypass revocation. Numeric budgets, trace
capture and data-use/retention policy remain DEC-004/005/006 decisions.

## Learning, evaluation and optional self-development

The learning chain is reported defect -> reproducible case -> violated property ->
observed repair -> retained regression -> proposed method revision -> qualification
-> authorized promotion, rejection or deferral. MEM retains these distinctions and
their candidate/provenance links. Missing repair/confirmation stays explicit;
one incident does not automatically become a globally active rule.

EVL-001/002 evaluate escaped defects, false positives, regressions, maintainability,
oracle adequacy, review/integration burden, completion/repair time and observed
tokens/compute/lifecycle cost. Retain failed/incomplete attempts and relevant
populations; separate model and harness contributions or disclose confounding.
No benefit from a plan, checklist, test count or parallelism is presumed.

Optional Plan–Execute–Reconcile supports manual/single-agent and heterogeneous
executors. Parallel work needs stable shared contracts, explicit ownership/resource
paths and candidate-bound handoffs, followed by separate integration and fresh
combined-candidate checks. Worktrees are not sandboxes. The manual pilot provides
bounded integration evidence, not native CAP-WFL-004 implementation or a speedup claim.

Progressive self-application uses only delivered methods and explicit grants.
Evaluate a proposed policy/method version under the applicable prior accepted
baseline as well as required new checks. A candidate cannot approve its own
authority expansion, weaken gates or replace protected oracles to pass. Earlier-
version comparison is not automatically independent review; critical cases retain
the qualified/external review required by their accepted contract.

## Future acceptance scenarios

All cases are **PLANNED; execution NOT_RUN** for this native mechanism. Existing
002/003 tests demonstrate only their documented bounded behavior.

| Case | Required discriminating observation | Owner |
| --- | --- | --- |
| 1. Applicable rule, missing observation | Obligation retained; no PASS or completed task claim | WFL-002, EVD-002/003 |
| 2. Genuine non-applicability | Inspected absent trigger/basis, distinct from lack of tooling | WFL-002, Evidence |
| 3. Uncertain applicability | Visible unresolved obligation, no silent filtering | WFL-002 |
| 4. Tool unavailable | NOT_RUN/BLOCKED as appropriate, never unsupported-as-N/A | SKL-002, Evidence |
| 5. Candidate weakens acceptance | Governed baseline mismatch blocks acceptance despite green tests | CORE-003 |
| 6. Authorized baseline revision | Retain old revision; accept reviewed update with fresh candidate evidence | CORE-003, WFL-002 |
| 7. Heuristic matches valid code | Near-neighbor remains legitimate; no confirmed defect manufactured | SKL-002 |
| 8. Relevant dependency/rule/source/config changes | Affected validity reopened/STALE; justified non-impact needed for reuse | EVD-003, KNW-001 |
| 9. Mixed-language boundary | Both component and interface obligations selected; absent consumer verification remains unresolved | WFL-002, CTX-001 |
| 10. Executor challenges a plan | Counterexample reconciled against the contract; no unauthorized extra edits | WFL-003 |
| 11. Workers pass separately | A distinct integrated candidate still needs combined verification | WFL-004 |
| 12. Imported success report | Preserve reported claim; no invented CORTEX capture, approval or PASS | Evidence, CTX-002 |
| 13. Proposed weaker self-policy | Applicable prior protected gates and separate approval cannot be replaced by the candidate's policy | EVO-002, CORE-003 |
| 14. High-risk trigger | Required qualified review remains a gate despite inconvenient cost | WFL-002, DEC-012 |
| 15. Trivial low-risk change | Select only relevant obligations; no mandatory agents, consensus or formal machinery | WFL-002 |

### Early concrete targets

Use CORTEX itself: propose a bounded change to its JSON knowledge API. A future
preview selects shared identity/error/read-only properties and its applicable Rust
ownership, crate-boundary and verification methods from the observed workspace.
It retains the protected single-source API and explicit unknown evidence; it does
not execute the task or claim PASS merely by emitting the proposal.

Contrast a Rust producer and JavaScript consumer exchanging large identifiers:
select component and cross-boundary schema/encoding/precision obligations, including
an integer beyond JavaScript's exact numeric range. Describing those obligations
does not establish native JavaScript analysis or execution support. Producer-only
Rust checks cannot establish consumer behavior; missing consumer observations stay
unresolved. Qualify actual language mechanisms before broader coverage claims.

## Decision scope and reopening

Accepted invariants are shared authority/evidence/temporal semantics, one derived
index and explicit applicable obligations; they are not first-version backend choices.
The planned initial direction is small explicit in-memory method inputs, inspectable
qualification and a non-executing contract proposal. This offers utility before a
complete runner while leaving unsupported facts/manual approvals visible. Its cost
is narrower coverage and explicit operator-supplied context, not an invented guarantee.

Separate prompt stores, a new evidence or permission authority, and a universal
compiler/IR add no needed responsibility and are rejected here. Persistence, automatic
discovery, portable formats and execution backends are deferred to actual consumers
and their existing decisions. Reopen internal interfaces/coverage when a real consumer,
language mechanism, privacy constraint or qualification counterexample requires it;
record a new revision and requalify affected methods/evidence rather than silently
changing old meanings. Adoption/reviewer thresholds and evaluation governance still
require DEC-012/008 decisions, not implementation defaults.

## Implementation and contributor boundary

The [delivery sequence](../product/roadmap.md#native-engineering-delivery-sequence)
starts with bounded inspectable records and a candidate-aware contract proposal,
then returned-report reconciliation, actual governed execution, coverage extensions
and measured evolution. Human-readable inspection may precede portable interchange;
no storage schema, public trait, wire protocol, backend, engine or new crate is chosen.

Logical SKL/WFL ownership does not imply new crates. Core retains deterministic
identities/constraints; Evidence owns qualification observation/assessment semantics;
adapters own capture/presentation. Concrete APIs require actual consumers and must
preserve dependency direction. At DEC-007/012, settle the minimum definition,
qualification/import trust and adoption contracts before their respective consumers.
Runtime/effect/persistence gates remain separate; a pure preview cannot exercise them.

A contributor must be able to inspect why a method applies, its exact inherited
obligations, exceptions, qualification evidence, missing context and revision history.
Add a focused discriminating positive/negative case when changing a qualified rule;
keep rationale, supported versions and exclusions discoverable. Do not scatter
duplicate rule definitions or conflate runtime tests with evidence that they ran.
Risk/reviewer policy, publication, privacy defaults and broad API compatibility
remain open at their existing decision points.
