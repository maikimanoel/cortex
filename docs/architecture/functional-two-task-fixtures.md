# Trusted two-task functional integration

GPE-FUNC-2T connects the existing CAP-CORE-003, CAP-WFL-001/004,
CAP-EVD-003, CAP-HRN-001/002, CAP-MEM-001, CAP-SEC-002, CAP-CTX-002 and
CAP-CLI-004 boundaries. This owner-authorized development increment changes no
complete capability status or milestone. Actual acceptance results belong to the
reviewed candidate's private handoff, not this source contract.

## Observable behavior and ownership

One accepted `ProgramDefinition` contains A, B and one exact versioned artifact
edge. A's finite trusted Rust fixture writes real deterministic prime-list bytes.
The parent captures those bytes and independently compares them with a fixed
literal oracle. A successful process alone creates neither assessment nor release.
Unified Evidence records and A-1 exact correspondence support a separate scoped
coordinator decision. A remains Active when its increment releases B's Preview
edge at ContractStable, represented as RELEASED_NON_EFFECTFUL.

After the release commits, the parent drops the controller and reopens the same
encrypted format-3 ProgramStore. The direct child has already been reaped and
its streams closed. The parent retains the synthetic key; no child receives it.
Reconstruction preserves the baseline, producer/artifact association, release and
operation identities. Repeated reads and exact duplicate admissions cause no
launch, additional release or journal mutation. This is controlled development
reconstruction, with no power-loss, host-restart or live-interruption claim.

B receives a separate claim, actual captured source bytes, package, operational
grant, resource reservation, process preparation and durable dispatch. Its fixture
reads A's exact file, writes a consumed-byte receipt and computes count, sum and
squared sum. A separate fixed output oracle checks the result. Explicit settlement
uses actual terminal observations and retains cumulative elapsed/output accounting.

A material expected-verification-profile change invalidates the released support
through the existing Evidence examination and causal consumer closure. B's old
generation and release cannot be reused; original PASS and release history remain.
The upstream producer is preserved. The existing independent-branch regression
additionally checks that unrelated D/E work survives a scoped A/B/C/F change.

A new synthetic no-child dispatch in the same main journal retains an Unknown,
its reservation and metadata claim. Retry, invented settlement and absence/refund
without observations are refused. This deliberately unresolved synthetic record
does not describe a live or historical operational process.

Core still owns transitions, Evidence composes the single Workflow journal,
Storage encrypts and reconstructs it, Harness owns process/file capture, and CLI
only presents retained state. No new crate, dependency, format, policy authority,
workflow database or effect ledger is introduced.

## Artifact capture boundary

`cortex_harness::artifact::capture_artifact` retrieves the retained successful,
quiescent process observation and checks current program/baseline/generation,
candidate, attempt and exact producer increment/artifact correspondence. Retired
attempts, unresolved effects and mismatched artifact revisions are refused.

The caller explicitly associates an output path with that effect. Capture cannot
authenticate who wrote a file. It returns bounded bytes, their SHA-256 and exact
artifact/effect/observation references; the caller supplies independent semantic
assessment and separate coordination authority. A hash is integrity correspondence,
not authentication or acceptance.

Output must lie in an explicit private canonical directory outside every retained
source root, Git directory and Git common directory, plus other caller-supplied
protected sources such as Cortex itself. Protected paths are resolved before
overlap checks. Output opens with NOFOLLOW/NONBLOCK and requires a private owned
single-link regular file. Descriptor/path stamps and bounded before/after reads
reject observed change. A FIFO cannot block the file-open boundary. These checks
assume a cooperating local filesystem; they are not hostile same-user isolation,
an atomic snapshot or permission to execute arbitrary payloads.

## Resource and inspection contracts

The trusted direct-child profile keeps its existing WorkerSlots, ElapsedMillis
and OutputBytes accounting. Logical admission and configured limits establish no
physical RAM, CPU, disk or descendant containment. The fixture executable is
stripped to fit its preselected executable bound; ProgramLimits is not enlarged
to accommodate an incompatible package. Fixture source, Cortex source, output,
state, controller, build, evidence and quarantine use separate paths; the campaign
root itself is not declared a source root.

