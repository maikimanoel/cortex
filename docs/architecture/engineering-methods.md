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

The table records existing coverage and remaining obligations. Coverage is an input
to the independent decisions below, not a classification of applicability,
qualification, disposition or implementation priority. No new capability family is needed.

| Obligation | Reconciliation against existing contracts | Retained obligation |
| --- | --- | --- |
| Qualified knowledge | SKL-001/002 own immutable definitions and positive/near-neighbor qualification | Inspectable knowledge categories, full qualification dependencies and retirement lifecycle |
| Common properties and specialization | WFL-002 selects target-language/ecosystem methods | Native baseline, component/interface applicability, inheritance without weakening and explicit profile conflicts |
| Protected change contracts | CORE-003/WFL-002 protect behavior and test baselines | Explicit pre/postconditions, error/effect ordering, data/schema obligations and versioned exceptions |
| Verification of verifiers | SKL-002 qualification, EVL-001 protected cases and EVL-002 false-positive/regression evaluation | Apply existing gates; examples alone are not execution evidence |
| Technical grounding | KNW-001/002 and CTX-001 own source/version correspondence | No additional technical index or authority from retrieved documentation |
| Effects and recovery | WFL-001/HRN-001/CORE-003 own unknown effects and guarded retry | Cancellation is not rollback; data-use constraints and descendant/retry budget accounting remain explicit |
| Broader lifecycle domains | WFL-002 owns applicable methods, with domain owners below | Maintain an honest operational-versus-research coverage inventory; no automatic subsystem proliferation |
| Defect learning and evaluation | MEM-001/002, EVL-001/002 and EVO-002 | Preserve the complete confirmation/qualification/promotion chain below |
| Optional Plan–Execute–Reconcile and parallel work | WFL-003/004 and INV-003/009 | Retain challenge/reconciliation and combined-candidate gates; no mandatory topology |
| Governed self-development | EVO-001/002, EVL-001/002 and engineering principles | Evaluate under applicable prior policy; no self-approval or independent-review claim from version comparison |
| Overlapping prompt/rule/index or evidence/policy proposals | Existing SKL/KNW/Evidence/Core owners | Consolidate overlapping obligations without retaining a competing authority or erasing material distinctions |
| Universal compiler/IR proposal | No required consumer established | Reject this additional mechanism on scope and maintenance grounds; retain applicable language-independent objectives |
| Unqualified specialized techniques | Owners depend on the actual property and use case | Retain unresolved proposals for research until applicability and an adequate oracle are established |

No new independently owned subsystem responsibility is introduced. Normative details
refine the existing owners above; no IMPLEMENTED contract is enlarged by this document.

## Independent reconciliation decisions

First distinguish the engineering property from each proposed method. A property
states an objective and a meaningful violation condition; a method attempts to
establish or preserve it. For example, trustworthy failure reporting is broadly
relevant; process termination, fault injection and formal modeling verify different
parts of that property under different assumptions. None is a universal procedure.
Normalize compound source claims into distinct obligations before deciding what to adopt.

Record four decisions independently, with scope, rationale and supporting references.
These are specification labels, not new Rust enums or replacements for Evidence.

| Decision | Required distinctions |
| --- | --- |
| Applicability | **CROSS_CUTTING:** relevance broadly independent of language/architecture/domain, with an objective and violation condition. **CONTEXTUAL:** identify the trigger, resulting obligations and legitimate absence conditions. **UNDETERMINED:** available facts do not establish applicability. **NOT_APPLICABLE:** inspected evidence establishes the required trigger's absence within the assessed scope |
| Qualification | Explain what evidence supports the exact method, what remains unestablished and any material contradiction, as specified below. Applicability or source reputation cannot supply missing qualification |
| Disposition | **INCORPORATE:** adopt an objective into its existing applicable contract, subject to authority and qualification. **DEFER:** retain justified work for dependencies, cost, scope or sequence. **RESEARCH:** retain an unresolved proposal without normative/operational authority. **CONSOLIDATE:** normalize overlap, preserving distinctions/provenance. **REJECT:** exclude with a substantive technical or product-scope reason. **NO_CHANGE:** existing coverage has no demonstrated material deficiency |
| Implementation priority | If implementation remains necessary, justify **P0** (essential guarantees or first useful native workflow), **P1** (strong near-term value), **P2** (justified later/use-case-dependent work), or **UNSCHEDULED** (insufficient evidence, unresolved applicability or adoption prerequisites). A priority is not implementation authorization |

Scope matters: retain a property's general relevance separately from the finding
about a particular task. Neither CROSS_CUTTING nor CONTEXTUAL automatically maps to
Evidence's `Applicable`; a concrete assessment needs its own inspected context.
Missing tools, source access, inspection or expertise cannot establish
NOT_APPLICABLE. Uncertainty must not silently remove an inherited obligation.

A cross-cutting objective may be adequately covered and require NO_CHANGE with no
additional implementation to prioritize. A contextual method may be P0 when it
is necessary for an essential workflow. Deferral does not diminish importance;
incorporating an objective does not qualify every proposed detector for it.
Research can address a relevant property without authorizing an unqualified method.

