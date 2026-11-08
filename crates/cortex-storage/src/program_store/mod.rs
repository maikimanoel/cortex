//! Explicit encrypted GPE-B Stage 1 program/Evidence journal (CAP-WFL-001/004).
//!
//! Separate format/policy and managed database from canonical history. One trusted
//! Linux operator, cooperating clients, responsive local filesystem. No automatic
//! capture/export/migration or external execution. SQLCipher DELETE/EXTRA is the
//! synchronization boundary; no universal power-loss or exactly-once claim.
//! A caller supplies an already accepted contract and explicit key/location:
//! ```no_run
//! use cortex_core::program::ProgramDefinition;
//! use cortex_storage::{history_store::{StoreKey, Access}, program_store::{ProgramStore, ReadLimits}};
//! fn create_program(path: &std::path::Path, key: &StoreKey,
//!     definition: ProgramDefinition, limits: ReadLimits) -> Result<(), Box<dyn std::error::Error>> {
//!     let store = ProgramStore::create(path, key, definition, limits)?;
//!     let state = store.load(key, Access::ReadOnly)?;
//!     assert_eq!(state.revision(), 0); // No work completion or permission is inferred.
//!     Ok(())
//! }
//! ```
//!
//! Read deadlines are cooperative checks around bounded engine/replay operations,
//! not preemption of a blocked kernel I/O call. Encoded bounds are not RSS quotas.

mod codec;
use crate::history_store::{
    self, Access, Completion, ErrorKind, StoreError, StoreId, StoreKey, location::Location,
};
use cortex_core::program::ProgramDefinition;
use cortex_evidence::program::{
    Command, CoordinationKernel, KernelError, KernelReceipt, Operation,
};
use rusqlite::{Connection, TransactionBehavior};
use std::{
    error::Error,
    fmt, fs,
    io::Read,
    path::Path,
    time::{Duration, Instant},
};
use zeroize::Zeroizing;

/// Explicitly selected program journal contract. No implicit upgrade is performed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgramProfile {
    /// Original Stage 1 metadata-only journal; fixture effect inputs are refused.
    MetadataV1,
    /// Stage 2A controlled effect fixtures; never real dispatch authorization.
    FixturesV2,
    /// Explicit Stage 2B trusted local process fixtures; never an AI executor grant.
    SupervisedFixturesV3,
    /// One shared-pool coordinator with recoverable provider-free domain records.
    RecoverableDomainsV4,
}
impl ProgramProfile {
    fn format(self) -> i64 {
        match self {
            Self::MetadataV1 => 1,
            Self::FixturesV2 => 2,
            Self::SupervisedFixturesV3 => 3,
            Self::RecoverableDomainsV4 => 4,
        }
    }
    fn policy(self) -> &'static str {
        match self {
            Self::MetadataV1 => "local-operator-program-metadata-v1",
            Self::FixturesV2 => "local-operator-program-fixtures-v2",
            Self::SupervisedFixturesV3 => "local-operator-program-supervised-fixtures-v3",
            Self::RecoverableDomainsV4 => "local-operator-recoverable-domains-v4",
        }
    }
    fn check(self, input: &Operation) -> Result<(), ProgramStoreError> {
        use cortex_evidence::program::Command;
        if let Command::DomainMember { operation, .. } = &input.command
            && matches!(
                operation.command,
                Command::Domain(_)
                    | Command::DomainMember { .. }
                    | Command::RegisterDomainMember(_)
            )
        {
            return Err(ProgramStoreError::Unsupported);
        }
        if matches!(
            input.command,
            Command::Domain(_) | Command::DomainMember { .. } | Command::RegisterDomainMember(_)
        ) && self != Self::RecoverableDomainsV4
        {
            return Err(ProgramStoreError::Unsupported);
        }
        if matches!(input.command, Command::SupervisedEffect(_))
            && !matches!(
                self,
                Self::SupervisedFixturesV3 | Self::RecoverableDomainsV4
            )
            || matches!(input.command, Command::FixtureEffect(_))
                && matches!(
                    self,
                    Self::SupervisedFixturesV3 | Self::RecoverableDomainsV4
                )
        {
            return Err(ProgramStoreError::Unsupported);
        }
        if self == Self::MetadataV1
            && matches!(
                input.command,
                cortex_evidence::program::Command::FixtureEffect(_)
                    | cortex_evidence::program::Command::PrepareExecutorInput(_)
            )
        {
            Err(ProgramStoreError::Unsupported)
        } else {
            Ok(())
        }
    }
    fn kernel(self, initial: ProgramDefinition) -> Result<CoordinationKernel, KernelError> {
        match self {
            Self::SupervisedFixturesV3 => CoordinationKernel::new_supervised_fixture(initial),
            Self::RecoverableDomainsV4 => CoordinationKernel::new_domain_fixture(initial),
            _ => CoordinationKernel::new(initial),
        }
    }
}
const META: &str = "CREATE TABLE program_meta(singleton INTEGER PRIMARY KEY CHECK(singleton=1), format INTEGER NOT NULL, instance BLOB NOT NULL, policy TEXT NOT NULL, initial BLOB NOT NULL)";
const ENTRIES: &str = "CREATE TABLE program_inputs(ordinal INTEGER PRIMARY KEY, operation TEXT NOT NULL UNIQUE, payload BLOB NOT NULL)";

