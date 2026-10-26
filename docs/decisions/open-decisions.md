# Open decisions

These questions are unresolved. The proposed decision points below do not select
defaults, grant authority, or authorize implementation. Capability requirements
remain in [the catalog](../product/capability-catalog.md); execution order is in
[the roadmap](../product/roadmap.md). Preserve the accepted
[foundation decision](../architecture/0001-foundation.md). Record a new ADR when
an actual consequential choice is authorized and made.

## DEC-001

**Open-source license.** No license or SPDX identifier has been selected. Review
distribution intentions and actual dependency obligations with the owner before
007/publication. Until then, keep packages unpublished and the repository private.
Dependent capability: CAP-CLI-006.

## DEC-002

**Public brand and trademark review.** Determine appropriate name/brand checks
and any resulting changes before public launch. Current product identity does
not imply legal clearance. Decision point: 007. Dependency: CAP-CLI-006.

## DEC-003

**Contribution and intellectual-property governance.** Decide contribution
terms, ownership/permission requirements, and review responsibilities before
accepting outside contributions. Neither a private repository nor agent-generated
work settles those questions. Dependencies: CAP-SKL-002 and CAP-CLI-006; decide
before outside contributions, even if that occurs before 007.

## DEC-004

**Privacy, retention, and deletion defaults.** Determine classification, consent,
retention for canonical/transient/ephemeral layers, physical deletion, backups,
export handling, and permitted non-sensitive audit remnants before persistent
memory is enabled in 003-A. Logical correction/supersession of retained history is
distinct from policy-governed physical deletion; neither selects unconditional
retention or a deletion guarantee before implementation. Before 004-D, also decide
sensitive trace capture, access, omission/redaction and retention limits. Useful
execution evidence does not authorize unrestricted storage of raw tool output.
Dependencies: CAP-MEM-001/002/003 and CAP-SEC-002.

## DEC-005

**Default execution, isolation, and CI budgets.** Choose explicit time, CPU,
memory, operation, output/data-volume, path/network, and cleanup bounds appropriate
to each actual operation and host. Set the bounded inspection limits before
002-A and agent/sandbox defaults before 004-D. Hosted CI usage and authorization
must be established before any dispatch. The foundation's existing ten-minute
manual job timeout is a configured limit, not a universal product budget.
Dependencies: CAP-CLI-002/005, CAP-CORE-003, CAP-HRN-001, and CAP-EVL-001.

