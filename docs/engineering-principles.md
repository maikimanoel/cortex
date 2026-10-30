# Cortex engineering principles

Cortex makes engineering claims answerable to observable evidence. Probabilistic
agents may help interpret a problem and propose work; deterministic mechanisms
must govern contracts, permissions, state, execution boundaries, and completion.
No model response, tool exit code, or agent confidence can substitute for those
mechanisms.

The [capability catalog](product/capability-catalog.md) owns functional requirements
and status. The [core invariants](architecture/core-invariants.md) own precise
cross-cutting rules; the [roadmap](product/roadmap.md) owns sequence. This document
explains the engineering doctrine behind them, without claiming future mechanisms
are implemented by the current foundation.

The [native method contract](architecture/engineering-methods.md) specifies how
future Cortex methods make this doctrine inspectable: shared properties, qualified
language/domain refinements, explicit applicability and candidate-bound evidence.
Method selection must improve contract formation as well as verification; its
planned native baseline is not a claim of current autonomous engineering support.

## Complete the authorized capability at the right layer

For every capability, first describe inputs, the observable acceptance behavior,
and a counterexample that should be rejected. Implement the mechanism at the
layer that actually controls that behavior. A missing convenient library or paid
service is a reason to investigate an appropriate local implementation, not to
quietly weaken acceptance. Technical difficulty also does not authorize an
unbounded project: keep scope, assumptions, and unfinished work explicit.

The current foundation applies this rule to validated domain construction,
evidence transitions, and CLI behavior. It makes no execution-harness claim.

## Own boundaries, failure, and termination

Use domain-specific error categories where a caller must choose how to respond.
Reject invalid input before retaining it. Keep diagnostics specific about the
field or failed condition without exposing its private contents. Resource owners
must define limits, cancellation, cleanup, and terminal conditions for the
operations they introduce. State changes need explicit allowed transitions and
must leave a valid state when rejected.

Current text contracts have finite byte limits. Foundation evidence transitions
are atomic in-memory updates. Unified evidence preserves observations and terminal
judgments on rejection; a valid context examination can separately retain a newly
observed invalidation even when a conclusive judgment is rejected. Future process
execution requires separate controls for time,
output, filesystem access, memory, and process cleanup. In-memory validation is
not an operating-system sandbox.

## Keep observation, judgment, and permission separate

A task identifies the desired result. A candidate identifies the source version
under consideration. An execution observation reports what a mechanism did. An
assessment explains what that observation supports about the acceptance criterion.
Human authorization governs whether a consequential operation may happen.
None of these concepts implies another.

Exit code zero alone cannot establish semantic correctness. A skipped,
unavailable, cancelled, or timed-out check must not become an observed pass.
Do not erase failures, stale evidence, partial coverage, or unavailable tools to
produce a reassuring conclusion. A capability is complete only to the extent
that its actual behavior and required acceptance have supporting evidence.

Keep agent statements, observations, inferences, evaluations, approvals, and
transitions distinguishable. Typed authority must be enforced by real tool/host
controls; prompts, recovered memory, skills, and tool responses cannot enlarge
their own authority. Lower-trust content remains data regardless of channel.

## Retain the basis for each claim

Evidence must identify the relevant task contract, source candidate, inputs,
verification mechanism and version, configuration, observed result, and the
reasoning behind any assessment. Record environmental conditions and limitations
that could change the conclusion. New code, inputs, or configuration require
fresh relevant verification; an old report cannot be reassigned to new bytes.

Descriptions and hashes bind claims to context but do not authenticate their
authors or prove their truth. This milestone accepts caller-reported provenance;
it provides neither tamper-evident storage nor independent verification.

Temporal provenance applies to knowledge, memory, decisions, and observations.
Identity, source/version references, domain validity and recording time, and
dependency-aware staleness must accompany future persistent/context capabilities.
Recall is not freshness. Logical history and privacy-driven deletion require
explicit policy; neither silently overrides the other.

Tests must exercise observable contracts and discriminate plausible incorrect
behavior. Inspect what actually ran. A green invocation with zero relevant tests
does not verify a capability. State precisely which surfaces remain untested.

## Make the baseline locally operable

Cortex's operational core must work without a commercial API, paid AI provider,
subscription, managed database, or hosted infrastructure. Prefer free,
open-source, locally run, or operator-controlled resources when they satisfy the
engineering requirement. Zero mandatory service expenditure does not mean local
computation has no physical cost or capacity limits.

Optional paid integrations may be added only as explicit, separately authorized
extensions. An operator selecting an integration must not silently lose a
complete free implementation. Cost savings never override correctness, security,
authorization, evidence integrity, or the boundaries of the requested product.

## Preserve a foundation that can change honestly

Keep APIs small and dependencies directional. Build extension points from actual
requirements, not an imagined complete platform. The same engineering controls
must be useful when the application being built contains no AI.

Report limitations and unperformed checks alongside successful results. Do not
claim comprehensive readiness, security certification, human approval, or remote
CI success without the corresponding authority and evidence. Repository privacy
and unpublished packages remain deliberate boundaries. The owner-selected
[MIT License](../LICENSE) does not authorize changing either boundary.

## Maintainability and contributor experience