/// Typed non-disclosing store error; missing/corrupt stores never imply absence.
#[derive(Debug)]
pub enum ProgramStoreError {
    /// Shared encrypted local-store boundary failed.
    Storage(StoreError),
    /// Core/Evidence rejected a new operation.
    Domain(KernelError),
    /// Version/schema/canonical encoding/reconstruction is invalid.
    Corrupt,
    /// Unknown program format or policy; never migrate implicitly.
    Unsupported,
    /// Caller-selected finite encoded/count limit was exceeded.
    Capacity,
    /// Cooperative read/reconstruction deadline reached.
    Deadline,
    /// Store incarnation changed at the explicit location.
    TargetChanged,
    /// Commit/create acknowledgment was not established; reconcile explicitly.
    Indeterminate,
}
impl fmt::Display for ProgramStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(e) => e.fmt(f),
            Self::Domain(e) => e.fmt(f),
            other => write!(f, "program store: {other:?}"),
        }
    }
}
impl Error for ProgramStoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Storage(e) => Some(e),
            Self::Domain(e) => Some(e),
            _ => None,
        }
    }
}
impl From<StoreError> for ProgramStoreError {
    fn from(e: StoreError) -> Self {
        Self::Storage(e)
    }
}
impl From<KernelError> for ProgramStoreError {
    fn from(e: KernelError) -> Self {
        Self::Domain(e)
    }
}
impl From<std::io::Error> for ProgramStoreError {
    fn from(e: std::io::Error) -> Self {
        Self::Storage(e.into())
    }
}
fn sql(e: rusqlite::Error) -> ProgramStoreError {
    history_store::sql(e).into()
}

