# Cortex

Cortex is a software engineering platform designed to turn human intent into
constructed, verified, and continuously improved solutions by integrating
intelligence, execution, knowledge, and responsibility into one coherent
engineering process. Its deterministic Rust core defines contracts, source
identity, and the conditions under which observations can support an engineering
judgment. Future agents may propose or perform authorized work within those
boundaries. Projects engineered with Cortex do not need to contain AI.

**Pre-alpha.** The source implements the bounded capabilities below; it is not
production-ready and makes no general API-stability or cross-platform guarantee.
Milestone 003 has begun with canonical history, encrypted storage and bounded JSON
reference knowledge, including selected source sets. Its remaining capabilities
and Milestones 004–006 remain planned. The owner-approved open-source scope is an
MIT source-only pre-alpha preview, including these incomplete increments. It
includes no binary release, package publication or stable API commitment.

**Milestone 002 delivers a bounded task-to-evidence CLI slice.**
`cortex inspect` observes repositories; `cortex check` evaluates committed-HEAD
equality with explicit task/revision and candidate-bound evidence. Help/version
remain available. The libraries retain foundation APIs and independently represent
assessment result, applicability and contextual evidence validity. There are no agents,
task runners, providers, network services, background workers or telemetry. A bounded
Linux library adapter now persists explicitly submitted canonical history through
SQLCipher; it adds no storage command or automatic collection.

## Product blueprint

The planned platform combines typed operational authority, mandatory evidence,
and temporal provenance. These are product requirements; the foundation does not
yet implement permission enforcement or general persistent memory. Checkpoint 001A records
the blueprint; 003-A now supplies deterministic in-memory
[requirement/decision history](docs/architecture/temporal-records.md) in Core and
its bounded encrypted storage adapter.

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
correctness. The first 003-A library slice adds temporal records; remaining
increments require separate authority.

