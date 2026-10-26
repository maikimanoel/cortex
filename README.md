# Cortex

Cortex is a software engineering platform designed to turn human intent into
constructed, verified, and continuously improved solutions by integrating
intelligence, execution, knowledge, and responsibility into one coherent
engineering process. Its deterministic Rust core defines contracts, source
identity, and the conditions under which observations can support an engineering
judgment. Future agents may propose or perform authorized work within those
boundaries. Projects engineered with Cortex do not need to contain AI.

**Milestone 001 is a foundation, not an execution platform.** The current binary
prints help and version information. The libraries validate in-memory contracts
and evidence records. There are no agents, runners, persistence, providers,
network services, background workers, or telemetry.

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

Milestone 002 is planned, not implemented: actual read-only Git inspection,
task/candidate binding, unified evidence, and end-to-end CLI inspection are four
required increments. An inspection command alone will not complete it.

## Run locally

Install the toolchain specified in `rust-toolchain.toml` using Rustup. Rust 1.99.1
with rustfmt and Clippy is pinned; Rust 2024 and Cargo resolver 3 are used.
Once that toolchain is installed, this workspace has no third-party Rust
dependencies and builds offline.

```sh
cargo run --offline --locked -p cortex-cli -- --help
cargo run --offline --locked -p cortex-cli -- --version
```

The executable is named `cortex`. No arguments and `-h` also print help; `-V`
prints the version. Unsupported or additional arguments exit with code 2 without
echoing their values. Output failures exit with code 1. Successful help/version
requests exit with code 0. These commands do not execute engineering tasks.

## Workspace

| Crate | Current responsibility |
| --- | --- |
| `cortex-core` | Validated task identities, single-criterion task contracts, and full Git source revision syntax |
| `cortex-evidence` | Task/candidate-bound provenance, reported execution status, and explicit assessments |
| `cortex-cli` | The terminal help/version interface |

Only `cortex-evidence` currently depends on `cortex-core`. The CLI is independent
until a command needs the libraries. No library depends on the CLI or an agent
framework. All packages have `publish = false`; version 0.1.0 identifies this
private foundation and carries no stable API commitment.

An execution report and a semantic judgment are different values. A newly
constructed evidence record has no assessment, even when its exit code is zero.
Pass requires an explicitly supplied rationale and a completed zero-exit report.
These checks cannot establish whether the supplied report or reasoning is true.
An assessment grants no human authorization.

## Verify

```sh
cargo metadata --format-version 1 --locked
cargo fmt --all -- --check
cargo check --workspace --locked
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --locked
cargo run --locked -p cortex-cli -- --help
cargo run --locked -p cortex-cli -- --version
git diff --check
git diff --cached --check
git status --short
```

Behavioral tests exercise the CLI as a subprocess, input boundaries, provenance
retention, and the observation/assessment rules. Passing tests support these
scoped contracts, not comprehensive correctness or security certification.

The GitHub Actions workflow has only `workflow_dispatch`, read-only repository
permissions, and a bounded job timeout. It is prepared for later operator use;
its presence does not establish a successful remote run. Before dispatching it,
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
private. License selection is deferred until before any public release; no
license grant or license identifier is supplied by this milestone. Do not publish
packages, create releases, or change visibility without separate authorization.
