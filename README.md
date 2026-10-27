# Cortex

Cortex is a software engineering platform designed to turn human intent into
constructed, verified, and continuously improved solutions by integrating
intelligence, execution, knowledge, and responsibility into one coherent
engineering process. Its deterministic Rust core defines contracts, source
identity, and the conditions under which observations can support an engineering
judgment. Future agents may propose or perform authorized work within those
boundaries. Projects engineered with Cortex do not need to contain AI.

**Milestone 002 delivers a bounded task-to-evidence CLI slice.**
`cortex inspect` observes repositories; `cortex check` evaluates committed-HEAD
equality with explicit task/revision and candidate-bound evidence. Help/version
remain available. The libraries retain foundation APIs and independently represent
assessment result, applicability and contextual evidence validity. There are no agents,
task runners, persistence, providers, network services, background workers or telemetry.

## Product blueprint

The planned platform combines typed operational authority, mandatory evidence,
and temporal provenance. These are product requirements; the foundation does not
yet implement permission enforcement or temporal history. Checkpoint 001A records
the blueprint without changing runtime behavior.

| Document | Canonical responsibility |
| --- | --- |
| [Vision](docs/product/vision.md) | Mission, audience, operating model, ambition and demonstrated limits |
| [Capability catalog](docs/product/capability-catalog.md) | Stable capability IDs, functional contracts, advanced properties, status and acceptance |
| [Roadmap](docs/product/roadmap.md) | Milestones 001–007, 002-A/B/C/D, dependencies, exits and future backlog |
| [Architecture overview](docs/architecture/overview.md) | Current crates versus planned areas, flow and integration boundaries |
| [Core invariants](docs/architecture/core-invariants.md) | Authority, evidence, candidate, temporal and trust semantics |
| [Subsystems](docs/architecture/subsystems.md) | Logical ownership and interaction responsibilities |
| [Open decisions](docs/decisions/open-decisions.md) | Unresolved policies and proposed decision points |
| [Foundation decision](docs/architecture/0001-foundation.md) | Preserved accepted design of the current implementation |

002-A/B/C/D implement the bounded first slice. See the
[inspection contract](docs/architecture/repository-inspection.md),
[binding contract](docs/architecture/candidate-binding.md),
[evidence contract](docs/architecture/unified-evidence.md) and
[CLI check contract](docs/architecture/cli-check.md) for scope, limits and acceptance
mechanisms. `cortex inspect` continues to create no binding or assessment. `check`
verifies one explicit Git proposition; it does not establish general software
correctness. Milestones 003 and later remain planned, requiring separate authority.

## Run locally

Install the toolchain specified in `rust-toolchain.toml` using Rustup. Rust 1.99.1
with rustfmt and Clippy is pinned; Rust 2024 and Cargo resolver 3 are used.
With that toolchain and the locked dependency cache installed, the workspace
builds offline. The CLI uses a JSON serializer; Core and Evidence still use only
the standard library and the existing Core dependency. No runtime network service
is required.

```sh
cargo run --offline --locked -p cortex-cli -- --help
cargo run --offline --locked -p cortex-cli -- --version
cargo run --offline --locked -p cortex-cli -- inspect
cargo run --offline --locked -p cortex-cli -- inspect --repo "/path/to/repository"
cargo run --offline --locked -p cortex-cli -- check --help
```

For a bounded task, supply full lowercase commit IDs:

```sh
cortex check --task release.head --revision 1 \
  --candidate "$SUBJECT_COMMIT" --expect-head "$REQUIRED_COMMIT" --repo "/path/to/repository"
cortex check --json --task release.head --revision 1 \
  --candidate "$SUBJECT_COMMIT" --expect-head "$REQUIRED_COMMIT" --repo "/path/to/repository"
```

The candidate pin identifies the subject, while expected HEAD is the checked
proposition. Check exits 0 only for CURRENT PASS, 8 for CURRENT FAIL, and 9 when
unresolved; operational failures retain codes 1..7. `--json` must immediately
follow `check`. Schema version 1 preserves dimensions, identities, provenance and
limits; task revision is a decimal string. See the [full interface](docs/architecture/cli-check.md).
A stable dirty candidate remains UNKNOWN/UNESTABLISHED, even with matching HEAD.

The executable is named `cortex`. No arguments and `-h` also print help; `-V`
prints the version. Unsupported or additional arguments exit with code 2 without
echoing their values. Output failures exit with code 1. Successful help/version
requests exit with code 0.

Inspection currently requires Linux, Git supporting `--no-lazy-fetch`, and
util-linux `/usr/bin/prlimit`. It selects HEAD in the current directory or the
specified working tree, including subdirectories and linked worktrees. It reports
canonical locations, resolved commit, branch/detached/unborn state and separate
staged, unstaged and untracked counts. Dirty inspection also exits zero; this is
an observation, never software verification or a complete working candidate ID.
Ignored paths are excluded, and two matching observations are not an atomic
snapshot. Unsupported forms and incomplete observations fail without a report.