/// Explicit finite inspection/admission reconstruction limits; no default policy.
#[derive(Debug, Clone, Copy)]
pub struct ReadLimits {
    bytes: usize,
    timeout: Duration,
}
impl ReadLimits {
    /// Bound encoded input and cooperative elapsed read/replay time.
    pub fn new(bytes: usize, timeout: Duration) -> Result<Self, ProgramStoreError> {
        if bytes == 0
            || bytes > 64 * 1024 * 1024
            || timeout.is_zero()
            || Instant::now().checked_add(timeout).is_none()
        {
            return Err(ProgramStoreError::Capacity);
        }
        Ok(Self { bytes, timeout })
    }
    /// Maximum retained encoded inputs permitted by this caller.
    pub fn bytes(self) -> usize {
        self.bytes
    }
    /// Cooperative elapsed deadline, including lock waiting.
    pub fn timeout(self) -> Duration {
        self.timeout
    }
}
struct Budget {
    start: Instant,
    limits: ReadLimits,
    used: usize,
}
impl Budget {
    fn new(limits: ReadLimits) -> Self {
        Self {
            start: Instant::now(),
            limits,
            used: 0,
        }
    }
    fn check(&self) -> Result<(), ProgramStoreError> {
        if self.start.elapsed() >= self.limits.timeout {
            Err(ProgramStoreError::Deadline)
        } else {
            Ok(())
        }
    }
    fn add(&mut self, n: usize) -> Result<(), ProgramStoreError> {
        self.check()?;
        self.used = self
            .used
            .checked_add(n)
            .ok_or(ProgramStoreError::Capacity)?;
        if self.used > self.limits.bytes {
            Err(ProgramStoreError::Capacity)
        } else {
            Ok(())
        }
    }
}
/// Exact successful metadata admission, not a verification or acceptance judgment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdmissionReceipt {
    /// Expected encrypted store incarnation.
    pub store: StoreId,
    /// Retained operation ordinal.
    pub ordinal: u64,
    /// Exact duplicate; no append occurred.
    pub duplicate: bool,
}
/// Admission failure explicitly separates rejection/rollback from uncertainty.
#[derive(Debug)]
pub struct AdmissionFailure {
    /// Whether a commit may have occurred.
    pub completion: Completion,
    /// Non-disclosing typed cause.
    pub error: ProgramStoreError,
}
impl fmt::Display for AdmissionFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "program admission {:?}: {}", self.completion, self.error)
    }
}
impl Error for AdmissionFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.error)
    }
}
impl AdmissionFailure {
    fn rejected(e: impl Into<ProgramStoreError>) -> Self {
        Self {
            completion: Completion::Rejected,
            error: e.into(),
        }
    }
}
/// Reconciliation compares the whole input, initial baseline and store identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reconciliation {
    /// Exact input was committed at this ordinal.
    Present(AdmissionReceipt),
    /// Confirmed absent at the exact expected predecessor; no automatic retry.
    Absent,
    /// Same identity differs, or predecessor/initial baseline changed.
    Conflict,
}
/// Explicit matching whole-store removal scope; never selected implicitly.
#[derive(Debug, Clone, Copy)]
pub struct RemovalRequest {
    store: StoreId,
}
impl RemovalRequest {
    /// Explicitly select the observed program-store incarnation.
    pub fn whole_store(store: StoreId) -> Self {
        Self { store }
    }
}
/// Single-owner handle to an existing encrypted program journal.
/// Debug omits paths, contracts and keys; methods recheck incarnation under lock.
pub struct ProgramStore {
    location: Location,
    id: StoreId,
    limits: ReadLimits,
    profile: ProgramProfile,
}
impl fmt::Debug for ProgramStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ProgramStore([redacted])")
    }
}
impl ProgramStore {
    /// Create an explicit private store outside every declared source repository.
    /// Caller must provide finite limits and a raw key. Partial creation is retained.
    pub fn create(
        path: &Path,
        key: &StoreKey,
        initial: ProgramDefinition,
        limits: ReadLimits,
    ) -> Result<Self, ProgramStoreError> {
        Self::create_profile(path, key, initial, limits, ProgramProfile::MetadataV1)
    }
    /// Create a distinct controlled-fixture journal. This is not a real effect
    /// store or runtime grant, and never changes an existing format 1 collection.
    pub fn create_fixture(
        path: &Path,
        key: &StoreKey,
        initial: ProgramDefinition,
        limits: ReadLimits,
    ) -> Result<Self, ProgramStoreError> {
        Self::create_profile(path, key, initial, limits, ProgramProfile::FixturesV2)
    }
    /// Create a separate explicit journal for supervised trusted local fixtures.
    /// Neither creation nor replay launches a process or activates a host backend.
    /// Metadata/history format 1 and pure fixture format 2 are never upgraded.
    pub fn create_supervised_fixture(
        path: &Path,
        key: &StoreKey,
        initial: ProgramDefinition,
        limits: ReadLimits,
    ) -> Result<Self, ProgramStoreError> {
        Self::create_profile(
            path,
            key,
            initial,
            limits,
            ProgramProfile::SupervisedFixturesV3,
        )
    }
    /// Create one explicit shared-pool fixture coordinator. It must be the sole
    /// registered journal for its physical pool under the cooperating-operator
    /// contract; copying a store creates no additional capacity or permission.
    pub fn create_domain_fixture(
        path: &Path,
        key: &StoreKey,
        initial: ProgramDefinition,
        limits: ReadLimits,
    ) -> Result<Self, ProgramStoreError> {
        Self::create_profile(
            path,
            key,
            initial,
            limits,
            ProgramProfile::RecoverableDomainsV4,
        )
    }
    fn create_profile(
        path: &Path,
        key: &StoreKey,
        initial: ProgramDefinition,
        limits: ReadLimits,
        profile: ProgramProfile,
    ) -> Result<Self, ProgramStoreError> {
        let kernel = profile.kernel(initial)?;
        let bytes = codec::definition(kernel.initial())?;
        let mut budget = Budget::new(limits);
        budget.add(bytes.len())?;
        // Location exclusion checks all roots, including multiple linked worktrees.
        let parent = path
            .parent()
            .ok_or_else(|| StoreError::new(ErrorKind::InvalidLocation))?
            .canonicalize()?;
        let name = path
            .file_name()
            .ok_or_else(|| StoreError::new(ErrorKind::InvalidLocation))?;
        let target = parent.join(name);
        check_source_exclusion(&target, kernel.initial())?;
        let roots: Vec<_> = kernel
            .initial()
            .graph()
            .tasks()
            .iter()
            .flat_map(|m| {
                let r = m.context().contract().candidate().repository();
                [
                    r.root().to_owned(),
                    r.git_dir().to_owned(),
                    r.common_dir().to_owned(),
                ]
            })
            .collect();
        let location = Location::program_create(path, &roots)?;
        let _lock = location.lock_for(limits.timeout)?;
        budget.check()?;
        let mut id = [0; 16];
        fs::File::open("/dev/urandom")?.read_exact(&mut id)?;
        location.create_database()?;
        let mut conn = history_store::connect(&location, key, Access::Recover)?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sql)?;
        tx.execute_batch(META).map_err(sql)?;
        tx.execute_batch(ENTRIES).map_err(sql)?;
        tx.execute(
            "INSERT INTO program_meta VALUES(1,?1,?2,?3,?4)",
            rusqlite::params![
                profile.format(),
                id.as_slice(),
                profile.policy(),
                bytes.as_slice()
            ],
        )
        .map_err(sql)?;
        budget.check()?;
        tx.commit().map_err(|_| ProgramStoreError::Indeterminate)?;
        location
            .sync()
            .map_err(|_| ProgramStoreError::Indeterminate)?;
        Ok(Self {
            location,
            id: StoreId::from_bytes(id),
            limits,
            profile,
        })
    }
    /// Read one explicit store once under a single cooperative deadline. This
    /// read-only entry point neither creates a handle with write access nor recovers.
    pub fn inspect(
        path: &Path,
        key: &StoreKey,
        limits: ReadLimits,
    ) -> Result<(StoreId, CoordinationKernel), ProgramStoreError> {
        let mut budget = Budget::new(limits);
        let location = Location::program_open(path)?;
        let _lock = location.lock_for(limits.timeout)?;
        location.ready()?;
        budget.check()?;
        let conn = history_store::connect(&location, key, Access::ReadOnly)?;
        load(&conn, &location, &mut budget).map(|(id, kernel, _)| (id, kernel))
    }
    /// Open existing data; read-only refuses journals rather than recovering.
    pub fn open(
        path: &Path,
        key: &StoreKey,
        access: Access,
        limits: ReadLimits,
    ) -> Result<Self, ProgramStoreError> {
        let mut budget = Budget::new(limits);
        let location = Location::program_open(path)?;
        let _lock = location.lock_for(limits.timeout)?;
        location.ready()?;
        budget.check()?;
        let conn = history_store::connect(&location, key, access)?;
        let (id, _, profile) = load(&conn, &location, &mut budget)?;
        Ok(Self {
            location,
            id,
            limits,
            profile,
        })
    }
    /// Persisted profile; matching source contracts do not upgrade its authority.
    pub fn profile(&self) -> ProgramProfile {
        self.profile
    }
    /// Selected cooperative reconstruction/lock limits. Supervisors must account
    /// for these limits separately from child deadlines; they cannot preempt I/O.
    pub fn read_limits(&self) -> ReadLimits {
        self.limits
    }
    /// Exact expected store incarnation, independent of program/candidate identity.
    pub fn id(&self) -> StoreId {
        self.id
    }
    /// Reconstruct through Core/Evidence from complete canonical inputs. This does
    /// not recapture Git or establish live freshness. Read-only performs no writes.
    pub fn load(
        &self,
        key: &StoreKey,
        access: Access,
    ) -> Result<CoordinationKernel, ProgramStoreError> {
        let mut budget = Budget::new(self.limits);
        let _lock = self.location.lock_for(self.limits.timeout)?;
        self.location.ready()?;
        budget.check()?;
        let conn = history_store::connect(&self.location, key, access)?;
        let (id, kernel, profile) = load(&conn, &self.location, &mut budget)?;
        if id != self.id || profile != self.profile {
            return Err(ProgramStoreError::TargetChanged);
        }
        Ok(kernel)
    }
    /// Reload, validate and append one complete expected-version operation under
    /// one lifecycle lock and IMMEDIATE transaction. No partial claim or release.
    pub fn admit(
        &self,
        key: &StoreKey,
        input: Operation,
    ) -> Result<AdmissionReceipt, AdmissionFailure> {
        self.admit_observed(key, input, |_, _| {})
    }
    fn admit_observed(
        &self,
        key: &StoreKey,
        input: Operation,
        observe: impl FnMut(Stage, &Connection),
    ) -> Result<AdmissionReceipt, AdmissionFailure> {
        self.admit_coordinated(key, input, observe, |_| ())
            .map(|(receipt, _)| receipt)
    }
    /// Commit a new supervised fixture dispatch before invoking its launch boundary.
    /// The lifecycle lock remains held through the short callback, preventing a
    /// cooperating admission from changing the grant between commit and launch.
    /// Exact duplicate inputs return `None` and NEVER invoke the callback. Failed
    /// or uncertain commits also never invoke it. A crash after commit retains
    /// dispatch uncertainty; reopening or retrying this method does not relaunch.
    ///
    /// The callback belongs to the trusted local supervisor, must not reenter this
    /// store or wait for process completion, and is not a sandbox. It receives the
    /// exact validated post-admission projection for final correspondence checks.
    /// Its result is an observation, not an atomic external-effect commit. A panic,
    /// lost reply or later storage failure leaves the durable uncertainty intact.
    /// Responsive filesystem and cooperative-lock assumptions remain unchanged.
    pub fn dispatch_fixture<R>(
        &self,
        key: &StoreKey,
        input: Operation,
        launch: impl FnOnce(&CoordinationKernel) -> R,
    ) -> Result<(AdmissionReceipt, Option<R>), AdmissionFailure> {
        if self.profile != ProgramProfile::SupervisedFixturesV3
            || !matches!(
                &input.command,
                cortex_evidence::program::Command::SupervisedEffect(event)
                if matches!(**event, cortex_core::program::operational::EffectEvent::Dispatch { .. })
            )
        {
            return Err(AdmissionFailure::rejected(ProgramStoreError::Unsupported));
        }
        self.admit_coordinated(key, input, |_, _| {}, launch)
    }
    /// Commit a version-four creation/release/cancellation intent before one short trusted
    /// callback, under the existing lifecycle lock. Duplicate or uncertain
    /// admission never calls it; replay only reconstructs the retained intent.
    /// No transaction spans the manager. A lost external reply stays uncertain.
    pub fn dispatch_domain<R>(
        &self,
        key: &StoreKey,
        input: Operation,
        launch: impl FnOnce(&CoordinationKernel) -> R,
    ) -> Result<(AdmissionReceipt, Option<R>), AdmissionFailure> {
        use cortex_core::program::domain::DomainEvent;
        if self.profile != ProgramProfile::RecoverableDomainsV4
            || !matches!(&input.command, cortex_evidence::program::Command::Domain(e) if matches!(e.as_ref(), DomainEvent::CreateRequested { .. } | DomainEvent::ReleaseRequested { .. } | DomainEvent::Cancel { .. }))
        {
            return Err(AdmissionFailure::rejected(ProgramStoreError::Unsupported));
        }
        self.admit_coordinated(key, input, |_, _| {}, launch)
    }
    fn admit_coordinated<R>(
        &self,
        key: &StoreKey,
        input: Operation,
        mut observe: impl FnMut(Stage, &Connection),
        launch: impl FnOnce(&CoordinationKernel) -> R,
    ) -> Result<(AdmissionReceipt, Option<R>), AdmissionFailure> {
        let mut budget = Budget::new(self.limits);
        let _lock = self
            .location
            .lock_for(self.limits.timeout)
            .map_err(AdmissionFailure::rejected)?;
        self.location.ready().map_err(AdmissionFailure::rejected)?;
        budget.check().map_err(AdmissionFailure::rejected)?;
        let mut conn = history_store::connect(&self.location, key, Access::Recover)
            .map_err(AdmissionFailure::rejected)?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|e| AdmissionFailure::rejected(sql(e)))?;
        let mut validation =
            || -> Result<(KernelReceipt, Zeroizing<Vec<u8>>, CoordinationKernel), ProgramStoreError> {
                let (id, k, profile) = load(&tx, &self.location, &mut budget)?;
                if id != self.id || profile != self.profile {
                    return Err(ProgramStoreError::TargetChanged);
                }
                profile.check(&input)?;
                let encoded = codec::operation(&input, k.initial().limits().bytes)?;
                let mut draft = k.clone();
                let receipt = draft.admit(input.clone())?;
                // Historical duplicates were already decoded by load. New inputs
                // must also reconstruct before any write can be acknowledged.
                if !receipt.duplicate {
                    if codec::decode_operation(&encoded, &k)? != input {
                        return Err(ProgramStoreError::Corrupt);
                    }
                    budget.add(encoded.len())?;
                    if budget.used > k.initial().limits().bytes {
                        return Err(ProgramStoreError::Capacity);
                    }
                }
                check_kernel_source_exclusion(self.location.path(), &draft)?;
                Ok((receipt, encoded, draft))
            };
        let (receipt, encoded, draft) = match validation() {
            Ok(v) => v,
            Err(error) => {
                return Err(AdmissionFailure {
                    completion: if tx.rollback().is_ok() {
                        Completion::Rejected
                    } else {
                        Completion::Indeterminate
                    },
                    error,
                });
            }
        };
        if receipt.duplicate {
            tx.rollback().map_err(|e| AdmissionFailure {
                completion: Completion::Indeterminate,
                error: sql(e),
            })?;
            return Ok((
                AdmissionReceipt {
                    store: self.id,
                    ordinal: receipt.ordinal,
                    duplicate: true,
                },
                None,
            ));
        }
        observe(Stage::BeforeWrite, &tx);
        let ordinal = i64::try_from(receipt.ordinal)
            .map_err(|_| AdmissionFailure::rejected(ProgramStoreError::Capacity))?;
        if let Err(e) = tx.execute(
            "INSERT INTO program_inputs VALUES(?1,?2,?3)",
            rusqlite::params![ordinal, input.id.as_str(), encoded.as_slice()],
        ) {
            return Err(AdmissionFailure {
                completion: if tx.is_autocommit() || tx.rollback().is_ok() {
                    Completion::RolledBack
                } else {
                    Completion::Indeterminate
                },
                error: sql(e),
            });
        }
        observe(Stage::BeforeCommit, &tx);
        if let Err(error) = budget.check() {
            return Err(AdmissionFailure {
                completion: if tx.rollback().is_ok() {
                    Completion::RolledBack
                } else {
                    Completion::Indeterminate
                },
                error,
            });
        }
        tx.commit().map_err(|e| AdmissionFailure {
            completion: Completion::Indeterminate,
            error: sql(e),
        })?;
        observe(Stage::AfterCommit, &conn);
        budget.check().map_err(|error| AdmissionFailure {
            completion: Completion::Indeterminate,
            error,
        })?;
        let receipt = AdmissionReceipt {
            store: self.id,
            ordinal: receipt.ordinal,
            duplicate: false,
        };
        Ok((receipt, Some(launch(&draft))))
    }
    /// Reconcile an uncertain input with exact initial contract and predecessor.
    /// Any error, including a missing store, remains unresolved rather than absent.
    pub fn reconcile(
        &self,
        key: &StoreKey,
        initial: &ProgramDefinition,
        input: &Operation,
        access: Access,
    ) -> Result<Reconciliation, ProgramStoreError> {
        let k = self.load(key, access)?;
        if k.initial() != initial {
            return Ok(Reconciliation::Conflict);
        }
        for (i, o) in k.operations().iter().enumerate() {
            if o.id == input.id {
                return Ok(if o == input {
                    Reconciliation::Present(AdmissionReceipt {
                        store: self.id,
                        ordinal: i as u64 + 1,
                        duplicate: true,
                    })
                } else {
                    Reconciliation::Conflict
                });
            }
        }
        Ok(if k.revision() == input.expected {
            Reconciliation::Absent
        } else {
            Reconciliation::Conflict
        })
    }
    /// Explicitly remove managed program payloads only. Empty directory/lock remain;
    /// sectors, caller copies, swap, backups and filesystem snapshots are not erased.
    pub fn remove(&self, key: &StoreKey, request: RemovalRequest) -> Result<(), ProgramStoreError> {
        if request.store != self.id {
            return Err(StoreError::new(ErrorKind::AuthorizationRequired).into());
        }
        let _lock = self.location.lock_for(self.limits.timeout)?;
        self.location.files()?;
        let marker = self.location.intent()?;
        if marker.is_some_and(|id| id != self.id) {
            return Err(ProgramStoreError::TargetChanged);
        }
        if self.location.database_exists()? {
            let conn = history_store::connect(&self.location, key, Access::Recover)?;
            let (id, _, profile) = identity(&conn, self.limits.bytes)?;
            if id != self.id || profile != self.profile {
                return Err(ProgramStoreError::TargetChanged);
            }
        } else if marker.is_none() {
            return Err(StoreError::new(ErrorKind::Missing).into());
        }
        if marker.is_none() {
            self.location.write_intent(self.id)?;
        }
        for name in ["program.db", "program.db-journal"] {
            self.location.unlink(name)?;
        }
        self.location.sync()?;
        self.location.unlink(".removing")?;
        self.location.sync()?;
        Ok(())
    }
    /// Resume only a retained matching removal intent. No payload is recreated.
    pub fn resume_removal(
        path: &Path,
        key: &StoreKey,
        request: RemovalRequest,
        limits: ReadLimits,
    ) -> Result<(), ProgramStoreError> {
        let location = Location::program_open(path)?;
        {
            let _lock = location.lock_for(limits.timeout)?;
            if location.intent()? != Some(request.store) {
                return Err(StoreError::new(ErrorKind::AuthorizationRequired).into());
            }
        }
        let profile = {
            let _lock = location.lock_for(limits.timeout)?;
            if location.database_exists()? {
                let conn = history_store::connect(&location, key, Access::Recover)?;
                identity(&conn, limits.bytes)?.2
            } else {
                ProgramProfile::MetadataV1
            }
        };
        Self {
            location,
            id: request.store,
            limits,
            profile,
        }
        .remove(key, request)
    }
}
#[derive(Clone, Copy)]
enum Stage {
    BeforeWrite,
    BeforeCommit,
    AfterCommit,
}
fn identity(
    c: &Connection,
    max: usize,
) -> Result<(StoreId, Vec<u8>, ProgramProfile), ProgramStoreError> {
    let mut schema = c
        .prepare("SELECT name,sql FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%' ORDER BY name")
        .map_err(sql)?;
    let names = schema
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .map_err(sql)?
        .take(3)
        .collect::<Result<Vec<_>, _>>()
        .map_err(sql)?;
    if names
        != [
            ("program_inputs".into(), ENTRIES.into()),
            ("program_meta".into(), META.into()),
        ]
    {
        return Err(ProgramStoreError::Corrupt);
    }
    if c.query_row("SELECT count(*) FROM program_meta", [], |r| {
        r.get::<_, i64>(0)
    })
    .map_err(sql)?
        != 1
    {
        return Err(ProgramStoreError::Corrupt);
    }
    let (format, id, policy, initial) = c
        .query_row(
            "SELECT format,instance,policy,initial FROM program_meta WHERE singleton=1",
            [],
            |r| {
                let id = r.get_ref(1)?.as_blob()?;
                let b = r.get_ref(3)?.as_blob()?;
                if id.len() != 16 || b.len() > max {
                    return Err(rusqlite::Error::InvalidQuery);
                }
                Ok((
                    r.get::<_, i64>(0)?,
                    id.to_vec(),
                    r.get::<_, String>(2)?,
                    b.to_vec(),
                ))
            },
        )
        .map_err(|_| ProgramStoreError::Corrupt)?;
    let profile = [
        ProgramProfile::MetadataV1,
        ProgramProfile::FixturesV2,
        ProgramProfile::SupervisedFixturesV3,
        ProgramProfile::RecoverableDomainsV4,
    ]
    .into_iter()
    .find(|p| p.format() == format && p.policy() == policy)
    .ok_or(ProgramStoreError::Unsupported)?;
    Ok((
        StoreId::from_bytes(id.try_into().map_err(|_| ProgramStoreError::Corrupt)?),
        initial,
        profile,
    ))
}
fn check_kernel_source_exclusion(
    path: &Path,
    kernel: &CoordinationKernel,
) -> Result<(), ProgramStoreError> {
    for definition in kernel.state().definitions() {
        check_source_exclusion(path, definition)?;
    }
    for member in kernel.domain_members() {
        for definition in member.state().definitions() {
            check_source_exclusion(path, definition)?;
        }
    }
    Ok(())
}

