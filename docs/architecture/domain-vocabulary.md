# Domain vocabulary

This document defines Cortex's canonical cross-domain terminology. The
[capability catalog](../product/capability-catalog.md) remains authoritative for
functional contracts and implementation status, the [core invariants](core-invariants.md)
for shared rules, [subsystem ownership](subsystems.md) for responsibilities, and
the [roadmap](../product/roadmap.md) for sequence. Definitions include accepted
planned responsibilities; a definition does not claim that the complete
capability is implemented or authorize an operation.

Rust names describe the active source APIs. Versioned storage and CLI
representations have their own compatibility contracts; source spelling is not
a serialized identity. See [representation compatibility](#representation-compatibility).

## Product and ownership

| Term | Positive definition and boundary | English / Rust usage |
| --- | --- | --- |
| Cortex | A native software-engineering and governance system that works with people and optional external AI tools. Deterministic contracts, authority, evidence and state belong to Cortex; model-dependent reasoning belongs to external participants. | Cortex; `cortex_*` crates |
| Capability | A separately identified product responsibility with a functional contract, owner and acceptance obligations. Implementation status describes delivery scope, not the validity of an individual result. | capability; existing capability IDs |
| Subsystem | A logical responsibility owner. A crate is a source/package boundary and may implement parts of several related logical responsibilities. | subsystem; crate/module names reflect actual ownership |
| Core | The owner of domain identities, validated contracts and guarded domain transitions, independent of infrastructure. | Core; `cortex_core` |
| Evidence | The responsibility for observations, claims, provenance, assessments, applicability, contextual validity and their inspectable correspondence, including governed interchange and invalidation under accepted contracts. | Evidence; `cortex_evidence`; implementation scope follows the catalog |
| Workflow | The owner of durable execution intent, recorded transitions, uncertain effects and reconciliation. Infrastructure persists those records without becoming another decision authority. | workflow; existing program journal |
| Harness | The native boundary that performs supported supervised operations and reports their observations under applicable authority. | execution harness; `cortex_harness` |
| Storage | The infrastructure owner of compatible durable representation, transactional admission and reconstruction. | storage; `cortex_storage` |
| CLI | A human/machine interface projecting canonical contracts and state. Command success must be interpreted according to that command's documented purpose. | command-line interface; `cortex_cli` |
| Knowledge | Repository intelligence through one shared derived, rebuildable code/document/schema/artifact index, with governed extraction, queries, conflicts, impact and retrieval. The bounded lexical index is one implementation slice. | Knowledge; reference indexes and adapter profiles |
| Memory | Canonical requirements, decisions and learning history, memory-layer retention, version-aware recovery and continuity/handoff composition. The current requirement/decision history is a bounded slice. | Memory; `CanonicalHistory` for that artifact |
| Skills | Typed, inspectable, versioned definitions and governance of registry, composition, imports and promotion. Definitions alone do not establish invocation or adoption. | skill definitions; skill registry |
| Context | Governed context assembly and translation with source maps, provenance, omissions, limits, disclosure rules and portable transport. A context package is an output, not the whole responsibility. | Context; instruction/context packages |
| Security and privacy | Cross-boundary threat analysis, privacy and executable-input provenance, with enforcement by the applicable domain/infrastructure owners. | scoped trust, retention and enforcement contracts |
| Evaluation | Declared cases/populations, protected oracles, reproducible procedures, complete attempt/resource accounting and controlled comparisons. | evaluation contracts and observations |
| Evolution | Identified change proposals, impact/evaluation basis, retained disposition and authorized promotion or rollback with separated proposal and approval roles. | evolution proposal; controlled promotion |

## Work, contracts and artifacts

| Term | Positive definition and relationship | English / Rust usage |
| --- | --- | --- |
| Task | A bounded engineering objective with an observable acceptance criterion and an identity independent of source revision. | task contract; `TaskContract`, `TaskId` |
| Program | A versioned, owner-accepted grouping of engineering outcomes and their protected acceptance baseline. Its ordering plan is a separate mutable record. | program definition; `ProgramDefinition`, `ProgramId` |
| Workstream | A coherent responsibility grouping for coordinated tasks within a program. Membership is separate from participant assignment, process ownership and sessions. | workstream; `program::Workstream`, `tasks: Vec<TaskId>` |
| Coordinated task | An engineering task represented in a dependency-coordination context, retaining its task/candidate contract, consumer kind and declared dependency coverage. It can be modeled before admission into a program. Its lifecycle projection and attempts remain separate records. | coordinated task; `parallel::Task`; existing `TaskId` and `TaskContract` |
| Task increment | A named delivery subdivision of a coordinated task, with zero or more exact artifacts and declared exclusive claim scopes. Releasing an intermediate artifact need not complete the task. | task increment; `program::Increment`, `task: TaskId` |
| Artifact | An identified engineering output or interface. ArtifactReference carries ID, revision and candidate; producer and dependency kind belong to the surrounding graph/requirement correspondence. A report about it is not its acceptance. | artifact reference; `ArtifactReference` |
| Acceptance baseline | The protected version of the program outcome and acceptance obligations. A plan change cannot silently amend it. | acceptance baseline; baseline revision |
| Ordering plan | A versioned ordering of tasks already included in the accepted program baseline, with its justification. Task-outcome acceptance is a separate condition. It changes precedence, not accepted scope, criteria, resource policy or authority. | ordering plan; `program::OrderingPlan` |
| Contract revision | An explicit version of a contract's meaning and protected inputs. | contract revision; `ContractRevision` |
| Generation | The current coordination generation used to reject stale work, separate from contract revision, source commit, process birth and boot identity. | task generation; `parallel::TaskScope` |
| Candidate | The exact material under assessment, identified to the extent supported by the source-observation contract. A Git revision assertion and a bound repository/worktree observation provide different evidence. | source candidate; `CandidateId`, `CandidateRequirement` |
| Candidate binding | The retained correspondence between a task contract revision, candidate requirement and actual scoped source observations. | candidate binding; `CandidateBindingContract`, `BoundCandidate` |
| Source reference | A bounded reference to source/version and declared provenance. The reference's existence does not authenticate the author or source contents. | source reference; `SourceReference` |
| Milestone | A roadmap acceptance boundary whose complete exits must be evidenced. | milestone, with stable existing identifiers |
| Phase / stage | A named portion of a specified procedure or delivery sequence. Use the enclosing procedure to distinguish a roadmap stage from a runtime phase. | campaign phase; `DomainPhase` only for the runtime state |
| Iteration / batch / campaign | An iteration is a repeated refinement cycle; a batch groups explicitly bounded operations; a campaign is a scoped verification/delivery effort. None is implicitly a new runtime work entity. | qualified ordinary prose |
| Governed Parallel Engineering | Cortex's coordinated engineering approach using explicit dependencies, authority, evidence and integration conditions. Parallelism is optional and does not itself establish productivity. | Governed Parallel Engineering (GPE) |

## Participants, roles and execution components

| Term | Definition and boundary | English / Rust usage |
| --- | --- | --- |
| Owner | The person or governing authority empowered to accept the relevant baseline or consequential policy decision. State which contract or resource is owned. | program owner; policy owner; resource owner |
| Operator | The person using or controlling a supported Cortex installation and supplying the applicable inputs. Being an operator does not imply every permission. | local operator |
| Actor | An identified participant attributed to an action. An identifier alone does not authenticate that attribution or implement the actor concurrency model. | actor; `ActorId` |
| Principal | The identity against which an operational grant is checked. Authentication and identity assurance are separate properties of the selected trust profile. | claimed principal; grant principal |
| Coordinator | The role or component reconciling program state and making only delegated coordination decisions. | coordinator role; `CoordinationKernel` for the current composition component |
| Reviewer | A participant assessing a scoped artifact or evidence basis. Independence requires applicable provenance and authorship separation, not a different role label. | technical reviewer; independent review within its stated scope |
| External AI assistant | An optional external tool providing model-dependent reasoning or authorized implementation. Its use to develop Cortex is distinct from Cortex-initiated execution. | external AI assistant; external coding assistant |
| Executor | A participant/tool implementation capable of carrying out a defined execution contract. Its supported operations and limits must be established separately from its advertised name. | executor; executor descriptor |
| Executor adapter | The translation boundary between a Cortex contract and one executor's protocol. | executor adapter |
| Controller | The native component retaining control of a particular supervised lifecycle, including the pre-release channel where supported. | execution controller |
| Observer | A mechanism producing bounded observations with declared scope, identity and limitations. Trusted observation requires an applicable trust basis. | domain observer |
| Session | An external tool's bounded interaction context, where that tool defines one. It is not an attempt, process or authority grant. | executor session |
| Worker | A contextual participant performing assigned work, or a conventional infrastructure worker when so qualified. Do not infer a model, OS process or permission from the unqualified noun. | engineering worker; worker thread |

## Coordination and lifecycle

| Term | Definition and boundary | English / Rust usage |
| --- | --- | --- |
| Responsibility assignment | A declared allocation of responsibility; it need not start an attempt or execution. | responsibility assignment |
| Ownership claim | A guarded exclusive responsibility record for a task increment and its declared claim scopes. Claim admission does not start a worker or grant operational authority. | ownership claim; `claim` |
| Exclusive claim scope | A declared semantic scope whose equal identity participates in conflict checks between ownership claims. A claim can include several such scopes; other conflict rules still apply. | `program::ClaimScopeId` |
| Attempt | A uniquely identified occurrence of undertaking assigned work. A metadata ownership attempt can exist without an external execution effect. | coordination attempt; `Attempt`, `AttemptId` |
| Operation | An exact journal admission input with identity and expected predecessor. Duplicate admission of the same complete typed input, with its canonical stored encoding, is distinct from repeating an external effect. | journal operation; `Operation`, `OperationId` |
| Effect | A proposed or represented externally meaningful action with durable intent and observations. ControlledFixture represents an effect without a real external operation; SupervisedFixture admits only its bounded supported operation. A retained effect record is not proof that execution occurred. | execution effect; `EffectId`, `EffectIntent` |
| Invocation | A particular manager/executor activation, identified according to its protocol. | manager invocation; invocation identity |
| Process incarnation | A particular process lifetime identified by PID and corresponding birth/epoch context. | process incarnation; `DomainProcessObservation` |
| Execution domain | The recoverable outer boundary intended to contain an attempt's processes/helpers and effects under a declared profile. Its identity is more than a name. | execution domain; `DomainIdentity` |
| Signal | Informational content that cannot itself satisfy a dependency or grant authority. | `Event::Signal` |
| Notification | An immutable graph-scoped envelope for an event. Its effect follows the contained event and guarded admission; a contained review is different from a signal. | `Notification`, `Event` |
| Artifact report | An assertion or observation that a specified artifact exists in a stated revision and context. | artifact report; `ArtifactReport` |
| Dependency release | A recorded decision that a specific consumer edge may use the specified support under its predicate and evidence context. | dependency release; `ReleaseRecord` |
| Responsibility handoff | A guarded transfer between accountable attempts/participants, preserving acknowledgments, history and outstanding obligations. | responsibility handoff |
| Dependency readiness | Satisfaction of the relevant declared dependency predicates. Contract stability, provisional implementation and integrated acceptance are distinct levels. | dependency readiness; `ReadinessLevel` |
| Eligibility | Satisfaction of coordination conditions for a named action within the represented graph/profile. It does not imply current authority or reserved resources. | eligibility; `eligible` |
| Authorization | Applicable permission for an exact operation, identity, scope, candidate, limits and policy. | operational grant; `OperationalGrant` |
| Reservation | A retained allocation used by admission accounting. Its enforcement, physical pool and cumulative/reusable meaning must be explicit. | resource reservation |
| Dispatch | Crossing, or possibly crossing, the external launch boundary. A lost acknowledgment can leave dispatch uncertain. | dispatch; `DispatchUncertain` |
| Prepare / bind | Admit intent, create at most once under that intent, and retain matching actual identity while holding workload release. A retained binding record is not a reconstructed live channel. | `prepare_nonce_domain`; binding observation |
| Payload release | The explicit operation permitting workload execution after current checks on the retained live preparation boundary. | `release_nonce_domain` |
| Cancellation | A recorded request to prevent or stop work. It does not by itself establish termination or cleanup. | cancellation requested; durable `Cancel` |
| Quiescence | Evidence-supported absence of continuing activity within the declared process/effect scope. Parent exit alone is insufficient. | domain quiescence |
| Closure observation | A bounded post-cancellation sample of domain state. Scope/completeness and missing proof remain visible. | `DomainClosureObservation` |
| Closure receipt | A retained scoped claim supporting closure/disposition under the selected trust contract. Structural acceptance and physical proof are distinct. | `ClosureReceipt` |
| Program journal admission receipt | Acknowledgment of an exact admitted or reconciled-present journal operation at a store incarnation and ordinal, including identical-duplicate status. It does not assert a new append, external completion or closure. | `program_store::AdmissionReceipt`; qualify the module or use a local import alias when needed |
| History admission receipt | Confirmation of a record version at a store incarnation and ordinal. History admission rejects duplicate versions; reconciliation can separately confirm retained presence. | history `AdmissionReceipt` |
| Admission ordinal | A position in a store admission sequence, independent of domain and recording timestamps. | admission ordinal |
| Settlement | Evidence-backed disposition of specified resource dimensions while retaining original outcome, consumption and unresolved obligations. | dimension-specific settlement |
| Recovery / reconstruction | Recovery addresses interrupted state under its contract; reconstruction rebuilds retained projections without repeating effects. | store recovery; projection reconstruction |
| Reconciliation | Comparison of retained intent and applicable observations to resolve a specified uncertainty, or retain it when evidence is insufficient. | commit reconciliation; effect reconciliation |
| Next-step guidance | Conditional advisory information derived from retained state. It is not a complete prerequisite evaluation, current permission or executable instruction. | `next_step_guidance`; declared advisory scope |
| Prepared-handle availability | Whether a current controller-owned channel/handle is established by the observer. A journal-only view leaves that availability unestablished; a retained binding does not recreate a channel. | `prepared_handle_availability: UNESTABLISHED_BY_JOURNAL` in retained-state inspection |
| Native closure support | The properties supported by bounded native samples, distinct from separate caller-asserted receipts and resource disposition. Incomplete samples do not establish whole-domain closure or final resource usage. | `native_support_status: INCOMPLETE` for the current limited observation path |
| Administrative disposition | A separate decision such as pause or abandonment that does not imply technical cleanup, refund or acceptance. | `AdministrativeDisposition` |

## Resources and measurements

| Term | Definition and units | English / Rust usage |
| --- | --- | --- |
| Logical worker slot | One logical unit of worker capacity declared in work demand or accounted usage. Reusable versus cumulative accounting is selected by policy; the unit is not a person, PID, thread, session or proof of a running worker. | worker slots; `Resource::WorkerSlots` |
| Build job / test-process units | Declared quantities of build jobs and test processes, respectively, with accounting selected by profile. They are not test cases, assertions, threads or measured OS population. | `Resource::BuildJobs`, `Resource::TestProcesses` |
| CPU capacity | A rate allocation expressed in millicores: 1000 units represent one CPU equivalent. This is distinct from cumulative consumed CPU time and from proof of physical allocation/enforcement. | `Resource::CpuMillicores`; `cpu_capacity_millicores()` |
| CPU execution time | Cumulative consumed CPU time in an explicitly named unit. Sampling and hard lifetime enforcement are separate properties. | `CpuExecutionMillis`; sampled CPU microseconds |
| Elapsed time | Duration in wall/monotonic time under the applicable clock contract, separate from CPU execution time and logical event ticks. Capacity/debit and consumption fields retain their separate roles. | `Resource::ElapsedMillis`; `Consumption::ElapsedMillis` |
| Memory quantity | Bytes measured or limited under a declared accounting basis and scope. Resident-memory bytes and cgroup-charged memory bytes are distinct quantities even when both use bytes. Their correspondence must not be inferred from a generic name. | resident-memory bytes; charged-memory bytes; explicit basis where the contract supports it |
| Storage quantity | Bytes, file objects, pathnames and inodes are independent dimensions. Logical image size does not establish physical host allocation. | managed disk bytes; file count; inode count |
| Output / input / buffering | Bounded transport quantities with separate stream, frame, event and aggregate-buffer scopes. | stdout/stderr bytes; frame bytes; event count |
| Process / thread capacity | Kernel task limits include the tasks specified by the host mechanism; direct-process observations cover only their declared membership scope. | process count; task/thread limit |
| Resource pool | An identified shared physical or governed allocation domain with an incarnation and policy. Different labels do not establish independent capacity. | `PoolIdentity`, `PoolContract` |
| Verification reserve | Capacity withheld from governed allocations for verification/recovery under the accounting policy. Physical protection requires an actual enforcement mechanism. | verification/recovery reserve |
| Declared limit / enforced limit / observed usage | A declaration is a configured intention; enforcement constrains effects within a supported mechanism; observation reports measured use with coverage and uncertainty. | qualify all three independently |
| Cumulative consumption | Reported or measured use accumulated under its accounting scope, with provenance identifying which basis applies. Recording a debit does not establish measurement, finality or completeness. | cumulative consumption |
| Unresolved consumption debt | A retained obligation where applicable final consumption is unestablished. It is distinct from quantified debit with its declared observation basis; missing measurement is neither zero nor a proven upper bound. | unresolved consumption debt |
| Reusable capacity | Capacity that may return to an allocation pool after the required scoped disposition. Its release does not clear cumulative debt or authorize retry. | reusable capacity |
| CI/model allowance | Declared job/usage limits under their separate policies. Job count, duration, tokens and money remain distinct quantities. | CI jobs; model tokens; observed cost |

## Evidence, knowledge, memory and methods

| Term | Positive definition and limit | English / Rust usage |
| --- | --- | --- |
| Observation | A report about what a mechanism observed or did, with provenance and declared coverage. It can remain caller-supplied and unauthenticated. | observation; `ExecutionObservation` / unified observation |
| Evidence record | Retained observations and any scoped assessment basis used to support an engineering judgment. | `EvidenceRecord` |
| Assessment | An explicit judgment against a criterion, with reasoning and supporting context. An absent assessment is unassessed, not an inferred NOT_RUN result. | `UnifiedAssessment`; foundation `Assessment` |
| Applicability | Whether the governed object/trigger is present and the criterion applies. Uncertainty is not demonstrated absence. | `Applicability` |
| Contextual validity | Whether support corresponds to the requested candidate/dependency/profile context within declared coverage. | `Validity::Current`, `Validity::Stale`; absent validity (`None`) is unestablished, not a third variant |
| Acceptance | A scoped judgment/decision that the relevant engineering outcome satisfies its acceptance contract. It is distinct from process success and operational permission. | artifact acceptance; task acceptance |
| Integration | A separately governed mutation producing a combined candidate that requires applicable evidence on that candidate. | protected integration |
| Verification / evaluation | Verification examines stated properties against explicit oracles; evaluation examines a declared case population or comparison. Neither is established merely by having test source. | verification evidence; evaluation case |
| Result | An explicit assessment conclusion: PASS, FAIL, UNKNOWN, NOT_RUN, BLOCKED or N/A under its stated rules. NOT_RUN records a procedure not performed. Foundation `Verdict::Inconclusive` retains its own semantics and has no implicit conversion into these results. | `AssessmentResult` |
| Invalidation | A recorded relevant mismatch that prevents reuse as current support without appropriate new evidence. It preserves original observations and terminal judgments. | causal invalidation |
| Provenance | The declared source, mechanism, version, input and temporal basis of a record. Integrity hashes do not authenticate its claims. | `Provenance`, `VerificationProfile`, `SourceReference`; `SourceOrigin` is the origin discriminator |
| Extracted relation | A relation obtained from supported source syntax, with its source span and extraction provenance. Resolution is not semantic validation. | `RelationOrigin::Extracted` |
| Reference index | A bounded derived projection of selected source occurrences and targets under an exact adapter profile. | `ReferenceIndex` |
| JSON reference source set | A selected set of JSON sources with bounded cross-document pointer resolution. Other lexical adapters cannot be passed to this JSON-specific resolver. | `ReferenceSet` |
| Reference resolution | Whether every emitted reference in the receiver’s declared scope has a resolved target. The universal predicate is true for a valid empty reference collection; this establishes neither extraction coverage nor semantic truth. | `all_references_resolved()` |
| Snapshot / delta | A retained bounded projection and a comparison between compatible projections. A knowledge snapshot is not a host/VM rollback guarantee. | knowledge snapshot; snapshot delta |
| Contextual validity inspection | Examination of a retained knowledge projection against supplied candidate/profile observations, producing a view without acquiring new source data. Its current summary also depends on emitted-reference resolution. The verb inspect does not promise purity in every other API. | Knowledge `inspect_validity` |
| Repository reinspection | Actual reacquisition of repository observations, followed by contextual validity inspection. | `reinspect` |
| Canonical history | Authoritative requirement/decision records with explicit corrections, supersession and temporal perspective. Logical append-only history coexists with separately authorized privacy deletion. | canonical history; `CanonicalHistory` |
| Domain time / recording time | Domain time identifies when a fact applies; recording time identifies when it was recorded. A domain-time interval and recording-time cutoff select different perspectives; an admission ordinal is a sequence position. | `DomainInterval`; recording instant/cutoff |
| Supersession | A correction overriding an earlier record only within its applicable domain interval and recording-time perspective. The eligible predecessor can remain applicable outside that interval; its history is retained. | supersession link |
| Knowledge index / memory record / context package | A knowledge index is a derived source projection; a memory record preserves canonical continuity; a context package is a selected bounded transfer artifact. These artifacts do not exhaust their owning subsystem responsibilities. | reference index; canonical record; context package |
| Instruction package | Deterministically assembled contract fields and explicit source inputs with identity, omissions and limits. Byte correspondence does not establish natural-language fidelity or authority. | `InstructionPackage` |
| Portable handoff / context package | A planned self-contained transfer of task context with project/candidate identity, source map, integrity checks, explicit omissions and disclosure/freshness rules. A snapshot recipient does not thereby gain live verification, responsibility or execution authority. | portable handoff; Portable Context Package |
| Skill / method / workflow | A skill is an inspectable typed definition; a method describes an engineering approach and applicability; a workflow governs its steps and transitions. | skill definition; engineering method; workflow |
| Method qualification | Evidence-backed examination of a method's declared cases and supported domain, separate from contextual applicability, adoption, blocking authority and effectiveness. | method qualification inspection |
| Contextual method selection | A proposal of suitable methods and obligations for a candidate and task; it can still await qualification, adoption or execution prerequisites. | contextual selection proposal |
| Method adoption | An authority-governed decision to incorporate or activate a method under applicable policy. Selection, availability and qualification support do not themselves adopt it or make it mandatory. | method adoption decision |
| Effectiveness | A measured outcome under declared comparison conditions, distinct from method availability or qualification. | effectiveness evaluation |
| Evolution proposal / promotion | A proposed change and its separately authorized activation after required evaluation and review. | evolution proposal; controlled promotion |
| Redacted projection | An output that omits or masks specified sensitive content under disclosure policy; underlying retained data may remain. | redacted inspection/disclosure |
| Privacy redaction / deletion / retention | Privacy redaction of retained payload is a planned governed operation across historical versions and scoped copies; deletion removes managed data under its contract; retention governs what is kept. None guarantees erasure of unowned copies or physical media. | retained-payload privacy redaction; scoped deletion; retention policy |
| Trust boundary / isolation / qualification | A trust boundary separates authority/data assumptions; isolation requires enforced controls; qualification is evidence for an exact profile and scope. | supported host profile; host-control qualification |

## Application rule

Use qualified names where two domains legitimately share a noun: journal admission versus resource admission; dependency release versus payload release; knowledge snapshot versus machine snapshot; observation inspection versus state-updating validity examination. Names do not substitute for the actual contract or its evidence. Existing external protocol terminology retains its protocol-defined meaning.

## Representation compatibility

Public Rust APIs and their in-scope consumers use these names together. This
source migration does not establish binary ABI stability or compatibility for
unknown external Git/path consumers. Module qualification distinguishes separate
receipt and task contracts without duplicate public abstractions.

### Active Rust API migration

Known in-tree callers migrate together to these public source names; no duplicate
public abstractions are retained. External Git/path consumers must update their
imports, exhaustive matches, struct fields and method calls as applicable.

| Previous public source spelling | Active source spelling and scope |
| --- | --- |
| `parallel::Mission` and its context/scope/state/status/blocker family | `parallel::Task`, `TaskContext`, `TaskScope`, `TaskState`, `TaskStatus`, `TaskBlocker`; the existing `TaskId` / `TaskContract` identity is unchanged |
| Coordination `mission` / `missions` fields and accessors; `MissionAcceptance` | `task` / `tasks` and `TaskAcceptance`, including graph, grant, increment and package consumers |
| `program::Plan`, `plan()` / `set_plan()` | `program::OrderingPlan`, `ordering_plan()` / `set_ordering_plan()`; `CoordinationPermission::Plan` remains the broader permission |
| `program::ResourceId`, increment `resources` | `program::ClaimScopeId`, `claim_scopes`; equality-based exclusion semantics stay unchanged |
| `Workers`, `Builds`, `Tests`, `CpuMillis`, `TimeMillis` resource variants | `WorkerSlots`, `BuildJobs`, `TestProcesses`, `CpuMillicores`, `ElapsedMillis` with unchanged units and accounting policies |
| `cpu_capacity_millis()` | `cpu_capacity_millicores()`; the same checked rounding and accessor/validation boundaries apply |
| Reference index/set `complete()` | `all_references_resolved()`; no extraction-completeness or semantic-validity claim |
| Knowledge adapter/view `qualify()` | `inspect_validity()`; `reinspect()` still acquires new repository observations |
| `program_store::Receipt` | `program_store::AdmissionReceipt`; other module-qualified receipts remain distinct |

These source changes may also change public Rust Debug/type names and compiler
paths. They do not establish source compatibility for undiscovered consumers or a
stable Rust ABI. Versioned machine representations use explicit mappings rather
than the renamed types' Debug spellings. The CPU capacity accessor still returns
`None` for a zero period or multiplication overflow, and `Some(0)` for zero quota
with a positive period; the separate scope validator still rejects zero quota.
`CpuExecutionMillis`, `CiJobs`, `ModelTokens` and the existing `RamBytes` dimension
retain their meanings.

### Retained formats and selected presentation

Supported historical journal and history formats retain their tags, field
positions, units, canonical bytes, identifiers and replay behavior. Instruction
packages retain their assembler/version-specific canonical rendering and hashes.
Historical names inside those representations are format vocabulary, not a
second active domain model. Unsupported versions remain explicit refusals.

Program inspection keeps its existing human and JSON schema by default.
`cortex program inspect ... --schema-version 2` selects the canonical projection;
add `--json` for its machine representation. Explicit schema version 1 retains
the original contract. Other command families retain their own schema, errors
and exits. The [program contract](program-kernel.md#read-only-cli-schemas) specifies the
selection and limits.

`RamBytes` retains its existing profile-dependent memory meaning. A generic byte
quantity does not establish resident or cgroup-charged measurement, and legacy
values cannot gain a measurement basis from a label. A universal typed memory
basis and a separate Knowledge summary for correspondence, extraction coverage,
reference resolution and query support remain design requirements; this
vocabulary does not change their current accounting or validity rules.
