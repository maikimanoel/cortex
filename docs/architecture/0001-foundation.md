# 0001: A bounded deterministic foundation

Status: accepted for Milestone 001. This decision governs the initial foundation,
not a stable public API or the design of the complete platform.

## Responsibility and constraints

Cortex supports evidence-driven software engineering by placing deterministic
contracts and controls around work that may eventually involve probabilistic
agents. The platform must also be useful when a project's final application has
no AI. Its operational baseline must be local, CLI-oriented, and independent of
paid services. This milestone implements domain validation and a terminal entry
point; it does not perform engineering tasks.

## Modules and dependency direction

```text
cortex-evidence ──depends on──> cortex-core
cortex-cli (currently independent; may integrate both libraries when needed)
```

- `cortex-core` owns `TaskId`, `TaskContract`, `CandidateId`, and `ContractError`.
  It has no infrastructure, CLI, or third-party dependencies.
- `cortex-evidence` owns `Provenance`, `ExecutionObservation`, `Evidence`,
  `Assessment`, `Verdict`, and `EvidenceError`. Its only dependency is core.
- `cortex-cli` owns the `cortex` binary. It validates command arguments and prints
  help/version. It uses only the standard library at present.

The workspace uses Rust 2024 and resolver 3, with Rust 1.99.1 pinned to the locally
verified toolchain. All three packages inherit `publish = false`. A shared
`Cargo.lock` records the complete workspace resolution. There are no empty
roadmap crates, third-party Rust dependencies, build scripts, or runtime network
requirements.

## Domain contracts

A `TaskId` is an operator-assigned name of 1 through 128 ASCII bytes. Its first
character is alphanumeric; the rest may also contain `.`, `_`, and `-`. Names
are preserved without normalization. A `TaskContract` pairs that identity with
one objective and one observable acceptance description. Each description must
contain non-whitespace text, is limited to 4096 UTF-8 bytes, and excludes control
characters except line feed. Multiple criteria and task lifecycle management
are deferred. Changing a contract's meaning requires fresh relevant evidence even
if its operator-assigned name is reused; this library is not a uniqueness registry.

A `CandidateId` accepts full lowercase hexadecimal Git object IDs of 40 or 64
characters. It is separate from task identity. Construction checks syntax only:
it does not resolve a repository, verify object type or existence, establish a
clean checkout, or hash uncommitted files. Callers are responsible for associating
the identifier with the actual verified bytes. A candidate revision alone is
insufficient if tests ran against modified working files.

An `Evidence` value retains the entire supplied task contract, candidate,
provenance, and execution observation. `Provenance` requires mechanism, inputs,
and configuration descriptions, each nonblank and bounded to 4096 UTF-8 bytes with
the same control-character rule. These descriptions should identify relevant
tool versions, input identities, flags, and environment. They are caller claims,
not structured execution capture, secret detection, or authenticated records.

## Observations and assessment state

Execution status has a deliberately small vocabulary:

| Observation | Meaning |
| --- | --- |
| `NotRun` | No verification was attempted |
| `Exited { code }` | A caller reports a completed process and its nonnegative exit code |
| `TimedOut` | A time limit prevented a completed result |
| `Cancelled` | An interruption prevented a completed result |
| `Unavailable` | A prerequisite prevented verification from starting |

These are supplied reports, not states driven by a runner. Construction always
leaves evidence **unassessed**, including for `Exited { code: 0 }`.

The sole mutable transition is `unassessed -> assessed`. `assess` validates all
conditions before retaining an assessment, so a rejected call leaves the record
unchanged. Every judgment requires a nonblank rationale, limited to 4096 UTF-8
bytes with the same control-character rule.

| Requested verdict | Necessary execution condition |
| --- | --- |
| `Pass` | `Exited { code: 0 }` plus explicit reasoning |
| `Fail` | Any `Exited` report plus explicit reasoning |
| `Inconclusive` | Any reported status plus explicit reasoning |

These are conservative consistency rules. A process that exits zero can still
receive `Fail` when its output violates acceptance. Interrupted or unavailable
checks can only be inconclusive in this initial model; more specific resource
criteria require a future contract change. A nonzero exit cannot support pass in
this model; a negative test should report the completed test mechanism's result.
The library cannot determine the adequacy or honesty of a rationale. In particular,
a zero-test test invocation does not substantiate semantic acceptance merely
because the library permits recording an explicit pass.

An assessed record is terminal. There is no reassessment, observation replacement,
or candidate relabeling API. Create a new record when context changes; future
storage must preserve the relationship and history without overwriting reports.
No persisted format, unique evidence identity, evaluator identity, cryptographic
attestation, timestamps, or cross-record history mechanism is implemented yet.

## Authorization and trust boundaries

Task intent, source identity, execution reports, assessments, and human permission
are separate architectural responsibilities. No current type grants authorization,
and a verdict must never be consumed as an approval token. A future authorization
boundary must bind a human decision to an operation, target, limits, and relevant
candidate before consequential execution is introduced.

Constructors validate sizes and syntax; they do not trust-check observations.
Errors identify invalid fields and categories without echoing payloads. Owned
domain values can still contain private text, including in derived debug output;
callers must avoid secrets and must not log entire records indiscriminately.
Unknown CLI arguments are not echoed. The CLI reads at most two arguments for
dispatch and performs no subprocess execution or network access. Standard stream
writes can block under operating-system backpressure; no stream timeout is
implemented. Only the tests launch the CLI subprocess.

## Verification and delivery boundaries

Core tests distinguish valid construction from blank, malformed, control-bearing,
or oversized input. Evidence tests cover provenance retention, every
observation/verdict combination, rejected transitions, and terminal assessment
preservation. CLI integration tests inspect the actual executable's output and
exit statuses, including invalid arguments; a unit test exercises output failure.
See `README.md` for the required local verification commands.

CI has a manual `workflow_dispatch` trigger, read-only contents permissions, a
commit-pinned checkout action with credential persistence disabled, and a
10-minute timeout. It must not be dispatched before usage and hosted-execution
authorization are established. No remote execution is implied by the workflow's
presence. A pinned toolchain and lockfile constrain inputs; this milestone does
not claim byte-identical artifacts across machines or verified remote builds.

## Alternatives and consequences

Using unrestricted strings would make malformed identities and missing provenance
easy to retain. Validated newtypes and immutable fields provide useful local
invariants without a broad schema framework. Standard-library CLI parsing is
sufficient for two flags; reconsider a maintained parsing library when actual
subcommands make manual parsing burdensome. Text provenance avoids freezing a
speculative execution schema, at the cost of leaving its completeness to callers.

A full task state machine, event store, provider abstraction, graph, or orchestration
framework would add dependencies and policies without implemented consumers.
They are deferred. The small initial APIs may change as real vertical slices
expose missing requirements; no compatibility or persistence contract is promised.

## Extension boundaries

The next slice should introduce one useful local operation with a verifiable
contract and candidate binding. Repository intelligence, graphs, memory,
continuity, orchestration, execution harnesses, security policy, and empirical
evaluation remain separate future responsibilities. Add a module or crate when
an implemented requirement justifies it. Core must remain independent of agents
and presentation; adapters must not displace the deterministic boundary.

Before any runner, define explicit authorization, source/worktree verification,
limits, timeout/cancellation semantics, output handling, cleanup, provenance
capture, and acceptance interpretation. Before persistence, define stable identity,
format evolution, integrity, access, and history rules. Before paid integrations,
preserve a complete operator-controlled baseline and obtain separate authorization.
License selection is required before public release. Repository indexing, databases,
agents, background workers, package publication, and network services are outside
this milestone.
