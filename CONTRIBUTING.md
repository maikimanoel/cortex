# Contributing to CORTEX

CORTEX is pre-alpha. Milestones 001 and 002 are delivered. Milestone 003 has begun
with canonical history, bounded encrypted storage and JSON reference knowledge;
003-A/003-B remain incomplete, and 003-C–003-F and 004–006 remain planned.
Maiki Manoel maintains the project and reviews external contributions through
GitHub pull requests. Automated checks support that review. There is no CLA or
DCO sign-off requirement. Submit only material you have the right to contribute
under the project's [MIT License](LICENSE), preserving applicable third-party
notices. Repository access remains private until separately authorized launch.

## Set up and verify

Follow [Run locally](README.md#run-locally) for Linux prerequisites, the pinned
Rustup toolchain, source checkout, initial online `cargo fetch --locked`, and
offline build/CLI examples. A first-time empty cache needs that fetch; `cargo run`
does not install a global command. Work as a non-root user for permission tests.
Run the complete [verification commands](README.md#verify) from the workspace root:

```sh
cargo metadata --format-version 1 --locked --offline
cargo fmt --all -- --check
cargo check --workspace --locked --offline
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo test --workspace --locked --offline
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked --offline
```

Use `cargo test -p <crate> --locked --offline` for focused iteration; run the
workspace checks before submitting a code change. Test release behavior when the
change depends on optimization or debug assertions. Validate changed Markdown
links and examples, and run `git diff --check`. Report unavailable or failed checks
explicitly; a passing count is not a general correctness or security guarantee.

## Find the relevant contract

- `cortex-core`: deterministic task/candidate contracts and binding; no Git or CLI.
- `cortex-evidence`: observations and assessments, including independent result,
  applicability and validity; depends on Core.
- `cortex-cli`: command parsing, bounded Git observation, task-to-evidence
  integration and encrypted history infrastructure; depends on Core and Evidence.

Start with the [architecture overview](docs/architecture/overview.md),
[subsystem ownership](docs/architecture/subsystems.md),
[core invariants](docs/architecture/core-invariants.md) and
[engineering principles](docs/engineering-principles.md). The
[capability catalog](docs/product/capability-catalog.md) owns contracts and status;
the [roadmap](docs/product/roadmap.md) owns sequence. Relevant inspection, binding,
evidence and check contracts are linked from the README. Consult
[open decisions](docs/decisions/open-decisions.md) before changing a policy boundary.

## Propose and submit a focused change

For ordinary bugs, open a GitHub issue with the affected commit, relevant tool
versions, minimal reproduction, expected/actual behavior and sanitized diagnostics.
Do not include credentials, private repository contents or raw sensitive tool
output. Use the [security policy](SECURITY.md) for suspected vulnerabilities.
Discuss material feature or architecture proposals in an issue before starting
large changes; planned milestones are not authorization to implement them.

A pull request should explain the problem, intended behavior, affected contracts
or capability IDs where applicable, and actual verification results and limits.
Keep changes cohesive, maintainable and proportional to their risk. Preserve
dependency direction, resource/error semantics, read-only inspection, explicit
candidate identity, evidence dimensions and the separation of evidence from
permission. Do not infer PASS from missing execution or binding compatibility.

Add tests that can reject plausible incorrect behavior: negative and boundary
cases, regression tests for fixes, and integration or end-to-end tests across
consequential boundaries. Documentation-only changes need accurate links and
examples rather than artificial runtime tests. Keep tests local and deterministic.
Avoid speculative abstractions and dependencies; justify any necessary dependency.
Use English for code, comments and technical documentation. Keep packages
unpublished and leave releases and public visibility to explicit owner decisions.
Maintainer review and automated checks do not guarantee acceptance or response time.