### Qualification findings and evidence

For the exact method/version, assess property, environments/versions, preconditions,
mechanism, required observations/oracle, supporting evidence, legitimate exceptions,
counterexamples, false-positive/false-negative limits, validity dependencies and
the scope of any justified conclusion. Correctness and practical effectiveness
require their respective evidence; unmeasured outcome benefits remain unestablished.
Distinguish in the reasoned finding:

- Sufficiently qualified within the declared scope, with adequate current evidence.
- Partially qualified: supported subclaims and unestablished parts are explicit.
- Unqualified proposal: qualification requirements are known but not met.
- Unassessed proposal: the relevant assessment has not occurred or sources are unavailable.
- Materially contradicted claim: retain the counterexample, affected claim and scope;
  do not infer that the broader engineering objective is invalid.

These descriptions summarize evidence, not a second evidence-state machine. Reuse
existing result/applicability/validity dimensions on each supporting assessment.
An absent assessment stays unassessed; a selected unexecuted mechanism remains
NOT_RUN, an unavailable prerequisite BLOCKED, insufficient interpretation UNKNOWN,
and changed relevant dependencies STALE or validity unestablished as contracted.
Do not infer a conclusive assessment from these prose descriptions or synthesize
missing observations. A reproduced defect does not itself qualify its detector;
evaluate defective and legitimate near-neighbor cases and the actual oracle.
Documentation, a scanner match or source endorsement alone is insufficient.

### Decision procedure and traceability

For each normalized candidate obligation:

1. State the engineering objective, separately from the proposed technique.
2. Identify the relevant failure mode/property and a discriminating violation case.
3. Inspect available source/version support, evidence and material limitations;
   identify unavailable material explicitly instead of treating a summary as exhaustive.
4. Assess applicability in the declared scope independently of verifier availability.
5. Locate the existing capability owner, coverage and dependencies; preserve adequate contracts.
6. Assess the mechanism and qualification requirements using the scoped evidence above.
7. Normalize overlap and reconcile contradictions/exceptions; preserve unresolved
   disagreement until facts or governing authority settle it.
8. Record disposition and technical/product rationale, including any required approval.
9. Prioritize remaining implementation from relevance/severity, practical utility,
   trigger frequency/significance, existing coverage, verification feasibility/evidence,
   implementation/maintenance cost, dependencies, mechanism risk, false-positive/process
   burden and reuse. Do not invent numerical weights, ROI or universal rankings.
10. State observable acceptance, unresolved decisions/owners and their dependency points.

Retain explainable source-to-obligation correspondence, scope/candidate/version,
the four separate decisions, rationale, acceptance and reopening dependencies.
Many source items may support one obligation; one source item may split into several.
Do not decide by terminology, repetition, apparent similarity or reputation alone.
Consolidation preserves material differences and provenance; it is not rejection.
An unresolved disagreement or missing source remains visible, without invented consent.
Private source correspondence stays outside the repository; official contracts retain
the normalized rationale and obligations needed by contributors, not private inventories.
No storage/wire format or new tracking subsystem is selected by these obligations.

### Conceptual roles

Keep these roles distinct when normalizing knowledge. References may connect them;
one item or field must not silently stand for all of them. These are conceptual
contracts, not new Rust types or a serialization model.

| Role | Meaning and boundary |
| --- | --- |
| Engineering property | Objective or invariant with a meaningful violation condition, independent of the chosen technique |
| Authorized requirement | Obligation adopted for a task/version by its applicable authority; advice alone is insufficient |
| Acceptance criterion | Observable condition used to judge a requirement, with scope and legitimate exceptions |
| Method or technique | Procedure intended to establish/preserve a property, with preconditions, costs and limitations |
| Applicability condition | Target/task/environment fact that triggers an obligation or justifies its scoped absence |
| Verification mechanism | Concrete procedure/tool/version that obtains relevant observations; its success is not automatically acceptance |
| Test oracle | Independently justified interpretation of expected behavior that discriminates violation from compliance |
| Diagnostic heuristic | Review lead with possible false positives/negatives; a match is not a demonstrated defect |
| Reported defect | Source claim about a failure, retaining its source and unverified parts |
| Demonstrated defect | Reproduced violation connected to its actual candidate, conditions, property and observations |
| Evidence | Retained property-relevant observations/provenance, with their declared verification coverage and validity dependencies |
| Assessment | Explicit judgment over supporting evidence; result, applicability and validity stay separate |
| Qualification record | Exact-method scoped support, counterexamples, exclusions and unresolved claims; not adoption authority |
| Engineering profile | Applicable shared and specialized obligations, preserving inheritance and explicit conflicts |
| Policy or authority | Governing decision and approval boundary for adoption, exceptions or effects; retrieved knowledge cannot grant it |
| Implementation capability | Actual supported behavior whose delivery status needs inspected code and matching verification |