Typed AccountingPool memory and legacy refusal are supplementary synthetic
compatibility cases in `program_memory.rs`. They do not account for or enforce
memory of the two real effects and do not create another authority for them.
Unknown historical bytes remain unspecified. No historical reservation or key is
adopted, and no historical store is opened.

Schema 3 adds `evidence` with observation/assessment identities, contract/candidate,
historical result, applicability and contextual validity against retained assessment
context. `validity_basis` explicitly excludes live verification. Release decisions
also expose exact `witnesses`, including artifact revision/candidate and Evidence
references. Human schema 3 includes the retained Evidence summary. Inspection
uses draft records and never persists an examination or recaptures Git. Historical
schemas 1/2 retain their representations, including explicit refusal when typed
memory cannot be represented. Default and explicit schema 3 are equivalent.

## Acceptance mechanisms

| Scenario | Discriminating oracle and observable result |
| --- | --- |
| Q0 | Valid source/effect siblings and accepted package launch; source-state overlap, wrong cwd/profile and executable/program-limit mismatches refuse before dispatch |
| Q1 | Actual file equals fixed prime bytes and independent SHA-256; exit alone leaves Evidence and release absent; wrong version, oversized capture, symlink, hardlink, FIFO and source alias refuse |
| Q2 | Current assessed exact support and scoped authority release one edge; absent/unassessed/forged/contradictory support stays held; plausible wrong bytes receive FAIL and cannot PASS |
| Q3 | Same baseline, artifact and one release after no-child reconstruction; duplicate release/dispatch retain original receipts with no callback; store bytes/metadata and revisions remain unchanged |
| Q4 | Separate B admission consumes actual A bytes once; fixed count/sum/squares result and consumed-byte receipt match; cumulative usage survives settlement |
| Q5 | Changed expected oracle profile makes support stale and increments B's generation; late reports/releases refuse; A and supplementary unrelated D/E branch retain their scoped state |
| Q6 | Synthetic Unknown retains demand and claim after replay; automatic retry, invented settlement and unjustified absence/refund refuse |
| Q7 | Actual CLI default/explicit schema 3, retained Evidence/witnesses, uncertainty and holds; representable schemas 1/2; supplementary typed-memory refusal; no journal byte/metadata change |

The connected mechanism is
[`gpe_functional.rs`](../../crates/cortex-cli/tests/gpe_functional.rs), using the
audited finite [`gpe.rs`](../../crates/cortex-cli/tests/fixtures/gpe.rs) fixture.
Supplementary mechanisms are
[`program_memory.rs`](../../crates/cortex-cli/tests/program_memory.rs) and
[`Evidence program tests`](../../crates/cortex-evidence/tests/program.rs).
Existing supervisor/storage tests retain malformed-input, concurrent-dispatch,
uncertain-commit, cancellation and recovery coverage. Test source is not a run log.

Local runs use one Cargo command, two jobs and two test threads; CI retains one
standard included-capacity Linux job and the same job/thread caps. The connected
test admits at most two sequential child launches per invocation, reserves finite
invocation/dispatch slots before use, retains failures when an explicit private
campaign root is selected, and never retries Unknown. External campaign control
owns aggregate time/disk/RAM limits. None of these cooperative development controls
qualifies a host or establishes operational multi-executor readiness.

Provider execution, arbitrary/untrusted workers, physical host qualification,
historical resource disposition, protected integration and broader GPE completion
remain separately gated.

## Fresh development identity and composed consumers

The optional `CORTEX_GPE_FIXTURE_NAMESPACE` accepts a bounded explicit ASCII
namespace before fixture effects. Test-only identity support qualifies program,
task, attempt, operation, package and release correspondence consistently. An
exclusive private marker refuses reuse; the namespaced scenario permits at most
two designated fixture children. The unparameterized fixture retains its original
identity behavior. This is not a production identity registry or retry policy.

The connected development path composes explicit advisory method selection,
non-effectful scheduling preparation and the complete declared evaluation
population. It retains launch manifests and exact executable bytes privately
before dispatch. Controller build/inspection helpers are distinct from the two
audited fixture executors, which invoke no tools or descendants. All actual
checks require candidate-scoped execution evidence; this source is not a test-run
record, live evaluation, host qualification or protected integration.
