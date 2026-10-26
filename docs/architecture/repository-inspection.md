# Bounded repository inspection (002-A)

This is the implemented scope of [CAP-CLI-002](../product/capability-catalog.md#cap-cli-002).
It resolves the inspection portions of [DEC-005](../decisions/open-decisions.md#dec-005)
and [DEC-011](../decisions/open-decisions.md#dec-011). It does not bind tasks, create
snapshot identities, persist evidence, assess software, or grant permission.
Milestones 002-B/C/D remain planned. The accepted foundation APIs are unchanged.

## Interface and meaning

```sh
cortex inspect
cortex inspect --repo "/path/with spaces/repository"
cargo run --offline --locked -p cortex-cli -- inspect
```

The default input is the current directory. `--repo` takes exactly one directory
path, including relative paths. No revision argument is accepted: inspection
selects the actual local `HEAD`. Existing help/version forms remain available;
use `cortex --help` for command usage. Invalid/extra arguments are rejected before
inspection, without repeating their values.

Output includes canonical working-tree, Git-directory and common-Git-directory
locations, full resolved commit ID, branch or detached/unborn state, and separate
staged/unstaged/untracked path counts. Paths and branch names are quoted and
escaped for terminal display. The private typed model retains path bytes and
change categories; downstream code need not parse the text. There is no public
serialization or stable library API for observations in this increment.

For example, the essential portion of a dirty report is:

```text
HEAD: <actual full local commit ID>
Branch: "main"
Working tree: DIRTY
Staged: 1
Unstaged: 1
Untracked: 1
```

A path changed in both index and worktree contributes to both counts. Rename
inference is disabled: a staged rename is an addition plus a deletion. A clean
result says `CLEAN (Git status coverage)`. An unborn branch says `HEAD: NONE
(unborn)`; detached HEAD retains its commit. HEAD object type and resolution are
checked locally, rather than accepting a caller's object-ID string. Missing or
non-commit objects cannot produce successful inspection.

Every successful report explicitly states:

- Coverage includes tracked and non-ignored untracked paths; ignored paths are
  excluded. Empty directories have no Git status entry.
- Two observations matched; this is not an atomic snapshot.
- HEAD identifies the committed revision only; no working snapshot identity exists.
- Assessment is absent: inspection does not verify software.

The mechanism line includes the observed Git version, compiled host OS/architecture,
and fixed `002-A-v1` inspection profile. It is descriptive, unauthenticated
provenance, without task binding, timestamps, persistence or evidence assessment.

## Supported scope and prerequisites

The initial host is Linux with an operator-trusted `git` on `PATH` and the
util-linux `/usr/bin/prlimit` executable. Git must support porcelain v2,
`--path-format=absolute`, and `--no-lazy-fetch`; incompatible Git fails explicitly.
Acceptance was exercised with Git 2.56.0 and util-linux 2.42.4 on Linux x86_64.
Other hosts are explicitly unsupported; no cross-platform result is claimed.

Ordinary and linked working trees, clean and modified repositories, detached HEAD,
unborn branches, subdirectory invocation, SHA-1 and SHA-256 commits, spaces,
Unicode, and Unix non-UTF-8 paths are covered. NUL-delimited records preserve
filename boundaries, including newlines, tabs and shell metacharacters.

The following return an unsupported result, never an empty clean report:

- Bare repositories and invocation inside Git metadata outside the working tree.
- Gitlinks/submodules, unexpanded untracked directories/nested repositories,
  unresolved merge entries, sparse checkouts and skip-worktree entries.
- Assume-unchanged index entries, unknown file modes, configured content filters,
  and partial-clone/promisor configurations.

Unknown/malformed status records are indeterminate. Repository discovery and
object failures, warnings indicating incomplete traversal, and denied access
produce errors. Git can sometimes establish that an unreadable tracked file is
modified from its metadata; that observation can still be reported as dirty.

## Read-only mechanism and trust boundary

The CLI invokes a fixed set of Git queries with structured arguments through
`prlimit`: version, repository discovery, effective configuration, index listing,
porcelain status, HEAD type/resolution and unborn-ref absence. It invokes no shell
and performs no checkout, staging, reference/configuration writes, object writes,
fetch, or other network operation. Git objects are not replaced by replace refs.

The inspection profile removes inherited `GIT_*` overrides (including index,
repository, config injection and tracing). It disables system/global configuration
and system/global attributes. Repository ignore rules apply, but the configured
user excludes file is disabled. Consequently coverage can differ from a user's
interactive Git configuration; ignored files are never claimed to be observed.

It disables optional locks/index refresh, filesystem monitoring, hooks,
untracked-cache use, external diff, rename inference, submodule recursion and
automatic maintenance. It disables lazy retrieval and denies all Git transports.
Configuration and index preflight rejects content filters and unsupported index
forms before status runs. Any captured stderr, including a warning, prevents a
complete report; raw Git output/configuration is not copied into diagnostics.

These are controls on an installed trusted Git, not an enforcement sandbox.
An adversary replacing Git, repository configuration, attributes or filesystem
mappings concurrently is outside this contract. In particular, configuration
preflight and status are not atomic: hostile configuration replacement could
introduce a new filter after preflight. The command does not claim to contain
that adversary. Use a separate effective host sandbox before inspecting such
actively hostile targets. Ordinary reads may update filesystem access times.

## Finite budgets

| Resource | Fixed limit per invocation |
| --- | --- |
| Each Git process, including pipe completion | 5 seconds |
| Whole inspection observation interval | 30 seconds |
| Git query operations | 32 maximum; no automatic retries |
| Each process stdout | 4 MiB |
| Each process stderr | 16 KiB; never emitted raw |
| Tracked index entries per sample | 10,000 |
| Changed/untracked entries per sample | 10,000 total |
| Git process virtual address space | 512 MiB via Linux `RLIMIT_AS` |
| Git core dump size | Zero |
| Direct-child cleanup polling after interruption | Up to 1 additional second |

Capture readers use fixed 8 KiB chunks and stop on overflow. Only two samples
and bounded capture/parser material are retained. Limit exhaustion suppresses the
entire report; no truncation is presented as complete. The address-space limit
also restricts Git's allocation before output is emitted. Allocation failures
reported by Git remain command failures when their cause cannot be established;
Cortex does not invent a resource diagnosis from a generic nonzero exit.

`prlimit` is a local runtime prerequisite, not a linked Rust dependency. It supplies
a resource control unavailable through safe Rust standard-library pre-exec APIs.
It execs Git in the direct child, preserving timeout cleanup ownership. The tool is
provided by the maintained util-linux project; its installed package licensing and
origin were inspected for this use. Cortex does not bundle, modify, distribute or
select a license for it. The inspection does not install software or contact a service.

Timeouts kill the direct child and poll for reaping. A pipe held by an unexpected
descendant does not extend the capture deadline; this is not process-tree isolation.
The CLI exits on failure, closing remaining capture threads. There is no task
runner or cancellation API. Kernel-blocked filesystem/process calls, scheduler
starvation and terminal backpressure are not hard real-time bounded by this
mechanism. Local responsive filesystems and ordinary scheduling are assumed.
These defaults are limits, not throughput or total-RSS performance guarantees.

## Consistency and remaining coverage limits

Each sample discovers the root/Git/common directories, reads effective configuration
and index records, obtains full porcelain status, and checks HEAD. Two complete
samples must match those materials, including branch, commit, index object IDs and
path/status records. Observed differences return a retryable inconsistency error.
A controlled test inserts a real untracked file between the samples and exercises
this rejection path.

This detects changes visible in those observations, not every filesystem edit.
Edit-and-revert races, content changes that retain the same dirty status, metadata
spoofing/stat-cache limitations, ignore-rule races, and changes after the final
sample can escape detection. No content hash or atomic snapshot of uncommitted
files is produced. Git status semantics, including file-mode/normalization rules,
remain relevant; clean does not imply byte identity, full filesystem coverage,
software correctness or completeness of the working candidate.

## Exit semantics

| Code | Meaning |
| --- | --- |
| 0 | Complete scoped observation, including dirty or unborn state |
| 1 | CLI output failure |
| 2 | Invalid arguments, invalid/inaccessible input path, or non-directory input |
| 3 | Git executable unavailable |
| 4 | Git/object/discovery/access/capture I/O failure, including unreadable traversal |
| 5 | Unsupported repository/host form or missing resource limiter |
| 6 | Observed time, output, operation or entry limit exhaustion |
| 7 | Changed/inconsistent or malformed/indeterminate observation |

Diagnostics identify the failing operation/category without echoing argument
values, repository file contents, configuration values or raw Git errors. Output
I/O causes remain available internally. Exit zero is not a passing assessment.

## Acceptance mechanisms and observed coverage

[Real CLI fixtures](../../crates/cortex-cli/tests/inspection.rs) cover the state
matrix, special paths, linked worktrees, SHA-256, object failures, denied traversal,
unsupported forms, environment/helper traps, actual entry/output/time exhaustion,
private diagnostics and repeated inspection. [Unit mechanisms](../../crates/cortex-cli/src/inspection/mod.rs)
cover controlled concurrent change and operation/deadline rejection;
[parser tests](../../crates/cortex-cli/src/inspection/parse.rs) cover malformed
records and exact entry boundaries; [capture tests](../../crates/cortex-cli/src/inspection/process.rs)
cover byte boundaries, pipe retention, deadlines and the actual child resource limit.
Existing help/version and core/evidence contract regressions remain required.

Read-only tests recursively compare all fixture paths, regular-file bytes,
symlink targets, modification times and permissions, including `.git` contents
(index, configuration, refs, objects and metadata). Linux comparisons also retain
inode/device, mode, owner/group and change timestamps. Both a mixed dirty worktree
and a clean linked worktree needing a stat-cache refresh are inspected repeatedly;
the linked test covers common and per-worktree Git metadata. Access times are
excluded because reads may change them; ACLs, extended attributes and forensic
write-attempt tracing are not established by these comparisons.

Executed acceptance and tool versions are recorded against the final candidate
in the external milestone handoff. Test source is a mechanism, not an execution
record. The catalog uses `IMPLEMENTED`; candidate-specific local results do not
promise a future candidate's result or a passing hosted pipeline. Remote CI remains
`NOT_RUN`. See [README verification](../../README.md#verify) for required commands.
