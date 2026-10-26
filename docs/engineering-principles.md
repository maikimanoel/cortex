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

Current text contracts have finite byte limits; evidence transitions are atomic
in-memory updates. Future process execution requires separate controls for time,
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
CI success without the corresponding authority and evidence. Repository privacy,
unpublished packages, and an undecided license remain deliberate boundaries of
this milestone.

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

Deliver genuinely usable slices early and use Cortex to help build Cortex only
as its capabilities become demonstrated and authorized. Optional planning,
delegation and parallel work must preserve shared contracts, independent bounded
workspaces, and human/core authority. An executor does not approve its own
consequential promotion. Revalidate the integrated candidate; passing branch-local
checks cannot establish integrated acceptance. Evaluate actual time to verified
integrated outcome, quality, token/resource expenditure, retries, coordination
cost and regressions before making efficiency or superiority claims.
