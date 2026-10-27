# Deterministic task-to-evidence check (002-D)

This is the bounded CLI implementation contract for
[CAP-CLI-003](../product/capability-catalog.md#cap-cli-003), completing the initial
CLI portion of [CAP-EVD-002](../product/capability-catalog.md#cap-evd-002).
It composes [002-A inspection](repository-inspection.md),
[002-B binding](candidate-binding.md) and [002-C evidence](unified-evidence.md).
The catalog owns capability requirements/status; the roadmap owns sequence.
No general task runner, agent, permission engine or persistence is provided.

## DEC-007: minimum input and criterion

The examples use the executable name. From an uninstalled source checkout, prefix
each invocation with `cargo run --offline --locked -p cortex-cli --` in place of
`cortex`; see [setup and runnable examples](../../README.md#run-locally).

```sh
cortex check --task release.head --revision 1 \
  --candidate "$SUBJECT_COMMIT" --expect-head "$REQUIRED_COMMIT" \
  --repo "/path/to/repository"
cortex check --json --task release.head --revision 1 \
  --candidate "$SUBJECT_COMMIT" --expect-head "$REQUIRED_COMMIT"
cortex check --help
```

Set the two commit variables to full lowercase Git object IDs. `--repo` defaults
to the current directory, including a repository subdirectory. The single fixed
criterion is **`git.head-equals.v1`**: compare locally resolved committed HEAD with
`--expect-head`. Equality supports its oracle; inequality refutes it. Absence or
uncertainty cannot establish either conclusion for the required committed candidate.
Only this proposition is checked; no arbitrary software acceptance text is executed.

| Input | Meaning and validation |
| --- | --- |
| `--task` | Required existing `TaskId` syntax: 1..128 ASCII bytes, alphanumeric first, then alphanumeric/dot/underscore/hyphen |
| `--revision` | Required positive `u64`, at most 20 decimal digits; no sign; leading zeros normalize numerically |
| `--candidate` | Required subject pin: full lowercase 40- or 64-hex commit ID syntax; actual existence/correspondence is established by observation/binding, not syntax |
| `--expect-head` | Required criterion value with the same object-ID syntax; independently compared with actual HEAD |
| `--repo` | Optional nonempty path; spaces, Unicode and Unix byte paths supported within 002-A's scope |
| `--json` | Optional, immediately after `check`; selects version 1 JSON, including invalid-input errors |

Candidate pin and expected criterion value have distinct roles. A stable clean
candidate at commit A, pinned to A, can correctly FAIL a requirement for B. Pinning
to B while observing A instead yields UNKNOWN with incompatible/stale candidate
context. Neither input is substituted for actual HEAD. Selected repository identity
comes from canonical worktree/Git/common locations, with no asserted remote project
identity. Changing the task, repository, pin or criterion requires a new contract
revision under 002-B; there is no cross-invocation history/alias registry.

Options other than `--json` may appear in any order, exactly once, as flag/value
pairs. Missing, duplicate, unknown, malformed or excessive input is rejected before
Git runs. There are at most 12 tokens after the optional JSON flag, each at most
16,384 encoded bytes. Task/revision/commit inputs must be UTF-8; paths may contain
non-UTF-8 bytes. No stdin, task file, free-form command or configuration language
is accepted. Objective and the one acceptance criterion are constructed explicitly
from the fixed check and expected commit using existing validated task types.

## Processing and evidence

The CLI captures the repository, builds the exact binding contract and source
observation, then performs a separate bounded reinspection. Both use the same
producer scope with increasing capture sequences. Core's existing `bind_candidate`
returns the typed binding decision. An explicit oracle compares captured resolved
HEAD to the expected value. Evidence alone enforces whether that oracle can support
a conclusive assessment against the fresh candidate/profile. A rejected conclusive
request is qualified as UNKNOWN only for `CurrentContextRequired`; other semantic
errors remain errors. No terminal judgment is overwritten.

The execution observation describes the actual completed porcelain-status queries
used by 002-A, whose successful capture requires exit zero. It is not the CORTEX
process exit. One relevant check means one explicit committed-HEAD comparison,
not a test suite. The oracle and candidate validation are separate from that exit.

| Observation context | Assessment |
| --- | --- |
| Clean stable committed subject matches its pin; oracle equal | PASS / APPLICABLE / CURRENT |
| Same supported subject; oracle unequal | FAIL / APPLICABLE / CURRENT |
| Wrong pin, changed commit/state/revision/profile | UNKNOWN with the typed mismatch/stale cause |
| Matching dirty state under `AllowChanges` | UNKNOWN / UNESTABLISHED; retain staged/unstaged/untracked distinctions |
| Detached HEAD | Commit retained; can PASS or FAIL when the binding is supported |
| Unborn HEAD | No observed commit; UNKNOWN / STALE relative to the required committed subject |
| Incomplete, incomparable or indeterminate observations | UNKNOWN; no current acceptance |
| Reinspection prerequisite unavailable after source capture | BLOCKED / UNESTABLISHED, no completed comparison or oracle; original operational error retained |
| Inconsistent/malformed reinspection | UNKNOWN / UNESTABLISHED with inference kind; no successful verification execution claimed |
| Failure before any source capture | Operational error, assessment absent; no fabricated candidate, provenance or time |

The fixed criterion is applicable; this command does not offer a skipped or N/A
mode. Existing 002-C NOT_RUN, N/A and uncertain-applicability contracts remain
available and tested independently. Missing information never becomes N/A or PASS.

Provenance retains criterion/task/revision, subject and capture IDs, mechanism,
version, inputs, fixed configuration description, OS/architecture, actual Git
version, inspection profile, observation/recording/assessment times and limitations.
Verification observation time is sampled after comparison or failure determination;
it is distinct from the source capture. Candidate captures retain ordered IDs,
without invented individual timestamps. IDs combine process/time scope and local
sequence; they are not global uniqueness, cryptographic integrity or authentication.
The evidence layer still receives caller reports, even when the CLI really captured
them. No output grants operational permission.

## Human output

Human output identifies the criterion/expected HEAD, task/revision, required subject,
canonical root, actual HEAD/branch, binding, mechanism/components and reported times.
It displays independent result/applicability/validity, reasoning, dirty categories,
coverage and explicit limitations. For a supported equal committed revision:

```text
Criterion: git.head-equals.v1
Expected HEAD: <required full commit>
Task: release.head
Contract revision: 1
Required candidate: <subject full commit>
...
Result: PASS
Applicability: APPLICABLE
Validity: CURRENT
...
Authority: NONE (evidence grants no permission)
```

Paths and branch bytes are quoted/escaped for terminal display; undecodable branch
bytes use hexadecimal. Diagnostics use stderr. A normal FAIL/UNKNOWN assessment is
reported on stdout; an operational failure also emits an actionable stderr diagnostic.
Help/version/`inspect` keep their prior behavior. Human presentation is not a parser API.

## JSON output version 1

`check --json` writes exactly one newline-terminated JSON object on stdout, including
recognized machine-mode errors. Use exit code and parsed fields together. Object
key order is not a compatibility promise. Times and invocation IDs vary across runs;
the criterion/binding/assessment rules are deterministic for their typed inputs.

| Field | Meaning |
| --- | --- |
| `schema_version`, `type`, `exit_code`, `authority` | Version `1`; `assessment` or `error`; documented integer exit; authority always `NONE` |
| `task` | ID, revision as an exact **decimal string**, generated objective and acceptance |
| `criterion`, `required_candidate` | Stable criterion ID/expected HEAD; separate caller-supplied subject pin |
| `source`, `reobserved` | Actual captured subject and fresh capture, or null when unavailable |
| `binding` | Core `status` and nullable typed `reason`; never an assessment |
| `observation` | Report ID, explicit caller-report source/authentication limit, producer, times, mechanism context, outcome/oracle and detail |
| `assessment` | Judgment ID, result, applicability, contextual validity/cause, `current_pass`, reasoning and assessment time; null when absent |
| `error`, `limits` | Nullable operational error category/message; explicit coverage/trust limitations |

Each candidate contains `observation_id`, `repository_root`, `head`, `working_tree`,
`coverage` and `consistency`. `head.kind` is ATTACHED/DETACHED/UNBORN/UNKNOWN; `commit`
is nullable and `branch` is nullable tagged bytes. Repository root and branch use
`{"encoding":"utf-8","value":"..."}` or lossless lowercase hex with `encoding:"hex"`.
Working-tree state is CLEAN/DIRTY/INDETERMINATE with separate integer counts.
Coverage is GIT_STATUS/INCOMPLETE; consistency is TWO_MATCHING_SAMPLES/INDETERMINATE.
No changed filenames or raw comparison material are exported.

Binding status is COMPATIBLE/INCOMPATIBLE/STALE/INSUFFICIENT. Reasons are the uppercase
002-B variants: TASK_IDENTITY, REPOSITORY, COMMIT, COMMIT_ABSENT, WORKING_TREE;
CONTRACT_REVISION, OBSERVATION_ORDER, OBSERVED_STATE_CHANGED; MISSING_OBSERVATION,
REOBSERVATION_REQUIRED, DIFFERENT_OBSERVATION_SCOPE, INCOMPLETE_COVERAGE,
INDETERMINATE_CONSISTENCY, MISSING_REPOSITORY, MISSING_COMMIT, UNCOMMITTED_IDENTITY,
MISSING_COMPARISON_BASIS. A compatible result has a null reason.

Assessment result uses 002-C's PASS/FAIL/UNKNOWN/NOT_RUN/BLOCKED/N/A vocabulary;
applicability is APPLICABLE/NOT_APPLICABLE/UNCERTAIN. Validity is CURRENT/STALE or
UNESTABLISHED when the typed validity is absent. `validity_issue` is null or
`{kind,cause}`: CANDIDATE_MISMATCH, CANDIDATE_STALE or CANDIDATE_INSUFFICIENT
with the binding reason, INVALID_CONTEXT, PROFILE_CHANGED, MISSING_PROFILE,
OBSERVATION_IDENTITY_COLLISION or EXPLICIT_INVALIDATION. Causes are null when absent.
`current_pass` requires PASS, APPLICABLE and CURRENT together; historical result
alone never establishes it.

IDs for captures/reports have `scope` and integer `sequence`. Times have integer
`unix_seconds` and `nanoseconds` since the Unix epoch. Mechanism fields are `name`,
`version`, `inputs`, `configuration`, `environment` and `{name,version}` components.
Execution outcomes retain status/nullable exit code, relevant check count and an
optional oracle with SATISFIED/VIOLATED/INSUFFICIENT plus reasoning. Inference has
no execution or oracle. No authenticated execution is implied by any field.

Errors contain a category and non-private message. Stable categories are
INVALID_INPUT, INVALID_PATH, GIT_UNAVAILABLE, INSPECTION_FAILURE, UNSUPPORTED,
RESOURCE_LIMIT, INCONSISTENT_OBSERVATION and INVALID_OBSERVATION. An output failure
may prevent JSON delivery and always exits nonzero. The v1 projection is a small
CLI contract, not a persisted evidence schema or Portable Context Package format.
Future incompatible changes require a new output version.

## Exit behavior and bounds

| Code | Meaning for `check` |
| --- | --- |
| 0 | CURRENT PASS only; `check --help` also succeeds without assessment |
| 8 | CURRENT FAIL for this criterion |
| 9 | Unresolved/non-current assessment without an operational inspection error |
| 1 | Output/diagnostic I/O failure; partial I/O cannot be retracted |
| 2 | Invalid task/options/path |
| 3 | Git unavailable |
| 4 | Git/discovery/object/access/capture I/O failure |
| 5 | Unsupported repository/host/resource-limiter prerequisite |
| 6 | Inspection resource exhaustion or report exceeds 65,536 bytes |
| 7 | Inconsistent/malformed/invalid observation, evidence or clock context |

A failed refresh retains its inspection exit even when a BLOCKED/UNKNOWN assessment
is available. Exit zero of help or `inspect` still means successful display/observation,
not verification. The two modes' exit conventions are explicitly distinct.

At most two 002-A inspections run, each with its existing 30-second observation
budget, 32-query maximum, five-second process deadline, capture/entry limits and
Linux resource controls. Thus the configured aggregate observation budget is at
most 60 seconds/64 queries, with at most two seconds additional child cleanup.
No automatic retries or new Git queries are added. Reports are built in a bounded
65,536-byte buffer before stdout; overflow produces an error instead of truncated
success. These are not hard real-time, total-RSS or terminal-backpressure guarantees.

## Privacy, compatibility and verification limits

The selected root, task identity, required/observed commits and necessary criterion
and mechanism metadata are intentionally disclosed. Raw repository configuration,
index/status bytes, changed filenames, environment values, authentication material
and tool transcripts are omitted. Output is a redacted projection, not a complete
replay package or credential-detection mechanism. The original typed evidence
retains its bounded comparison basis internally until process exit.

All 002-A supported/unsupported repository forms and read-only controls apply.
Ordinary reads may update access times. Equal captures are not atomic; ignored
files, edit/revert races, same-status dirty edits and changes after capture retain
their original limitations. Dirty HEAD is never the complete candidate. No persistent
freshness, authenticated provenance, general source correctness or sandbox is claimed.

The private binary modules separate input, composition and presentation. Core and
Evidence production APIs remain unchanged. `RepositoryObservation` exposes its
captured version/identity and consumes itself through the same typed adapter used
by `inspect_candidate`. CLI -> Evidence -> Core is acyclic. The six existing real-Git
evidence tests move into the CLI consumer; their mechanisms remain intact.

The CLI uses a locked JSON serializer because the standard library has none;
manual JSON escaping/parsing would add avoidable maintenance and encoding risk.
No runtime network dependency is introduced. Manual CI fetches locked dependencies
before offline checks, without adding triggers or authorizing hosted execution.

[Executable fixtures](../../crates/cortex-cli/tests/check.rs) cover passing and
refuting propositions, invalid input and errors, candidate mismatch, mixed dirty
state, detached/unborn, Unicode/byte paths, subdirectories, linked worktrees,
SHA-256, privacy and repeated content/metadata invariance. Controlled
[composition tests](../../crates/cortex-cli/src/check/tests.rs) exercise actual
between-capture changes, failed/uncertain refresh, profile/revision/source/coverage
limits, output overflow and temporal ordering. Existing inspection concurrency and
resource tests remain required. Invariance comparisons exclude atime and do not
establish ACL/xattr equality or forensic write-attempt tracing. Executed counts,
source tree, tools and private delivery observations belong in the external handoff;
test source and CI configuration are not execution evidence.
