# Open decisions

This register tracks unresolved questions and scoped resolutions. Proposed decision
points do not select defaults, grant authority, or authorize implementation.
Capability requirements remain in [the catalog](../product/capability-catalog.md); execution order is in
[the roadmap](../product/roadmap.md). Preserve the accepted
[foundation decision](../architecture/0001-foundation.md). Record a new ADR when
an actual consequential choice is authorized and made.

## DEC-001

**Open-source license — resolved by the owner.** CORTEX uses the standard
[MIT License](../../LICENSE), SPDX identifier `MIT`, with copyright (c) 2026
Maiki Manoel. All workspace packages inherit this sole project license and retain
`publish = false`. Third-party license obligations remain independently applicable.
License adoption does not authorize public access, releases or package publication;
keep the repository private. Distribution readiness and approval remain DEC-009.
Dependent capability: CAP-CLI-006.

## DEC-002

**Public brand and trademark review.** Determine appropriate name/brand checks
and any resulting changes before public launch. Current product identity does
not imply legal clearance. Decision point: 007. Dependency: CAP-CLI-006.

## DEC-003

**Contribution and intellectual-property governance — initial source policy
resolved for 002-P2.** Maiki Manoel maintains CORTEX. External source contributions
use GitHub pull requests, maintainer review and automated validation, preserving
architectural contracts. No CLA or DCO sign-off is required. Contributors must have
the right to submit their material under MIT and retain applicable third-party
notices. [CONTRIBUTING.md](../../CONTRIBUTING.md) owns the practical process.
This does not grant public visibility or settle future skill-registry submission,
moderation, broader governance or additional third-party rights questions.
Dependencies: CAP-SKL-002 and CAP-CLI-006; settle those remaining questions before
their actual external consumers.

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

**003-A preparation boundary:** The [temporal-record contract](../architecture/temporal-records.md)
bounds an in-memory domain slice with synthetic/non-sensitive inputs and no automatic
capture, persistence or export. It selects no retention duration, consent policy,
sensitive-data default or physical-erasure guarantee. These still gate persistent
memory; logical correction is not authorized deletion/omission behavior.

## DEC-005

**Default execution, isolation, and CI budgets.** Choose explicit time, CPU,
memory, operation, output/data-volume, path/network, and cleanup bounds appropriate
to each actual operation and host. Set the bounded inspection limits before
002-A and agent/sandbox defaults before 004-D. Hosted CI usage and authorization
must be established before hosted execution. The configured ten-minute CI job
timeout is a scoped limit, not a universal product budget.
Dependencies: CAP-CLI-002/005, CAP-CORE-003, CAP-HRN-001, and CAP-EVL-001.

**002-A scoped resolution:** The [inspection contract](../architecture/repository-inspection.md#finite-budgets)
sets time, capture, operation, entry and Linux address-space limits, prerequisite
failures and cleanup limits. This does not select agent/sandbox or hosted-CI policy.

**002-P2 scoped resolution:** The owner authorizes PR/main/manual validation in
one standard GitHub-hosted Linux job within the included free allowance. No paid
runners, payment methods, upgrades or charges are authorized. Establish the
no-charge execution boundary before running CI; quota exhaustion must be reported
as a block, not bypassed. The workflow uses no uploaded artifacts or caches. Actual remote
execution is candidate-bound handoff evidence, never inferred from configuration.
Future execution/sandbox budgets remain open.

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

**003-A preparation boundary:** The temporal-record contract selects only minimum
deterministic in-memory identity, temporal and correction rules in existing Core.
Public signatures and persisted schemas, transactions, crash recovery, migration
and storage remain undecided until an actual durable consumer is authorized.
Evaluate journal, transactional and snapshot-plus-log alternatives against those
contracts, not architectural symmetry. No new crate or public format is selected.

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

**Public release scope and approval — pre-alpha source scope resolved for
002-P2; launch approval remains open.** The owner approves preparation for an early
MIT source-only pre-alpha launch of the delivered 001/002 capabilities, before
003–006. This is a scoped exception to the full-platform 007 sequence, not 006/007
completion or permission to change visibility. No binary release, version tag,
package publication, production-readiness or general API-stability claim is needed
or authorized. [002-P1/P2/P3](../product/roadmap.md#pre-alpha-source-preparation-and-launch-checkpoints)
separates audit, remediation and final readiness preparation. Public visibility
still requires a subsequent explicit owner decision.

Before any public access, retain history/privacy disposition, DEC-002 brand
review, provider-managed metadata review and explicit owner approval as gates.
[SECURITY.md](../../SECURITY.md) records the approved private-reporting route and
its required launch-time activation/verification. No alternate contact or support
SLA is selected. Technical preparation never grants publication authority. The
full-platform 007 contract still depends on 006 and applicable DEC-001/002/003 exits.

**002-P3 readiness boundary:** Successful local checks do not satisfy an unexecuted
remote-CI gate. Review the normal path with actual restored remote execution and
any proposed temporary pre-alpha source-only exception separately. An exception
requires explicit owner acceptance of its candidate, scope, residual risk,
compensating verification and expiration conditions; none is approved here.
Keep unavailable CI and unperformed remote checks explicit. Preparation or an
exception decision alone never authorizes public visibility.

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

**003-A preparation boundary:** The in-memory record slice preserves supplied
observation states; it creates no new snapshot identity. Before snapshot/extraction
work, choose and verify clean-commit-only versus an explicit immutable working
snapshot scope, coverage/exclusions, content correspondence and race behavior.
Dirty status equality cannot select that mechanism. This remains an open gate,
not a completed CAP-CORE-002 extension.

## DEC-012

Before 004-A/004-B, decide the initial acceptance-baseline amendment authority and
rule-qualification/trusted-blocking policy under CAP-CORE-003/CAP-SKL-002. Protected
baselines must allow authorized evolution; an unqualified engineering rule cannot
become an authority decision. This does not select reviewer counts or risk thresholds.

**Proportional profiles and required reviewers.** Before 004-C, establish
auditable classification rules for trivial/standard/critical work and the qualified
review/approval obligations applicable to each contract. Keep contract-mandated
reviews even when automation is convenient; do not impose multi-agent or formal
methods on every task. Dependencies: CAP-WFL-002, CAP-CORE-003, and CAP-EVL-002.