Identifier inventory, normalized specification coverage, method qualification,
runtime implementation and runtime verification are independent claims. An inventory
can be complete while titles, examples, evidence or exceptions remain unavailable.
Preserve that distinction per source item; do not infer a missing problem statement
from its chapter or promote an indexed title into a qualified mechanism.

### Engineering intent and requirement quality

The following obligations refine CORE-003/WFL-002 contract formation, with KNW/CTX
source grounding, MEM decision history and Evidence/EVL observations. They are
PLANNED. Apply them at the material boundary; do not require a modeling notation,
prototype, extra reviewer or specialized tool on every task.

| Boundary | Required contract and discriminating acceptance |
| --- | --- |
| Intended need and validation | Retain the problem, affected actor/operator, intended observable outcome and non-goals separately from a preferred solution. Distinguish conformance to the authorized specification from validation of its intended use: a green implementation of a mistaken requirement cannot establish that the need was met |
| Vocabulary and actors | Resolve material domain terms, roles and decision rights using relevant sources, or expose uncertainty/conflict. Distinct meanings for the same identifier cannot silently produce one favorable contract; contextual stakeholder or user scenarios retain affected interests |
| Requirement classes and allocation | Distinguish functional outcomes, quality attributes, constraints, interfaces and operating-environment obligations. Link each applicable requirement/revision to responsible components and acceptance/evidence coverage. An omitted, orphaned or contradictory clause remains unresolved even when selected tests pass; preserve abnormal/degraded paths and explicit exceptions |
| Assumptions and trade-offs | Identify material assumptions, source/version, decision owner and review/expiry or change trigger. Where an assumption could reverse the choice, record that sensitivity and the alternative, not invented confidence or ROI. A contradicted assumption reopens affected conclusions; elapsed time alone does not establish validity |
| Architecture views and scenarios | When multiple perspectives matter, distinguish the viewpoint/question, its concrete view and the represented model/version. Relate intended boundaries to observed source composition and diagnose disagreement. A quality-attribute scenario identifies stimulus, environment, required response and observation; thresholds come from the authorized requirement, not the method selector |
| Fault, error and failure | Distinguish a fault/cause, erroneous internal state and observable service/contract failure where diagnosis requires them. Preserve degraded/partial effects and recovery limits. A successful retry does not erase the failed attempt, and process termination or injected call failure does not establish physical power-loss behavior |

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

Retain qualification by claim as well as method revision: a disputed example or
proposed repair can be excluded from supported coverage while the broader property
remains useful. Establish the actual language/API laws or execution mechanism
before accepting a counterexample or optimization; require representative measurements
for claimed gains. A renamed operation, plausible example or source-reported
demonstration cannot qualify itself. Record the correction, legitimate near-neighbor
and requalification dependency without silently repairing or erasing the source claim.

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
| 16. Broad property, contextual techniques | Preserve the shared objective without mandating every verifier; an essential contextual mechanism can still receive P0 | WFL-002, SKL-002 |
| 17. Overlapping or compound claims | Consolidate normalized obligations, split distinct properties and preserve exceptions/provenance; duplication alone is not rejection | SKL-001/002 |
| 18. Contradicted detector claim | Retain the scoped counterexample and unestablished coverage; no automatic rejection of the property or qualification from its reproduced defect | SKL-002, EVL-002 |
| 19. Missing or conflicting source support | Record unassessed material and unresolved disagreement; no exhaustive-coverage, non-applicability or adoption claim from a summary | SKL-001/002, CTX-001 |
| 20. Adequate coverage or deferred method | NO_CHANGE creates no artificial implementation work; DEFER preserves rationale/priority independently of applicability and qualification | WFL-002 |
| 21. Correct implementation, mistaken requirement | Conformance evidence does not establish intended-use validation; unmet actor/outcome assumptions remain visible | CORE-003, WFL-002, EVL-001 |
| 22. Ambiguous term or unallocated clause | Material meaning/owner conflict or omitted abnormal path stays unresolved despite unrelated green checks | CORE-003, WFL-002, KNW-001 |
| 23. Contradicted assumption or architectural view | Reconcile affected source/version/decision dependencies; neither an old diagram nor a fresh label establishes current conformance | WFL-002, KNW-001/002, EVD-003 |
| 24. Questionable example or optimization | Check actual laws/mechanism and a legitimate near-neighbor; retain unsupported gains and affected subclaims without rejecting the property | SKL-002, EVL-002 |
| 25. Inspect-only method consumer | Listing definitions/qualification executes no processes, changes no files or active policy, grants no permission and creates no acceptance assessment | SKL-001/002 |
| 26. Complete inventory, incomplete source | Separate inventory/specification/qualification/implementation/verification claims; missing source detail is not silently reconstructed | SKL-001/002, CTX-001 |

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
External instructional aids are progressively unnecessary only for methods whose
native applicability, qualification, inspection and required workflow behavior have
actually been implemented and verified. Reassess affected coverage on revision;
unsupported obligations and human review remain explicit. This specification alone
does not replace those aids or establish autonomous engineering ability.

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