fn load(
    c: &Connection,
    location: &Location,
    budget: &mut Budget,
) -> Result<(StoreId, CoordinationKernel, ProgramProfile), ProgramStoreError> {
    budget.check()?;
    if c.query_row("PRAGMA quick_check", [], |r| r.get::<_, String>(0))
        .map_err(sql)?
        != "ok"
    {
        return Err(ProgramStoreError::Corrupt);
    }
    let (id, initial, profile) = identity(c, budget.limits.bytes)?;
    budget.add(initial.len())?;
    let definition = codec::decode_definition(&initial, budget.limits.bytes)?;
    check_source_exclusion(location.path(), &definition)?;
    let mut kernel = profile
        .kernel(definition)
        .map_err(|_| ProgramStoreError::Corrupt)?;
    let mut statement = c
        .prepare("SELECT ordinal,operation,payload FROM program_inputs ORDER BY ordinal")
        .map_err(sql)?;
    let mut rows = statement.query([]).map_err(sql)?;
    let mut source_changed = false;
    kernel = kernel
        .reconstruct(|kernel| -> Result<Option<Operation>, ProgramStoreError> {
            if source_changed {
                check_kernel_source_exclusion(location.path(), kernel)?;
            }
            let Some(row) = rows.next().map_err(sql)? else {
                return Ok(None);
            };
            if kernel.operations().len() >= kernel.initial().limits().entries {
                return Err(ProgramStoreError::Corrupt);
            }
            let ordinal: i64 = row.get(0).map_err(sql)?;
            if ordinal <= 0 || ordinal as u64 != kernel.revision() + 1 {
                return Err(ProgramStoreError::Corrupt);
            }
            let bytes = row
                .get_ref(2)
                .map_err(sql)?
                .as_blob()
                .map_err(|_| ProgramStoreError::Corrupt)?;
            budget.add(bytes.len())?;
            if budget.used > kernel.initial().limits().bytes {
                return Err(ProgramStoreError::Capacity);
            }
            let input = codec::decode_operation(bytes, kernel)?;
            profile.check(&input)?;
            if row
                .get_ref(1)
                .map_err(sql)?
                .as_str()
                .map_err(|_| ProgramStoreError::Corrupt)?
                != input.id.as_str()
            {
                return Err(ProgramStoreError::Corrupt);
            }
            source_changed = matches!(
                &input.command,
                Command::Amend { .. }
                    | Command::RegisterDomainMember(_)
                    | Command::DomainMember { .. }
            );
            Ok(Some(input))
        })
        .map_err(|error| match error {
            // Invalid retained transitions remain corruption, while new admission
            // continues to report its typed domain rejection.
            ProgramStoreError::Domain(_) => ProgramStoreError::Corrupt,
            other => other,
        })?;
    check_kernel_source_exclusion(location.path(), &kernel)?;
    budget.check()?;
    Ok((id, kernel, profile))
}

fn check_source_exclusion(
    target: &Path,
    definition: &ProgramDefinition,
) -> Result<(), ProgramStoreError> {
    for task in definition.graph().tasks() {
        let repo = task.context().contract().candidate().repository();
        for root in [repo.root(), repo.git_dir(), repo.common_dir()] {
            let resolved = root.canonicalize().unwrap_or_else(|_| root.to_owned());
            if target.starts_with(resolved) {
                return Err(StoreError::new(ErrorKind::InvalidLocation).into());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