The planned [native governed parallel contract](docs/architecture/governed-parallel-engineering.md)
defines per-consumer dependency release and rolling integration under the existing
owners. Its first Rust slice, `cortex_core::parallel`, implements a bounded,
non-effectful in-memory readiness projection over caller-supplied assertions.
It reports per-edge release/hold/refusal and mission eligibility with causal
invalidation; it authenticates no Evidence and grants no execution permission.
There is no scheduler or agent launcher. The
[roadmap](docs/product/roadmap.md#native-parallel-delivery-sequence) records the
owner-approved G1-only preview exception and full execution prerequisites.

## Run locally

Use Linux with Rustup, Git supporting `--no-lazy-fetch`, and util-linux
`/usr/bin/prlimit`. Tests should run as a non-root user for permission fixtures.
Rust 1.99.1 with rustfmt and Clippy is pinned in `rust-toolchain.toml`; Rust 2024
and Cargo resolver 3 are used. Other platforms are not currently supported by
repository inspection.
The Linux storage build also needs a C compiler, `pkg-config`, and stable OpenSSL 3
development headers/libraries. Check `cc --version` and `pkg-config --modversion openssl`.
SQLCipher Community is compiled from the locked source dependency and links system
libcrypto; no beta cipher or network service is needed at runtime. Qualification
currently covers Linux x86_64 with OpenSSL 3.6.5, not every OpenSSL 3 host.

Obtain the source and bootstrap the toolchain and dependency cache with network
access:

```sh
git clone https://github.com/maikimanoel/cortex.git
cd cortex
rustup toolchain install 1.99.1 --profile minimal --component rustfmt --component clippy
cargo fetch --locked
cargo build --workspace --locked --offline
```

An empty Cargo cache cannot run the offline commands until the locked fetch
succeeds. Repeat the fetch when required dependencies change. After setup, the
workspace builds and verifies offline. The CLI library uses JSON/SQLCipher and small
key/host helpers; Core and Evidence still use only the standard library and the
existing Core dependency.
No runtime network service is required. From the workspace root:

```sh
cargo run --offline --locked -p cortex-cli -- --help
cargo run --offline --locked -p cortex-cli -- --version
cargo run --offline --locked -p cortex-cli -- inspect
cargo run --offline --locked -p cortex-cli -- inspect --repo "/path/to/repository"
cargo run --offline --locked -p cortex-cli -- check --help
```

For a bounded task in the current repository, use its actual full commit ID as
both subject and expected revision:

```sh
cortex_subject_commit=$(git rev-parse --verify HEAD)
cargo run --offline --locked -p cortex-cli -- check --task release.head --revision 1 \
  --candidate "$cortex_subject_commit" --expect-head "$cortex_subject_commit"
cargo run --offline --locked -p cortex-cli -- check --json --task release.head --revision 1 \
  --candidate "$cortex_subject_commit" --expect-head "$cortex_subject_commit"
```

These examples can PASS in a supported clean checkout; a dirty checkout remains
unresolved. To inspect another repository, add `--repo "/path/to/repository"`
and obtain the subject commit from that repository. Change `--expect-head` to
the full lowercase commit ID required by the criterion when testing a different
expectation. `cargo run` builds and runs the executable; it does not install a
global `cortex` command. Interface documentation uses `cortex` as the executable
name; the source-tree prefix above works without installation.

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
| `cortex-core` | Foundation contracts, typed candidate/task binding, canonical temporal records, immutable knowledge projections and advisory G1 readiness |
| `cortex-evidence` | Legacy reports plus unified candidate-bound result, applicability, validity and redacted library inspection |
| `cortex-cli` | Existing commands, bounded Git inspection, knowledge acquisition and encrypted history infrastructure |

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

The [encrypted-history decision](docs/architecture/0002-durable-history.md) defines
the new `cortex_cli::history_store` library interface. The operator supplies an
explicit private location outside the source repository, a `StoreKey` containing
32 random raw-key bytes and typed Core data. Keys are never command-line arguments
or stored beside the database. Create/open, explicit recovery, admission,
reconciliation and requested whole-collection removal are bounded operations.
No default location, automatic capture, backup, sync, export or lost-key recovery
exists. Restoring historical data does not refresh evidence or grant authority.

The [first knowledge operation](docs/architecture/knowledge-references.md) is
`cortex_cli::knowledge::extract_json_references`: select one explicit committed JSON
file, inspect exact source spans/local `$ref` relationships and query incoming
references through Core. `reinspect` reports contextual current/stale/unestablished
validity; relationships remain extracted and unverified. This is a library interface,
not a new command, full JSON Schema evaluator or multi-language incremental index.

The additive `cortex_cli::knowledge::source_set::extract_json_source_set` selects
1–8 committed JSON files under one before/after candidate binding. Its Core
`ReferenceSet::incoming(&target_path, "/Id")` query identifies both the target file
and locator, retaining original spans and provenance. Only supported relative
references within the explicitly selected set resolve; no other file is discovered.
The single-source API still leaves external-document references unresolved. See the
[source-set contract](docs/architecture/knowledge-references.md#additive-source-set-contract)
for lexical coverage, aggregate bounds and contextual validity limitations.

The library also supports [producer-owned JSON refresh and snapshot deltas](docs/architecture/knowledge-references.md#producer-owned-json-refresh).
Refresh reacquires every selected blob and binds a new generation, reusing parsing
only for exact supported path/byte dependencies. The Core delta query compares
retained snapshots with matching declared profiles; it does not establish semantic
equivalence or freshness. [Interval timelines](docs/architecture/temporal-records.md#interval-timelines)
inspect one canonical entity across a finite domain window and recording cutoff.
These are bounded library extensions; full incremental invalidation, knowledge
persistence, broader memory layers and Milestone 003 remain incomplete.

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
cargo metadata --format-version 1 --locked --offline
cargo fmt --all -- --check
cargo check --workspace --locked --offline
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo test --workspace --locked --offline
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked --offline
cargo run --locked --offline -p cortex-cli -- --help
cargo run --locked --offline -p cortex-cli -- --version
cargo run --locked --offline -p cortex-cli -- inspect
cargo run --locked --offline -p cortex-cli -- check --help
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
Core temporal tests additionally exercise immutable corrections, independent
domain/recording times, historical cutoffs, atomic rejection, limits and redaction;
real-Git consumer tests preserve capture state and unchanged evidence. Core history
alone is not durable, live verification or operational permission. The encrypted
adapter's fault tests additionally require a dynamically linked Linux/glibc test
binary, Bash and the existing C compiler/prlimit prerequisites. They compile a
test-only process-local shim and exercise child-only file-size limits. Injected
sync errors and process exits do not establish power-loss durability; see the
[qualification boundary](docs/architecture/temporal-records.md#storage-failure-qualification).
Passing tests support these scoped contracts, not comprehensive correctness or
security certification.

The [GitHub Actions workflow](.github/workflows/ci.yml) checks pull requests to
`main`, pushes to `main`, and manual requests in one standard Linux job named
`Rust workspace`. It uses read-only repository permissions, a pinned checkout
without persisted credentials, and a ten-minute timeout. It fetches locked Cargo
dependencies before offline checks, including warning-free Rustdoc. No secrets,
privileged pull-request trigger, caches or uploaded artifacts are configured.
Hosted execution requires authorized included capacity; it must not incur
unapproved charges. Inspect the actual run and commit before claiming CI success.

## Contributing and security

See [CONTRIBUTING.md](CONTRIBUTING.md) for focused pull requests, local checks and
maintainer review. Report ordinary bugs and discuss proposed features through
GitHub issues. For suspected vulnerabilities, follow [SECURITY.md](SECURITY.md)
and avoid public disclosure. The maintainer must enable and verify GitHub Private
Vulnerability Reporting before directing public reporters to that route. Follow
the policy's instructions if the route is unavailable.

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

The official source project is [maikimanoel/cortex](https://github.com/maikimanoel/cortex).
The owner has authorized its controlled public source-only pre-alpha transition
under [DEC-009](docs/decisions/open-decisions.md#dec-009), subject to verified
privacy and launch controls. Authorization is distinct from observed provider
state. The separate historical repository and private evidence remain private.
Cortex is licensed under the [MIT License](LICENSE), copyright (c) 2026 Maiki
Manoel. Third-party dependencies retain their own applicable license terms.
See [third-party notices](THIRD_PARTY_NOTICES.md) for the linked storage components.
All workspace packages remain unpublished. Do not publish packages, create releases,
or change visibility outside explicit owner authorization. Source visibility does
not complete Milestone 003, Milestone 007 or CAP-CLI-006.
