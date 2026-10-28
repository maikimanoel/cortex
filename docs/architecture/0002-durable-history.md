# 0002: First encrypted local canonical-history store

Status: accepted for the bounded 003-A implementation under CAP-MEM-001 and
CAP-SEC-002. Runtime verification is scoped to the candidate/environment recorded
in its external handoff. This does not deliver full 003-A or Milestone 003.

## Decision classification

| Classification | Status and scope |
| --- | --- |
| Permanent principles | Approved: deterministic Rust Core, typed authority, candidate-bound evidence/uncertainty, temporal provenance, domain/infrastructure separation, local-first operation, free baseline, explicit consequential authority, maintainability and interoperability. Their full enforcement remains capability-specific, not a claim that the permission engine exists |
| Initial implementation | Approved and implemented: SQLite through rusqlite, SQLCipher Community encryption, one explicit local operator/store, explicit canonical submissions, manual retention without eviction, whole-collection removal and no automatic external copies. This is an initial profile, not a permanent multi-user/backend restriction |
| Qualified configuration | Local functional tests: Rust 1.99.1, Linux x86_64, rusqlite 0.41.0, libsqlite3-sys 0.39.1, SQLCipher 4.19.0 Community/SQLite 3.53.4 and system OpenSSL 3.6.5. Other hosts/providers and physical power-loss behavior are unverified |
| Open hypotheses | Proposed future work: additional host qualification, key rotation/recovery, shared-user governance, synchronization, targeted lifecycle views and future codecs/adapters. No keychain, public interchange or migration framework is selected |

The catalog owns requirements/status; [temporal records](temporal-records.md) own
domain/admission rules. This decision records the actual engine/key/lifecycle
choices without creating a second registry of capabilities or authority.

## Context, alternatives and rationale

The in-memory history is authoritative for constructors, chains, chronology and
queries. Persistence must survive restart without silently changing that model,
exposing private context or interpreting a lost commit reply as absence.

An append journal or complete snapshot replacement would leave custom crash,
locking and rewrite/deletion protocols in CORTEX. Select one SQLite transaction
adapter instead. rusqlite supplies a maintained safe Rust boundary; its bundled
SQLCipher feature compiles a locked stable Community source package and links
system OpenSSL. The dependency/build/notice cost is justified by transactions,
recovery and encryption. No beta SQLCipher, vendored OpenSSL, second backend,
network service or paid dependency is used. The RustSec lockfile observation does
not authenticate source authors or cover every bundled/native vulnerability.

## Boundary and format

`cortex_cli::history_store` is Linux infrastructure. Core remains std-only; the
only Core extension explicitly borrows `ComparisonBasis::material()` for lossless
encoding, while Debug remains redacted. Evidence and existing commands are unchanged.
No key CLI flag or storage CLI command is added. Explicit library arguments are
the initial consumer interface; a compile-checked consumer example is in Rustdoc.

One private directory contains `history.db`, optional rollback journal, empty
`.lock` and, during removal, a 16-byte random-instance `.removing` intent. Location
selection is explicit, outside associated source repositories. Creation requires
a new directory; existing or missing/corrupt stores are never reset/recreated.
Directories/files use operator ownership, mode 0700/0600 and link/type checks.
All cooperating operations use one stable standard-library file lock, including
open/recovery/removal. This is not isolation from root or malicious same-user code.

Private format 1 contains one exact context, random instance ID and policy reference
`local-operator-encrypted-v1`; records have unique version IDs, durable ordinals
and bounded JSON projections. Strict schema/field/tag/version validation precedes
usable reconstruction through Core constructors/append. Unsigned values, byte paths,
optional states, comparison material and seconds/nanoseconds are retained exactly.
There is no public wire protocol, automatic migration, Git reinspection or validity
refresh. A changed or unsupported policy/format is rejected.

## Key and encryption contract

The caller supplies 32 random raw-key bytes through `StoreKey`; empty/wrong-length/
obviously empty keys are rejected, but entropy is not proven by construction.
No production key is hardcoded, persisted, accepted in argv, or included in Debug,
errors, traces or evidence. No key recovery, rotation or passphrase policy is provided.
Loss of the key can make the data irrecoverable. Caller/engine RAM, swap and dumps
are not securely erased; zeroization covers only owned key/encoding buffers.