Each Git process has a five-second deadline, 512 MiB address-space ceiling,
4 MiB stdout cap and 16 KiB stderr cap. An inspection has a 30-second observation
budget, at most 32 Git queries, and at most 10,000 tracked and 10,000 changed
entries per sample. Cleanup may add one second. See the
[full limits and trust boundary](docs/architecture/repository-inspection.md#finite-budgets);
these are not sandbox or hard real-time guarantees.

## Workspace

| Crate | Current responsibility |
| --- | --- |
| `cortex-core` | Foundation contracts, explicit contract revisions, typed repository state and scoped candidate/task binding |
| `cortex-evidence` | Legacy reports plus unified candidate-bound result, applicability, validity and redacted library inspection |
| `cortex-cli` | Help/version, bounded Git inspection, typed adapter and human/JSON task-to-evidence check |

The production direction is CLI -> Evidence -> Core, with CLI also using Core.
The inspection
adapter supplies actual observations to Core's binding boundary. Foundation
`TaskContract`, `CandidateId` and evidence APIs retain their meanings. A binding
retains its exact task revision and scoped observations; it grants no authority,
does not assess software and does not identify dirty bytes. Reobservation ordering
is producer-supplied, and trusted integration must actually capture current state.
Core and Evidence do not depend on the CLI. Real-Git evidence integration tests
live in the CLI consumer, preserving an acyclic graph. All packages have
`publish = false`; version 0.1.0 carries no general stable library API commitment.
The narrowly versioned CLI JSON projection is documented separately.

An execution report and a semantic judgment are different values. A newly
constructed evidence record has no assessment, even when its exit code is zero.
The legacy API retains its original zero-exit/rationale preconditions and terminal
`Inconclusive`. The separate unified model additionally requires applicable,
CURRENT context, positive relevant checks and an explicit matching oracle for PASS.
UNKNOWN, NOT_RUN, BLOCKED and examined N/A remain distinct. Known invalidation is
sticky; a historical PASS can be STALE. Missing validity is UNESTABLISHED. No
conversion invents legacy metadata. These checks cannot authenticate caller
reports or establish the truth of their reasoning. Assessments grant no authority.

## Verify

```sh
cargo metadata --format-version 1 --locked
cargo fmt --all -- --check
cargo check --workspace --locked
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --locked
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked
cargo run --locked -p cortex-cli -- --help
cargo run --locked -p cortex-cli -- --version
cargo run --locked -p cortex-cli -- inspect
cargo run --locked -p cortex-cli -- check --help
git diff --check
git diff --cached --check
git status --short
```

Behavioral tests exercise the CLI as a subprocess, real temporary Git repositories,
read-only content/metadata invariance, resource failures, concurrent-change rejection,
input boundaries, provenance retention and observation/assessment rules. Binding
tests cover invalid revisions, stale/mismatched/incomplete inputs and real-Git
translation, including dirty bytes hidden by identical status. Unified evidence
tests cover independent dimensions, unsupported success, exact context/version
mismatch, sticky invalidation, legacy compatibility, redaction and real Git reports.
Actual `check` subprocess tests cover supported/refuted criteria, parsed JSON,
non-successful binding, explicit errors and repeated read-only operation.
Inspection permission fixtures require a non-root Linux user; all fixtures are local/offline.
Validate relative links and anchors in changed Markdown documentation as well.
Passing tests support these scoped contracts, not comprehensive correctness or
security certification.

The GitHub Actions workflow has only `workflow_dispatch`, read-only repository
permissions, and a bounded job timeout. It is prepared for later operator use;
its presence does not establish a successful remote run. It fetches locked Cargo
dependencies before offline checks. Before dispatching it,
establish account usage availability and authorization for hosted execution.

## Engineering commitments

The operational baseline must work without a paid AI provider, subscription,
managed database, or hosted infrastructure. Local computation still consumes
hardware, energy, storage, and operator time. Optional paid integrations require
separate authorization and must not displace the free baseline.

See [engineering principles](docs/engineering-principles.md),
[the foundation architecture](docs/architecture/0001-foundation.md), and
[agent instructions](AGENTS.md). Keep private planning, credentials, execution
payloads, and reference materials outside this Git repository.

## Repository status and licensing

The official destination is the private repository `maikimanoel/cortex`. Keep it
private. CORTEX is licensed under the [MIT License](LICENSE), copyright (c) 2026
Maiki Manoel. Third-party dependencies retain their own applicable license terms.
All workspace packages remain unpublished. Do not publish packages, create releases,
or change visibility without separate authorization.