These are cross-cutting acceptance requirements for future milestones and focused
changes, applied in proportion to risk and the affected contract under
[INV-011](architecture/core-invariants.md#inv-011). Each subsystem owns its
implementation and documentation; CLI/delivery coordinates setup, verification
instructions and the future contributor walkthrough. The catalog owns functional
scope and the roadmap sequences its acceptance. This checkpoint establishes
requirements, not independent evidence that contributor usability has been tested.

**Architecture and modularity:** A reviewer must be able to trace a changed
requirement to its owning cohesive crate/module, invariants and consumers. Keep
dependencies directional and acyclic, responsibility boundaries explicit and
public visibility minimal. Reject duplicated domain authority/evidence semantics
and speculative generic frameworks. A new crate or abstraction needs an actual
consumer and an observable boundary; KISS and YAGNI apply to documentation too.

**Idiomatic Rust:** Make ownership, borrowing and resource lifetimes understandable;
use validated types where they enforce a meaningful invariant. Errors must have
predictable categories/causes and private-input-safe diagnostics. Document public
invariants and use consistent native naming/conversion conventions. Safe Rust and
the existing workspace prohibition on unsafe code remain the baseline; a future
exception would need an explicitly authorized policy change and demonstrated need,
with documented and verified obligations. Feature, toolchain and compatibility
changes require deliberate scope and affected checks, without inferring broader
API/platform guarantees from a version number (DEC-006/007).

**Verification:** Connect tests to observable requirements and plausible incorrect
behavior. Include relevant negative/boundary cases, corrected-defect regressions,
integration tests at consequential boundaries and end-to-end tests for essential
user workflows. Record actual results, configuration, skipped/unavailable checks
and unverified scope. Neither a passing count nor successful compilation is a
universal correctness metric; select techniques by the property being assessed.

**Documentation:** Examples, setup/build/test instructions, supported behavior and
limitations must agree with the inspected candidate. Keep terminology, architecture,
catalog status and roadmap consistent. Retain one canonical owner for a requirement
and link to it. A planned capability cannot be described as demonstrated behavior;
a future contributor must not need private conversation history to recover contracts.

**Contributor acceptance:** Under the future CAP-CLI-006 contract, a recorded
walkthrough must demonstrate that a competent external Rust contributor can use
the permitted project documentation to:

1. Understand the purpose and actual implementation status.
2. Locate the responsible subsystem and relevant modules.
3. Set up the documented local development environment.
4. Build and test the relevant workspace components.
5. Identify applicable contracts and invariants.
6. Make a focused change without private chat history.
7. Verify it with the documented commands and record limitations.
8. Submit a reviewable contribution with clear candidate-bound evidence.

Record prerequisites, attempted steps, failures and the assessed candidate; an
internal rehearsal is not independent external usability evidence. These are
observable requirements; 002-P2 rehearses the scoped pre-alpha source path without
claiming independent external acceptance. DEC-003's approved initial maintainer/PR
policy is documented in [CONTRIBUTING.md](../CONTRIBUTING.md); broader governance
remains deferred. This document does not authorize public access.

**Change review:** Preserve contract compatibility, accurate documentation, relevant
tests, security/privacy boundaries, resource/error semantics and evidence validity.
Changed relevant inputs require fresh verification. Review depth follows the
change's risk and required controls, with DEC-012 retaining its classification and
qualified-review decisions. No universal formal proof, agent count, orchestration
method or excessive fixed checklist is required for ordinary changes; critical
work cannot bypass its contract-required review.

## Choose proportionate methods and measure change

Satisfy each capability's complete functional contract before claiming its
advanced properties. Use auditable task profiles: trivial work need not create
multiple agents or formal models; critical work must retain the qualified review
required by its contract. Specification-first and test-driven methods are tools
to select for the problem, not universal ceremony.

Compatibility requires observations against actual adapters and environments.
Evolution proposals are separate candidates with evaluation and recorded
disposition; a proposer cannot approve itself. Reproducible baselines, negative
and held-out cases, resource records, and explicit oracles support measured
improvement. Separate harness and model effects where possible, disclose
confounding, and never replace missing experiments with confidence or aggregate
scores. Publication and consequential promotion need their own authority.

**Progressive dogfooding:** Deliver usable slices early and use Cortex to help
build Cortex only as its capabilities become demonstrated and explicitly authorized.
CLI/delivery coordinates this practice using the documented baseline (CAP-CLI-005);
future method automation belongs to CAP-WFL-002 and empirical benefit claims to
CAP-EVL-002. Practice adoption is optional, with feature status distinct from the
result/validity of a particular use. Optional planning,
delegation and parallel work must preserve shared contracts, independent bounded
workspaces, and human/core authority. An executor does not approve its own
consequential promotion. Revalidate the integrated candidate; passing branch-local
checks cannot establish integrated acceptance. Evaluate actual time to verified
integrated outcome, quality, token/resource expenditure, retries, coordination
cost and regressions before making efficiency or superiority claims.

The [native parallel contract](architecture/governed-parallel-engineering.md)
refines this practice with per-consumer evidence-gated releases, causal readiness
and separate dispatch authority. An intermediate release need not wait for its
producer's whole mission; a scheduler cannot convert a signal into approval.
Cortex will implement that control plane in Rust under the existing owners, preserving
the separate qualified-method and effective-host boundaries.

Plan–Execute–Reconcile, continuity/context features, portable packages and governed
parallel work retain their catalog owners and roadmap targets. Selecting these
methods/features is optional where applicable; satisfying their contracts and the
shared invariants is mandatory when selected. Their planned interfaces must also
support direct single-agent work. Additional agents alone never establish an improvement.
