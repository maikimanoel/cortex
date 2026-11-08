# Explicit semantic dimensions and interface migration

This contract extends the [domain vocabulary](domain-vocabulary.md) under the
existing [invariants](core-invariants.md) and [subsystem ownership](subsystems.md).
The capability catalog and roadmap continue to own status and sequence. It does
not introduce an operational grant, a resource waiver or a new scheduler.

## Decisions and alternatives

A byte count alone cannot distinguish process resident memory from memory charged
to a resource controller. Resource names alone also cannot identify the process,
member set or accounting pool. The selected contract carries both basis and scope.
Renaming every quantity to resident or charged would reinterpret historical data;
keeping an unqualified active quantity would permit invalid comparisons. Historical
values retain an explicitly unspecified basis, without fabricated provenance.

A Knowledge projection has several independent properties: source/profile
correspondence, extraction coverage, emitted-reference resolution and support for
a particular query. A single validity flag cannot answer all four. The selected
contract exposes those dimensions and names the retained aggregate by its actual
resolution-dependent scope. It does not implement general semantic validation.

Memory contracts belong to Core, admission to Evidence's existing composition,
representation/reconstruction to Storage, native observation to Harness and
presentation to CLI. Knowledge's immutable contracts stay in Core; current source
acquisition and lexical adapters stay in CLI. No second ledger or derived index
is introduced.

## Compatibility obligations

Supported historical formats preserve field order, arity, units, values, canonical
bytes and identifiers. A new resource representation has an explicit version and
cannot be encoded as the historical unqualified memory token. Readers that do not
support it refuse it. Reconstruction retains historical uncertainty and does not
launch, retry or infer missing measurements. Synthetic fixtures are used for
compatibility verification; real historical collections are not migrated.

Active Rust APIs migrate together with known consumers. Removed names have no
indefinite duplicate public aliases. External Git/path consumers must migrate;
there is no general Rust ABI or ecosystem compatibility promise. `Resource` and
resource-bearing executor controls are no longer Copy because the dimension owns
its bounded scope identity. Consumers borrow or explicitly clone those values.
OwnedScopeSpec adds optional memory correspondence: None retains historical
encoding; new scoped requests require the validated explicit field.

## Program inspection migration

`cortex program inspect` now defaults to schema 3, equivalent to explicitly
selecting `--schema-version 3`, for both human and JSON output. Versions 1 and 2
remain explicitly selectable. Existing states retain their old selected output
bytes; a newly representable memory basis/scope cannot be flattened into either
old schema and yields `SCHEMA_CANNOT_REPRESENT_MEMORY` instead.

This default transition is intentionally incompatible for scripts that depended
on implicit version 1. Such consumers must pin the retained version for old
states or migrate to the current schema. The `check --json` protocol, command
exit conventions, redaction and read-only storage behavior remain unchanged.
Schema selection supplies no execution authority or new observations.

## Memory basis and scope

`program::memory::MemoryResource` combines a validated basis with an identity
scope. ProcessResident identifies one host/boot/PID/birth lifetime;
SummedProcessResident identifies a sorted, unique, bounded process set and counts
shared pages separately for each process. CgroupCharged identifies the retained
host/boot/manager/invocation and cgroup filesystem incarnation. These definitions
are structurally checked caller assertions, not authenticated observations.

AccountingPool is a separate logical scope with an exact PoolIdentity. Budgets,
capacity, demands and reservations use the same basis and pool incarnation.
Allocation addition is checked for overflow and exact correspondence; it does not
sum peaks, different cgroups or unrelated process snapshots. A process/cgroup
sample is not interchangeable with the logical pool merely because both use
bytes. Comparisons require equal definitions or an explicitly supported retained
allocation/observation correspondence. Missing correspondence remains unknown.

`Resource::UnspecifiedMemoryBytes` represents retained bytes with no established
basis. It preserves the historical `RamBytes` token; it is not a new default
measurement. The reconstruction path preserves old decisions and counters.
Current admission cannot use that uncertainty to create new memory-backed work.
Unknown outcomes and obligations survive reconstruction; a cancellation or
administrative disposition cannot invent usage or refund a reservation.

