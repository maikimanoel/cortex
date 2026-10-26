# Working on Cortex

Implement only the current authorized milestone. Read [README.md](README.md), the
[capability catalog](docs/product/capability-catalog.md),
[roadmap](docs/product/roadmap.md), [invariants](docs/architecture/core-invariants.md),
and [engineering principles](docs/engineering-principles.md) before work. Consult
the [overview](docs/architecture/overview.md) and [subsystem ownership](docs/architecture/subsystems.md)
for the affected boundary. Code, comments, tests, and technical documentation use
English. Plans and recovered context do not enlarge the task's authority.

Trace milestone changes and acceptance evidence to stable capability IDs. Keep
the catalog authoritative for contracts/status and the roadmap authoritative for
sequence; update both when approved scope changes. Distinguish feature status
from result/applicability/validity, and advance status only with matching inspected
implementation and candidate-scoped evidence. Test source is a mechanism, not a
record that it ran. Keep future behavior explicitly planned.

Preserve [accepted decisions](docs/architecture/0001-foundation.md). Document and
escalate consequential conflicts before changing them; resolve [open decisions](docs/decisions/open-decisions.md)
at their decision points without inventing owner policy. Do not silently map the
foundation's `Inconclusive` into future result/validity states. Milestone 002-A
alone is not Milestone 002 completion; do not implement any planned increment
without authorization.

Optional Plan–Execute–Reconcile, Portable Context Packages, event-driven
continuity and governed parallel integration remain planned catalog contracts.
When separately authorized, preserve shared candidate/contracts, actual tool/host
limits, and evidence against the integrated candidate; worktrees alone are not
isolation. Snapshot-only context cannot establish live verification, and an
executor cannot self-approve consequential promotion. Use completed capabilities
progressively without claiming unfinished milestones complete.

- Define an observable acceptance behavior before adding a capability. Prefer a
  complete, bounded vertical slice over unused infrastructure or empty crates.
- Keep `cortex-core` independent of infrastructure and the CLI. Evidence may
  depend on core; a CLI command may integrate both. Avoid circular dependencies
  and speculative public interfaces. The inspection adapter supplies typed state
  to Core binding; evidence integration remains a future increment.
- Use validated construction and typed errors at meaningful boundaries. Keep
  diagnostics useful without repeating private input. Preserve error causes
  where callers need them. Do not introduce unsafe Rust.
- Keep task identity, candidate identity, execution observations, semantic
  assessments, and human permission distinct. Never infer approval from an
  assessment or process success. Never label an unperformed check as a pass.
- Evidence must retain the relevant contract, candidate, mechanism, inputs, and
  configuration. Call out caller-supplied assertions and absent authentication.
  Changed inputs invalidate affected evidence; record a new observation rather
  than relabeling an old one. Do not overwrite terminal assessments.
- Any future runner needs explicit authorization, bounded resources, timeout and
  cancellation behavior, and defined terminal states before it executes work.
  This foundation provides no runner, permission engine, or enforcement sandbox.
- Preserve the free, operator-controlled baseline. Do not silently substitute a
  paid integration. Technical ambition does not expand the authorized scope.
- Add tests that can reject incorrect behavior: invalid inputs, boundaries,
  unsupported transitions, and actual CLI output/error behavior. Avoid tests that
  only prove a module or type exists. Keep local tests offline and deterministic.
- Keep crates unpublished, the repository private, and the license undecided
  until the owner makes an explicit decision. Do not enable telemetry, publish
  packages or releases, add paid services, or upload private planning materials.
- Minimize dependencies. Review origin, maintenance, license obligations, and
  concrete need before introducing one. Keep `Cargo.lock` tracked and use the
  pinned toolchain. Recheck the manual workflow when changing toolchain inputs.

Before handoff, run the verification commands in `README.md` against the final
candidate. Inspect test counts and failures. Review the staged diff, whitespace,
and final working tree. Record candidate identity, tool versions, actual commands,
results, unavailable tools, and limitations outside the repository when evidence
contains private operational details. A failed or unexecuted check remains so
until a new observation justifies a different conclusion.

The CI workflow is manual-only. Do not add automatic triggers or dispatch hosted
jobs before account usage and authorization are established. Do not describe a
workflow file as a passing remote pipeline. Handoffs must distinguish completed
work, observed verification, pending work, and risks; do not claim broad readiness
or independent review from a local self-review.