**002-A scoped resolution:** The [inspection contract](../architecture/repository-inspection.md#finite-budgets)
sets time, capture, operation, entry and Linux address-space limits, prerequisite
failures and cleanup limits. This does not select agent/sandbox or hosted-CI policy.

## DEC-006

**Initial real agent adapters and supported host/runtime matrix.** Choose actual
agents, operating systems, runtime capabilities, and isolation backends using
requirements, threat analysis, and observations. No adapter is presumed
interchangeable; unsupported host controls must have explicit failure behavior.
Decide before 004-D/004-E; validate any expanded matrix in 006. Component-style
runtimes and MCP remain optional integration choices. Dependencies: CAP-HRN-001/002,
CAP-CTX-002, and CAP-SEC-001.

## DEC-007

**Storage, graph backends, formats, and API compatibility.** Choose persisted
formats, identities, temporal interval representation, transactions/rebuilds,
export evolution, graph storage, query ergonomics, and public compatibility
commitments only for actual consumers. Define the minimum inspection/evidence
output contract before 002-D and persistent/graph contracts before 003-A/003-B;
defer broader API guarantees until justified. Existing version 0.1.0 does not
promise a stable API. Dependencies: CAP-EVD-002/003, CAP-KNW-001/002/003,
CAP-MEM-001/003, and CAP-WFL-001.

Before 003-E/003-F, decide the minimum Portable Context Package integrity,
project/candidate identity, freshness and privacy/omission semantics for manual
exchange. Before 004-C/004-D, define parallel-workspace coordination and guarded
integration/effect records without treating worktrees as sandboxes. Dependencies:
CAP-CTX-002, CAP-WFL-003/004. Before 004-C, define the minimum durable effect record
and operation-specific retry/reconciliation semantics for CAP-WFL-001, linked to
actual harness dispatch and applicable Core authority. Prefer a bounded local-first
contract before considering distributed coordination. No format, transaction/backend
or workspace implementation is selected; lifecycle requirements are not a schema.

**002-C scoped resolution:** The [evidence contract](../architecture/unified-evidence.md#identity-provenance-and-time)
retains exact existing task/revision/candidate/capture identities, a separate
verification observation and assessment identity, versioned verification context
and caller-reported observation/recording/assessment times. In-memory versions are
exact references, independent of future format versions. The deliberately redacted
library summary is not a versioned interchange or CLI schema. The separate 002-D
resolution below settles only the minimum CLI interface; persistence, Portable
Context Packages, memory, graph and harness/transport formats remain deferred.

**002-D scoped resolution:** The [CLI check contract](../architecture/cli-check.md#dec-007-minimum-input-and-criterion)
selects flag/value task input with explicit task ID/revision and separate subject
pin/expected HEAD for criterion `git.head-equals.v1`. It uses human output by default
and `check --json` for a bounded version 1 projection, typed error/uncertainty
categories, explicit operational versus assessment exits and lossless byte encoding.
Task revision is a decimal string in JSON. The serializer avoids hand-written JSON
escaping; no generalized task/configuration language or storage schema is introduced.
The linked contract specifies fields, limits, privacy and compatibility. Existing
library APIs and 002-C semantics remain unchanged. Broader formats, persisted
identity/history, portable interchange and API commitments remain open at their
original milestone boundaries.

## DEC-008

**Evaluation cases, baselines, and comparison-publication policy.** Select
benchmark families, reserved/held-out cases, access controls, oracles, baselines,
resource accounting, and rules for reporting comparisons before 005-A or any
public quantitative claim. Specify case/attempt populations and denominators that
retain failures, execution errors, skips and incomplete attempts under CAP-EVL-001.
Decide protection/access boundaries for independent judgment material and accounting
outside the evaluated agent/harness's authority, and controlled comparison design
under CAP-EVL-002. These requirements do not select datasets, numerical targets,
reviewer governance or publication permission. Separate model and harness effects
where the study can identify them; otherwise report the confounding. Dependencies: CAP-EVL-001/002
and CAP-EVO-002. No benchmark score or superiority claim is approved here.

## DEC-009

**Public release scope and approval.** Decide the intended audience, distribution,
release scope, privacy/history review, and explicit owner approval before any
public launch. Technical completion of 007 is preparation, not publication
authority. Dependencies: DEC-001/002/003, CAP-CLI-006, and the 006 integration exits.

## DEC-010

**Evidence semantic extension — resolved for 002-C.** The
[evidence contract](../architecture/unified-evidence.md#dec-010-explicit-extension-no-implicit-migration)
selects a separate `cortex_evidence::unified` representation. Foundation evidence
and terminal `Inconclusive` retain their original meanings, with no inferred or
implicit conversion of missing distinctions, identities or times. Extending the
legacy enum or inferring richer metadata was rejected because it would alter
existing semantics or invent facts.

Result, applicability and contextual validity are independent. Conclusive judgments
require an explicit applicable/examined scope and current candidate/profile
correspondence; PASS/FAIL additionally require relevant checks and a matching oracle.
Known invalidation is sticky while historical judgments remain unchanged. Missing
validity cannot imply CURRENT. Reports remain unauthenticated caller assertions;
no assessment grants permission. The linked contract owns construction rules,
limits, redaction and compatibility. No storage schema, transitive dependency graph,
runner or persistence migration is selected. Dependencies: CAP-EVD-002 and CAP-CORE-002;
DEC-007's broader persistent/interchange/API decisions remain open; the minimum
002-D CLI projection is resolved only within its documented scope.

## DEC-011

**Candidate inspection and extraction coverage.** Before 002-A, decide revision
selection, supported repository/worktree forms, path encoding, untracked/ignored
coverage, concurrent-change detection, and any future dirty-snapshot mechanism.
An unsupported case must be explicit, never a false clean candidate. Before
003-B, declare the first language/document/schema/binary adapter coverage and
dependency invalidation rules. Dependencies: CAP-CLI-002, CAP-CORE-002, and
CAP-KNW-001. No arbitrary language count or complete snapshot guarantee is set.

**002-A scoped resolution:** The [inspection contract](../architecture/repository-inspection.md)
selects HEAD, ordinary/linked working trees, Unix byte paths, explicit ignored-file
exclusion and unsupported special states, with two matching observations and stated
race limitations. Dirty snapshot identity and later extraction coverage remain open.

**002-B scoped resolution:** The [binding contract](../architecture/candidate-binding.md)
selects explicit caller-assigned task revisions, scoped observation ordering,
exact contract applicability, canonical local worktree identity, bounded comparison
material and Core correspondence rules. Matching dirty status remains insufficient
for content identity; no dirty snapshot mechanism is selected. DEC-010's separate
002-C resolution consumes these limits without strengthening them.

## DEC-012

**Proportional profiles and required reviewers.** Before 004-C, establish
auditable classification rules for trivial/standard/critical work and the qualified
review/approval obligations applicable to each contract. Keep contract-mandated
reviews even when automation is convenient; do not impose multi-agent or formal
methods on every task. Dependencies: CAP-WFL-002, CAP-CORE-003, and CAP-EVL-002.