Keying precedes schema access. The statement contains only fixed syntax and hex
digits from the fixed-size key; no arbitrary input is interpolated. This avoids
rusqlite's PRAGMA helper reading SQLite metadata before keying. No custom cipher,
KDF, unsafe FFI or cryptographic primitive is implemented.

The adapter verifies SQLCipher 4.19.0 Community, OpenSSL 3 release-provider settings,
HMAC on, encrypted header, 4096-byte pages, memory temp storage and DELETE journaling.
Writes verify EXTRA synchronization. Existing WAL mode is refused, not converted.
No extensions, attachments, triggers, virtual tables or arbitrary SQL surface is
provided. Unknown settings/providers fail closed. Runtime compatibility checks do
not establish qualification for every stable OpenSSL 3 installation.

Correct-key reopen and confidential sentinel checks cover database pages, active
rollback journals and inspected process store descriptors. Temporary storage is
configured to memory. This does not prove absence of every plaintext copy or hide
file existence, sizes, access patterns, operator paths or coordination metadata.
Wrong keys and corrupt encrypted headers can be indistinguishable: `CannotUnlock`
is deliberately non-disclosing. There is no plaintext fallback.

## Outcomes, removal and limits

Core rejects invalid drafts before insert. Confirmed rollback, including SQLite's
observed automatic rollback after a failed INSERT, differs from a failed COMMIT:
commit errors remain indeterminate. Reconciliation compares exact instance,
context and record, returning present/absent/conflict only for a valid loaded store.
An absent result permits fresh Core validation, not blind retries or new identities.

Read-only access refuses a nonempty journal requiring possible recovery; Recover
explicitly permits engine writes. All active connections close at operation end.
Whole-collection removal requires an explicit matching `RemovalRequest`, never a
historical decision. Intent is synchronized before unlinking managed payloads;
pending intent blocks normal opens and requires explicit resume. Unknown files
are preserved and prevent removal. The empty directory/lock remain; no canonical
payload receipt is retained. If the database is already gone, intent matching
permits residual cleanup without authenticating a now-absent database key; this
neither decrypts nor restores information and still trusts the selected operator.

Removal covers only managed live files. It does not sanitize unallocated blocks,
snapshots/backups, swap, crash dumps or caller-owned Core clones. Encryption/key
discard does not prove universal erasure. Targeted redaction/withdrawal is deferred.

Adapter limits: Core's existing history/observation bounds, 64 MiB encoded input/
main database, 256 MiB checked managed working files and five seconds of cooperative
lock contention. Engine page limits bound main-file growth. Serialized operations
and memory-only temp data constrain the covered working set; there is no filesystem
quota, total-RSS or hard I/O deadline guarantee. Larger valid Core values can fail
the narrower encoded limit explicitly. Capacity failure never silently evicts history.

## Verification and reopening

Tests use real files, separate processes, controlled transaction/removal cuts,
engine capacity failure, native lock conflict, permissions and real Git consumers.
They distinguish pre-commit rollback from completed-commit persistence, exact retry
reconciliation, malformed data, unsupported versions and plaintext refusal.
Power interruption, failed physical flush injection and hostile-host assurance are
NOT_RUN; process termination tests do not establish those properties.

Subsequent [bounded fault qualification](temporal-records.md#storage-failure-qualification)
adds real kernel short writes/EFBIG and test-only libc write/sync/unlink error
injection in isolated children. Recovery I/O errors retain their typed cause rather
than becoming key/header failures. SQLite's journal-creation directory sync can
ignore an error; later sync success is observed, not a guarantee that every native
sync error reaches the adapter. Final directory-sync failures remain uncertain.
Damaged removal intents and absent intent after a failed final sync require explicit
operator inspection; the adapter does not invent identity or silently repair them.
These observations refine qualification, without changing the engine/key/profile
decision or establishing physical power-loss behavior.

Reopen this decision for an engine/provider/source/build change, new host, stronger
confidentiality/erasure requirement, multi-user concurrency, rotation/recovery,
targeted lifecycle or format evolution. Requalify affected properties; do not
convert the initial profile into a universal platform restriction. DEC-004/007
retain their broader open scope; DEC-011 snapshots remain separate.