`MemoryObservation` keeps an optional sampled value distinct from `MemoryLimit`.
A zero sample is known zero; None is unobserved. Requested and read-back limits
are separate roles, neither of which establishes consumption or qualified host
enforcement. Retained native domain identity can supply a cgroup measurement
scope. Older scope observations lack the full host/boot identity, so their known
charged-memory mechanism does not silently establish complete scope correspondence.
An explicitly scoped new request cannot attribute such a raw peak to its pool.
Its accounting remains unknown and recovery retains MissingUsage; an Observed
reconciliation is refused. Historical unspecified records remain reconstructible
under their original rules, without making a new scoped attribution. Their
retained recovery proposal can still contain unspecified quantities for compatible
legacy presentation. It is advisory: current admission refuses a new reconciliation
that depends on unspecified memory, even when that historical proposal exists.

New resources use the explicit `memory-resource-v1` representation. An owned-scope
request with explicit memory correspondence uses `owned-scope-memory-v1`;
requests without that extension retain their historical positional format. No
encoder hides explicit scope beneath the old unqualified token. Historical
readers refuse unsupported extensions rather than interpreting reduced data.

In schema 3, memory resource values carry kind, basis, scope kind and an
output-local scope_id. Equal scope IDs within that one output mean exact retained
identity equality, not authentication or independent physical allocation. Identity
strings remain redacted; the IDs are not stable across outputs. Unspecified
historical memory uses UNESTABLISHED and a null scope ID. Samples, requested
limits, readbacks, declared quantities and retained accounting remain separate
fields and roles. Versions 1/2 retain their earlier representations only where
no new scoped representation would be lost.

## Knowledge inspection contract version 2

The in-memory inspection API exposes four independent dimensions. Supplied
candidate correspondence and exact adapter/profile matching determine contextual
usability. Extraction coverage names a bounded supported subset and deliberate
omissions; an empty result does not establish complete extraction. Reference
resolution counts only emitted occurrences, distinguishing single-source from
explicitly selected-source resolution. Claim inspection answers a specific lexical
proposition with its retained source, extraction identity and byte span.

An unresolved external reference can support its occurrence without supporting
existence of its target or semantic truth. MissingTarget and OutsideCoverage are
separate gaps. SemanticRelationship and CompleteExtraction are explicitly
unsupported claims. An absent occurrence is not established, not evidence that
extraction was complete. Relations remain Extracted and Unverified.

All six current Knowledge view families consume this contract: JSON, Rust,
Markdown, Cargo manifest, selected JSON source sets and refresh. The active
`resolved_projection_validity()` accessor preserves the previous conjunction under
an explicit name: Current requires compatible source/profile correspondence and
resolution of all emitted references; relevant mismatch yields Stale and missing
support remains unestablished. It does not aggregate coverage or arbitrary claim
support. The old Knowledge-view `validity()` accessor is removed; Evidence validity
and CLI check JSON remain unchanged. No Knowledge-view persistence format exists
here; version 2 identifies this API contract, not a fictitious stored encoding.

Adapter view `inspect_claim` checks compatible supplied correspondence and the
supported exact profile. Core `inspect_retained_index_claim` and
`inspect_retained_set_claim` inspect retained structural assertions only; callers
must separately establish freshness and authenticity. `inspect_validity` acquires
no source data. `reinspect` retains its explicit acquisition contract.

## Preserved boundaries

Logical WorkerSlots retain their profile-specific cumulative or reusable policies.
Resident memory, charged memory, declared limits, observations and enforcement
remain separate. Inspection does not acquire sources unless its documented
reinspection operation does so. Retained identity is not authentication, a live
handle, a complete process set or a closure proof. No resource settlement, refund,
Unknown outcome, capability status or milestone is promoted by these changes.

## Review obligations

New public concepts require a definition, owner, units/scope, meaningful neighbor,
effects, representation version and discriminating acceptance example. Tests freeze
specific wire mappings and retired API boundaries; ordinary technical words are
not globally prohibited. Historical tokens, synthetic format fixtures and migration
documentation are deliberate compatibility surfaces, not duplicate active models.
