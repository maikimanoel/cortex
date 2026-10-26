# Candidate-bound task contracts (002-B)

This is the bounded implementation contract for
[CAP-CORE-002](../product/capability-catalog.md#cap-core-002). It builds on
[002-A inspection](repository-inspection.md), preserving the foundation APIs and
[INV-001 through INV-005](core-invariants.md). It does not implement evidence
assessment, execution permission, task completion, persistent history or snapshots.
[002-C evidence](unified-evidence.md) and [002-D CLI check](cli-check.md) consume
this boundary without changing its semantics.

## Decision A: identities and contract revisions

`TaskId` identifies the operator's task. Existing `TaskContract` still retains one
objective and one acceptance criterion, with its original validation and API.
`ContractRevision` is a positive `u64` assigned by the caller. A
`CandidateBindingContract` retains the complete foundation task, its revision and
`CandidateRequirement`: expected local repository/worktree identity, required
`CandidateId` commit, and `WorktreeRequirement`.

A task/revision pair identifies a contract revision. Applicability additionally
requires equality of the entire captured contract, including task text and
candidate requirements. Reusing a task/revision pair with changed contents is an
invalid input, `BindingError::ContractRevisionReused`. Another revision is stale
for this request regardless of numeric ordering. Another task is incompatible.
Changing objective, criterion or candidate requirements requires a new revision
and new applicable observations. There is no global uniqueness/history registry:
only the supplied contexts can be checked for aliasing.

`CandidateId` keeps its foundation meaning: full Git object-ID syntax only. Actual
object existence/type is supplied by the inspection producer. It never identifies
dirty bytes. `RepositoryIdentity` identifies one local worktree using canonical
root, per-worktree Git directory and common Git directory. Linked worktrees and
copies are distinct even at the same commit. Core validates absolute path syntax
and bounds without filesystem I/O; the producer resolves/canonicalizes paths.
It does not establish inode identity or continuity across moves/replacements.

`ObservationId` contains a producer scope and positive sequence. Scope syntax is
1..=128 ASCII bytes: alphanumeric first, followed by alphanumeric, `.`, `_`, `-`.
Producers must use a scope for one capture series and never reuse its sequence.
These references are caller-supplied, unauthenticated and not globally registered.
They are neither source hashes nor timestamps. Contract revision is independent
of future persisted-format/schema versioning; no serialization is selected.

## Decision B: correspondence and freshness

`CandidateObservation` retains its observation ID, the entire binding contract
under which it was requested, validated `RepositoryState`, and optional opaque
`ComparisonBasis`. Captured contexts are immutable; old records are not relabeled.

`bind_candidate(contract, baseline, reobserved)` requires a baseline and a separate
reobservation from the same producer scope with a strictly larger sequence. It
checks exact contract applicability, repository and commit correspondence,
coverage, consistency, typed state and mechanism comparison material. Elapsed
wall-clock time is not used. Absence, an increased sequence alone, equal HEADs,
or equal dirty counts cannot establish correspondence.

The trusted integration must actually perform the reinspection. Core accepts
structured producer assertions and cannot authenticate them, detect fabricated
captures, or discover that an old record was relabeled outside this API. Even a
newly captured observation may become obsolete after capture. There is no
permanently current flag, TTL-based freshness rule or atomic snapshot guarantee.

`ComparisonBasis` contains a bounded mechanism/profile name and opaque equality
material. The CLI adapter supplies length-framed effective configuration, index
listing and porcelain status from the existing inspection, with Git version,
002-A profile and host OS/architecture. Core does not parse these Git formats.
Changed material/profile makes the comparison stale, including index/configuration
changes hidden by status categories. Missing material is insufficient.

Comparison material can contain private configuration; its `Debug` output hides
the material. It is retained only in memory, with no export/storage API. Task,
path and profile debug fields may still contain private data. It is not a hash,
content attestation or working-snapshot identity. Identical material cannot detect
uncommitted byte edits that retain the same status; such dirty candidates remain
unresolved. This preserves 002-A's declared races, ignore rules, normalization,
coverage exclusions and host trust assumptions.

## Supported states and decision semantics

`CandidateRequirement` always names a required committed revision. It selects:

- `Clean`: observed staged/unstaged/untracked changes violate the requirement.
- `AllowChanges`: changes are permitted in principle, but matching dirty status
  still cannot establish uncommitted content identity in 002-B.

`RepositoryState` preserves attached/detached/unborn/unknown HEAD and sorted
root-relative `Entry` values with independent staged and unstaged `Change`
categories or untracked state. Constructors reject empty/control-bearing branch
names, invalid/traversing/NUL-bearing paths, unchanged tracked records, duplicate
paths and resource overflow. Git-specific object/ref validation remains with the
producer; structural validation is not proof that reports are true.

| Input/condition | Typed result |
| --- | --- |
| Matching clean commit and repository; applicable ordered observations; complete Git status coverage and matching consistency/material | `Compatible(BoundCandidate)` within that coverage only |
| Known different repository, commit or task | `Incompatible` with the corresponding reason |
| Unborn branch where a commit is required | `Incompatible(CommitAbsent)`; no commit is fabricated |
| Unknown repository or HEAD | `Insufficient(MissingRepository/MissingCommit)` |
| Different contract revision | `Stale(ContractRevision)` |
| Same task/revision but changed task text or candidate requirements | Invalid input: `Err(ContractRevisionReused)` |
| Missing baseline/reobservation or incomparable producer scopes | `Insufficient` with the corresponding reason |
| Reused or out-of-order observation reference | `Stale(ObservationOrder)` |
| Incomplete coverage, indeterminate consistency or missing comparison material | `Insufficient` with the corresponding reason |
| Changed typed state or comparison material | `Stale(ObservedStateChanged)` |
| Matching dirty observations under `Clean` | `Incompatible(WorkingTree)` |
| Matching dirty observations under `AllowChanges` | `Insufficient(UncommittedIdentity)` |
| Malformed revision, observation ID, repository identity or state | Constructor `BindingError`; no accepted value retained |

The deterministic priority is context applicability, observation ordering,
candidate facts (reobservation then baseline), coverage/consistency, comparison
availability, changed state, dirty policy, then scoped compatibility. This returns
one reason, not every possible problem. Detached HEAD can bind using its commit;
branch existence is not required. A changed branch/detached state between captures
is conservatively stale even if the commit is unchanged.

`BoundCandidate` retains the baseline reference and full later observation,
including contract, requirements, candidate facts, coverage, consistency and
comparison profile/material. It is an association supported by those captures,
not a new complete-candidate ID. Ignored files, byte identity outside Git semantics,
unobserved changes, software correctness and execution authority are not established.
Binding decisions are domain correspondence results, not `Verdict`, `PASS` or a
conversion of foundation evidence. [DEC-010](../decisions/open-decisions.md#dec-010)
is resolved for the separate 002-C model; binding still never creates an assessment.

## Decision C: the Core and inspection boundary

[`cortex-core::binding`](../../crates/cortex-core/src/binding.rs) owns validated
identities, requirements, shared `Head`/`Change`/`Entry` state, observations and
binding decisions. It uses only the standard library, with no filesystem access,
Git process, clock, CLI or remote service dependency. Existing task/evidence APIs
remain compatible. There is one shared state model, moved from the CLI rather
than reimplemented alongside it.

The existing `cortex-cli` package now has a small library entry point exposing its
existing inspection and `inspect_candidate(path, contract, observation_id)`.
That adapter always runs the existing bounded inspection before translation. It
never parses terminal output, accepts caller-reported state in place of inspection,
or turns failed inspection into a successful observation. It preserves error
causes at the translation boundary. No new command, crate, Rust dependency or Git
query was added. The existing executable consumes that same library implementation.

For a caller that already has a validated binding contract:

```rust
let baseline = inspect_candidate(path, &contract, ObservationId::new("local-run", 1)?)?;
let reobserved = inspect_candidate(path, &contract, ObservationId::new("local-run", 2)?)?;
let decision = bind_candidate(&contract, Some(&baseline), Some(&reobserved))?;
```

Each capture separately retains all 002-A subprocess, output and time limits.
Core input bounds are 10,000 changed entries, 4 MiB aggregate change-path bytes,
and 4 MiB per repository identity/branch field. Comparison profiles are at most
4096 UTF-8 bytes; material is at most three 4 MiB capture buffers plus 24 framing
bytes. These bounds do not promise total process RSS or elapsed binding time.

## Acceptance mechanisms and evidence

[Core binding tests](../../crates/cortex-core/tests/binding.rs) exercise matching
and mismatched candidates, missing facts, stale/order/version cases, revision
aliasing, dirty policy, incomplete/indeterminate coverage, constructor boundaries,
comparison redaction and deterministic repetition.
[Cross-crate real-Git tests](../../crates/cortex-cli/tests/binding.rs) exercise
actual Git -> adapter -> Core binding, including clean/mixed/dirty/detached/unborn,
SHA-256, non-UTF-8 byte paths, linked-worktree identity, changed commit/config/index,
same-status dirty
byte edits, failure propagation, revision applicability and repeated read-only
capture with a refresh-sensitive index. Existing 002-A CLI/read-only/resource
regressions and foundation/evidence tests remain required.

These sources are verification mechanisms. Actual command outcomes, counts,
versions, candidate tree and private delivery observations belong to the external
candidate-scoped handoff. This contract claims no independent verification,
authenticated producer, cross-platform acceptance, complete snapshot or hosted CI
result. The catalog records CAP-CORE-002 as `IMPLEMENTED`; 002-D composes this
contract through the existing adapter, without identifying dirty snapshots.
