//! One explicit local operator's encrypted canonical collection (CAP-MEM-001).
//!
//! Keys are caller-supplied 32-byte random secrets, never arguments/configuration.
//! Calls are a trusted local operator boundary, not authentication or an agent grant.
//! Core owns all record semantics. No automatic capture, fallback, backup or migration.
//! Linux local filesystems only; synchronization still depends on the host/storage.
//!
//! The caller supplies an actual observation, record and secret through library
//! arguments; neither a key CLI flag nor a default storage location exists:
//! ```no_run
//! use cortex_cli::history_store::{Access, HistoryStore, StoreKey};
//! use cortex_core::{binding::CandidateObservation, temporal::{CanonicalRecord, CanonicalHistory}};
//! fn persist(path: &std::path::Path, key: &StoreKey, subject: CandidateObservation,
//!            record: CanonicalRecord) -> Result<CanonicalHistory, Box<dyn std::error::Error>> {
//!     let store = HistoryStore::create(path, key, subject)?;
//!     store.admit(key, record)?;
//!     Ok(store.load(key, Access::ReadOnly)?)
//! }
//! ```

mod codec;
mod location;

use cortex_core::{binding::CandidateObservation, temporal::*};
use location::Location;
use rusqlite::{Connection, OpenFlags, TransactionBehavior, limits::Limit};
use std::{error::Error, fmt, fs, io::Read, path::Path, time::Duration};
use zeroize::Zeroizing;

const MAX_BYTES: usize = 64 * 1024 * 1024;
const DATABASE: &str = "history.db";
const JOURNAL: &str = "history.db-journal";
const REMOVING: &str = ".removing";
const FORMAT: i64 = 1;
const POLICY: &str = "local-operator-encrypted-v1";
const META_SCHEMA: &str = "CREATE TABLE meta(singleton INTEGER PRIMARY KEY CHECK(singleton=1), format INTEGER NOT NULL, instance BLOB NOT NULL, context BLOB NOT NULL, policy TEXT NOT NULL)";
const RECORD_SCHEMA: &str = "CREATE TABLE records(ordinal INTEGER PRIMARY KEY, version TEXT NOT NULL UNIQUE, payload BLOB NOT NULL)";

/// Non-disclosing storage failure categories; no underlying SQL/text is exposed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// A key was not supplied.
    KeyRequired,
    /// Key length/obvious empty key is invalid; entropy remains the caller's duty.
    InvalidKey,
    /// Wrong key or invalid encrypted header; observable I/O errors remain separate.
    CannotUnlock,
    /// Local path, file kind, link or ownership does not meet the supported contract.
    InvalidLocation,
    /// The expected store is absent; this is not confirmed non-admission.
    Missing,
    /// Creation would replace an existing directory/store.
    AlreadyExists,
    /// Host or effective engine/format configuration is unsupported.
    Unsupported,
    /// Stored data failed structural or Core validation.
    Corrupt,
    /// Expected instance identity differs from the store at this location.
    TargetChanged,
    /// Explicit whole-collection removal request is missing or mismatched.
    AuthorizationRequired,
    /// Another cooperating operation held the lifecycle lock for five seconds.
    Busy,
    /// A finite input, database or working-file limit was reached.
    Capacity,
    /// Host permissions rejected the operation.
    Permission,
    /// Host I/O failed; no raw path/transcript is included.
    Io,
    /// Core rejected an attempted record transition.
    Domain,
    /// A read-only request requires explicit recovery.
    RecoveryRequired,
    /// A removal intent exists; ordinary opens cannot revive the store.
    RemovalPending,
    /// An unexpected file prevents a safe operation; it is never deleted.
    UnexpectedFiles,
    /// Removal did not confirm all managed files/intent cleanup.
    RemovalIncomplete,
    /// Creation may have committed before an unsuccessful synchronization/reply.
    Indeterminate,
}

/// A bounded diagnostic retaining Core causes without secret/SQL/path payloads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreError {
    kind: ErrorKind,
    domain: Option<TemporalError>,
    io: Option<std::io::ErrorKind>,
    engine: Option<i32>,
}
impl StoreError {
    fn new(kind: ErrorKind) -> Self {
        Self {
            kind,
            domain: None,
            io: None,
            engine: None,
        }
    }
    fn domain(error: TemporalError) -> Self {
        Self {
            kind: ErrorKind::Domain,
            domain: Some(error),
            io: None,
            engine: None,
        }
    }
    /// Inspect the failure category.
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }
}
impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "canonical store: {:?}", self.kind)
    }
}
impl Error for StoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.domain.as_ref().map(|e| e as &dyn Error)
    }
}
impl From<std::io::Error> for StoreError {
    fn from(e: std::io::Error) -> Self {
        let mut error = Self::new(match e.kind() {
            std::io::ErrorKind::PermissionDenied => ErrorKind::Permission,
            std::io::ErrorKind::NotFound => ErrorKind::Missing,
            std::io::ErrorKind::AlreadyExists => ErrorKind::AlreadyExists,
            _ => ErrorKind::Io,
        });
        error.io = Some(e.kind());
        error
    }
}
fn sql(e: rusqlite::Error) -> StoreError {
    use rusqlite::ErrorCode;
    let mut error = StoreError::new(match e.sqlite_error_code() {
        Some(ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked) => ErrorKind::Busy,
        Some(ErrorCode::DiskFull | ErrorCode::TooBig) => ErrorKind::Capacity,
        Some(ErrorCode::PermissionDenied | ErrorCode::ReadOnly) => ErrorKind::Permission,
        Some(ErrorCode::DatabaseCorrupt | ErrorCode::NotADatabase) => ErrorKind::Corrupt,
        _ => ErrorKind::Io,
    });
    error.engine = e.sqlite_error().map(|e| e.extended_code);
    error
}

// Initial schema access may perform authorized hot-journal recovery. Preserve
// I/O/busy failures instead of misreporting them as a wrong encryption key.
fn unlock(e: rusqlite::Error) -> StoreError {
    if matches!(
        e.sqlite_error_code(),
        Some(rusqlite::ErrorCode::NotADatabase | rusqlite::ErrorCode::DatabaseCorrupt)
    ) {
        StoreError::new(ErrorKind::CannotUnlock)
    } else {
        sql(e)
    }
}

/// Caller-owned raw key. Debug is redacted; no Clone/default or key persistence.
///
/// Supply 32 uniformly random bytes from an appropriate generator. Length/nonzero
/// checks do not establish entropy. Zeroizing this copy does not erase caller/engine
/// copies, swap or crash dumps; this is not a secure-memory guarantee.
pub struct StoreKey(Zeroizing<[u8; 32]>);
impl StoreKey {
    /// Validate explicit key material; an absent/empty key is a typed failure.
    pub fn new(bytes: &[u8]) -> Result<Self, StoreError> {
        if bytes.is_empty() {
            return Err(StoreError::new(ErrorKind::KeyRequired));
        }
        let value: [u8; 32] = bytes
            .try_into()
            .map_err(|_| StoreError::new(ErrorKind::InvalidKey))?;
        if value.iter().all(|v| *v == 0) {
            return Err(StoreError::new(ErrorKind::InvalidKey));
        }
        Ok(Self(Zeroizing::new(value)))
    }
}
impl fmt::Debug for StoreKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("StoreKey([redacted])")
    }
}

/// A random local store incarnation, unrelated to candidate identity or permission.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct StoreId([u8; 16]);
impl StoreId {
    /// Inspect a non-secret instance reference for explicit reconciliation/removal.
    pub fn bytes(self) -> [u8; 16] {
        self.0
    }
    /// Reconstruct an instance reference; this does not authenticate it.
    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }
}
impl fmt::Debug for StoreId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("StoreId([redacted])")
    }
}

/// Whether a load explicitly permits recovery writes to the selected store.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    /// No recovery writes; report RecoveryRequired if needed.
    ReadOnly,
    /// Permit engine recovery in this operator-controlled store.
    Recover,
}
/// The storage knowledge retained on an unsuccessful admission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Completion {
    /// Rejected before transaction effects.
    Rejected,
    /// Transaction rollback was explicitly confirmed.
    RolledBack,
    /// Commit/recovery outcome requires reconciliation; never blind retry.
    Indeterminate,
}
/// An admission failure whose completion information must not be discarded.
#[derive(Debug)]
pub struct AdmissionFailure {
    completion: Completion,
    error: StoreError,
}
impl AdmissionFailure {
    /// Inspect whether non-admission is established.
    pub fn completion(&self) -> Completion {
        self.completion
    }
    /// Inspect a safe cause.
    pub fn error(&self) -> &StoreError {
        &self.error
    }
    fn rejected(error: StoreError) -> Self {
        Self {
            completion: Completion::Rejected,
            error,
        }
    }
}
impl fmt::Display for AdmissionFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "admission {:?}: {}", self.completion, self.error)
    }
}
impl Error for AdmissionFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.error)
    }
}
/// Confirmed presence of one version at an ordinal; never an engineering assessment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdmissionReceipt {
    store: StoreId,
    ordinal: u64,
}
impl AdmissionReceipt {
    /// Store incarnation in which presence was observed.
    pub fn store(&self) -> StoreId {
        self.store
    }
    /// Durable append order, independent of recording timestamps.
    pub fn ordinal(&self) -> u64 {
        self.ordinal
    }
}
/// Reconciliation compares full typed context/content, not identity alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reconciliation {
    /// Exact admission is already present.
    Present(AdmissionReceipt),
    /// Complete recovered store confirms absence; retry still requires Core validation.
    Absent,
    /// Same identity/context expectation differs; do not overwrite.
    Conflict,
}
/// Explicit operator request for this whole collection, not an authentication token.
#[derive(Debug, Clone, Copy)]
pub struct RemovalRequest(StoreId);
impl RemovalRequest {
    /// Name the exact incarnation approved by the trusted caller/operator.
    pub fn whole_collection(store: StoreId) -> Self {
        Self(store)
    }
}
/// Scoped removal observation. Empty coordination directory/lock remain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RemovalReceipt {
    store: StoreId,
}
impl RemovalReceipt {
    /// Inspect the removed incarnation; this proves no universal erasure.
    pub fn store(&self) -> StoreId {
        self.store
    }
}

/// Selected private directory and expected incarnation. No connection/key is retained.
///
/// Operations serialize on a stable empty coordination file. Historical caller-owned
/// Core snapshots cannot be revoked. Uncooperative same-user/root modification and
/// network filesystems are outside the qualified local-operator model.
pub struct HistoryStore {
    location: Location,
    id: StoreId,
}
impl fmt::Debug for HistoryStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("HistoryStore([redacted])")
    }
}

impl HistoryStore {
    /// Create a new private directory/store outside associated source repositories.
    ///
    /// Failure after creation may leave recoverable files; never recreates existing data.
    pub fn create(
        path: &Path,
        key: &StoreKey,
        context: CandidateObservation,
    ) -> Result<Self, StoreError> {
        let encoded = codec::encode_context(&context)?;
        let location = Location::create(path, &context)?;
        let _guard = location.lock()?;
        let mut id = [0; 16];
        fs::File::open("/dev/urandom")?.read_exact(&mut id)?;
        location.create_database()?;
        let mut conn = connect(&location, key, Access::Recover)?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sql)?;
        tx.execute_batch(META_SCHEMA).map_err(sql)?;
        tx.execute_batch(RECORD_SCHEMA).map_err(sql)?;
        tx.execute(
            "INSERT INTO meta VALUES(1,?1,?2,?3,?4)",
            rusqlite::params![FORMAT, id.as_slice(), encoded.as_slice(), POLICY],
        )
        .map_err(sql)?;
        tx.commit()
            .map_err(|_| StoreError::new(ErrorKind::Indeterminate))?;
        location
            .sync()
            .map_err(|_| StoreError::new(ErrorKind::Indeterminate))?;
        Ok(Self {
            location,
            id: StoreId(id),
        })
    }

    /// Open existing data without creating a missing store. Recovery requires explicit access.
    pub fn open(path: &Path, key: &StoreKey, access: Access) -> Result<Self, StoreError> {
        let location = Location::open(path)?;
        let _guard = location.lock()?;
        location.ready()?;
        let conn = connect(&location, key, access)?;
        let (id, _) = load(&conn)?;
        Ok(Self { location, id })
    }
    /// Inspect the expected store incarnation.
    pub fn id(&self) -> StoreId {
        self.id
    }
    /// Reconstruct validated Core history, without a Git query or fresh validity claim.
    pub fn load(&self, key: &StoreKey, access: Access) -> Result<CanonicalHistory, StoreError> {
        let _guard = self.location.lock()?;
        self.location.ready()?;
        let conn = connect(&self.location, key, access)?;
        let (id, history) = load(&conn)?;
        if id != self.id {
            return Err(StoreError::new(ErrorKind::TargetChanged));
        }
        Ok(history)
    }
    /// Append exactly one validated version. Failed commit replies remain indeterminate.
    pub fn admit(
        &self,
        key: &StoreKey,
        entry: CanonicalRecord,
    ) -> Result<AdmissionReceipt, AdmissionFailure> {
        self.admit_observed(key, entry, |_, _| {})
    }

    fn admit_observed(
        &self,
        key: &StoreKey,
        entry: CanonicalRecord,
        mut observe: impl FnMut(Stage, &Connection),
    ) -> Result<AdmissionReceipt, AdmissionFailure> {
        let _guard = self.location.lock().map_err(AdmissionFailure::rejected)?;
        self.location.ready().map_err(AdmissionFailure::rejected)?;
        let mut conn =
            connect(&self.location, key, Access::Recover).map_err(AdmissionFailure::rejected)?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|e| AdmissionFailure::rejected(sql(e)))?;
        let validate = || -> Result<(u64, Zeroizing<Vec<u8>>), StoreError> {
            let (id, mut h) = load(&tx)?;
            if id != self.id {
                return Err(StoreError::new(ErrorKind::TargetChanged));
            }
            h.append(entry.clone()).map_err(StoreError::domain)?;
            let ordinal = h.records().len() as u64;
            Ok((ordinal, codec::encode_record(&entry)?))
        };
        let (ordinal, payload) = match validate() {
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
        observe(Stage::BeforeWrite, &tx);
        // Core's collection bound establishes ordinal <= 1024, independently
        // of the full-width u64 values preserved by the payload codec.
        if let Err(error) = tx.execute(
            "INSERT INTO records VALUES(?1,?2,?3)",
            rusqlite::params![ordinal as i64, entry.id().as_str(), payload.as_slice()],
        ) {
            return Err(AdmissionFailure {
                // A failed fixed INSERT can cause SQLite to roll the transaction
                // back automatically (e.g. SQLITE_FULL). No COMMIT ran here.
                completion: if tx.is_autocommit() || tx.rollback().is_ok() {
                    Completion::RolledBack
                } else {
                    Completion::Indeterminate
                },
                error: sql(error),
            });
        }
        observe(Stage::BeforeCommit, &tx);
        tx.commit().map_err(|e| AdmissionFailure {
            completion: Completion::Indeterminate,
            error: sql(e),
        })?;
        observe(Stage::AfterCommit, &conn);
        Ok(AdmissionReceipt {
            store: self.id,
            ordinal,
        })
    }

    /// Compare exact intended context/record after recovery; errors never imply absence.
    pub fn reconcile(
        &self,
        key: &StoreKey,
        context: &CandidateObservation,
        entry: &CanonicalRecord,
        access: Access,
    ) -> Result<Reconciliation, StoreError> {
        let h = self.load(key, access)?;
        if h.context() != context {
            return Ok(Reconciliation::Conflict);
        }
        for (i, r) in h.records().iter().enumerate() {
            if r.id() == entry.id() {
                return Ok(if r == entry {
                    Reconciliation::Present(AdmissionReceipt {
                        store: self.id,
                        ordinal: (i + 1) as u64,
                    })
                } else {
                    Reconciliation::Conflict
                });
            }
        }
        Ok(Reconciliation::Absent)
    }

    /// Remove all managed payload files under explicit whole-collection scope.
    ///
    /// The empty lock/directory remain. Disk sectors, backups, swap and caller-held
    /// snapshots are not erased. An interruption leaves a minimal random-ID intent.
    pub fn remove(
        &self,
        key: &StoreKey,
        request: Option<RemovalRequest>,
    ) -> Result<RemovalReceipt, StoreError> {
        self.remove_observed(key, request, |_| {})
    }
    fn remove_observed(
        &self,
        key: &StoreKey,
        request: Option<RemovalRequest>,
        mut observe: impl FnMut(Stage),
    ) -> Result<RemovalReceipt, StoreError> {
        if request.map(|r| r.0) != Some(self.id) {
            return Err(StoreError::new(ErrorKind::AuthorizationRequired));
        }
        let _guard = self.location.lock()?;
        self.location.files()?;
        let marker = self.location.intent()?;
        if let Some(id) = marker
            && id != self.id
        {
            return Err(StoreError::new(ErrorKind::TargetChanged));
        }
        if self.location.database_exists()? {
            let conn = connect(&self.location, key, Access::Recover)?;
            let (id, _) = identity(&conn)?;
            if id != self.id {
                return Err(StoreError::new(ErrorKind::TargetChanged));
            }
        } else if marker.is_none() {
            return Err(StoreError::new(ErrorKind::Missing));
        }
        if marker.is_none() {
            self.location
                .write_intent(self.id)
                .map_err(|_| StoreError::new(ErrorKind::RemovalIncomplete))?;
        }
        observe(Stage::RemovalIntent);
        for file in [DATABASE, JOURNAL] {
            self.location
                .unlink(file)
                .map_err(|_| StoreError::new(ErrorKind::RemovalIncomplete))?;
            observe(Stage::RemovalFile);
        }
        self.location
            .sync()
            .map_err(|_| StoreError::new(ErrorKind::RemovalIncomplete))?;
        self.location
            .unlink(REMOVING)
            .map_err(|_| StoreError::new(ErrorKind::RemovalIncomplete))?;
        self.location
            .sync()
            .map_err(|_| StoreError::new(ErrorKind::RemovalIncomplete))?;
        Ok(RemovalReceipt { store: self.id })
    }
    /// Resume only an existing matching removal intent; never restore missing payloads.
    pub fn resume_removal(
        path: &Path,
        key: &StoreKey,
        request: RemovalRequest,
    ) -> Result<RemovalReceipt, StoreError> {
        let location = Location::open(path)?;
        {
            let _guard = location.lock()?;
            if location.intent()? != Some(request.0) {
                return Err(StoreError::new(ErrorKind::AuthorizationRequired));
            }
        }
        Self {
            location,
            id: request.0,
        }
        .remove(key, Some(request))
    }
}

#[derive(Clone, Copy)]
enum Stage {
    BeforeWrite,
    BeforeCommit,
    AfterCommit,
    RemovalIntent,
    RemovalFile,
}

fn connect(location: &Location, key: &StoreKey, access: Access) -> Result<Connection, StoreError> {
    location.files()?;
    location.check_database()?;
    if access == Access::Recover
        && std::os::unix::fs::MetadataExt::mode(&fs::metadata(location.path().join(DATABASE))?)
            & 0o200
            == 0
    {
        return Err(StoreError::new(ErrorKind::Permission));
    }
    if access == Access::ReadOnly && location.journal_exists()? {
        return Err(StoreError::new(ErrorKind::RecoveryRequired));
    }
    let flags = OpenFlags::SQLITE_OPEN_NOFOLLOW
        | OpenFlags::SQLITE_OPEN_NO_MUTEX
        | match access {
            Access::ReadOnly => OpenFlags::SQLITE_OPEN_READ_ONLY,
            Access::Recover => OpenFlags::SQLITE_OPEN_READ_WRITE,
        };
    let c = Connection::open_with_flags(location.path().join(DATABASE), flags).map_err(sql)?;
    // Only hex digits from a fixed-size raw key enter this statement. rusqlite's
    // pragma helper reads pragma_pragma_list and must not run before keying.
    c.execute_batch("PRAGMA cipher_log_level=NONE;")
        .map_err(sql)?;
    let mut statement = Zeroizing::new(String::from("PRAGMA key=\"x'"));
    use std::fmt::Write;
    for byte in key.0.iter() {
        write!(&mut *statement, "{byte:02x}")
            .map_err(|_| StoreError::new(ErrorKind::InvalidKey))?;
    }
    statement.push_str("'\";");
    c.execute_batch(&statement).map_err(unlock)?;
    c.busy_timeout(Duration::ZERO).map_err(sql)?;
    c.set_limit(Limit::SQLITE_LIMIT_LENGTH, MAX_BYTES as i32)
        .map_err(sql)?;
    c.execute_batch("PRAGMA cipher_compatibility=4; PRAGMA temp_store=MEMORY; PRAGMA cipher_memory_security=ON;")
        .map_err(sql)?;
    let cipher: String = c
        .query_row("PRAGMA cipher_version", [], |r| r.get(0))
        .map_err(|_| StoreError::new(ErrorKind::Unsupported))?;
    if cipher != "4.19.0 community" {
        return Err(StoreError::new(ErrorKind::Unsupported));
    }
    c.query_row("SELECT count(*) FROM sqlite_schema", [], |r| {
        r.get::<_, i64>(0)
    })
    .map_err(unlock)?;
    let initial_mode: String = c
        .query_row("PRAGMA journal_mode", [], |r| r.get(0))
        .map_err(sql)?;
    if initial_mode != "delete" {
        return Err(StoreError::new(ErrorKind::Unsupported));
    }
    let hmac: String = c
        .query_row("PRAGMA cipher_use_hmac", [], |r| r.get(0))
        .map_err(sql)?;
    let provider: String = c
        .query_row("PRAGMA cipher_provider", [], |r| r.get(0))
        .map_err(sql)?;
    let provider_version: String = c
        .query_row("PRAGMA cipher_provider_version", [], |r| r.get(0))
        .map_err(sql)?;
    if hmac != "1"
        || provider != "openssl"
        || !provider_version.starts_with("OpenSSL 3.")
        || ["beta", "alpha", "-dev"]
            .iter()
            .any(|v| provider_version.to_ascii_lowercase().contains(v))
    {
        return Err(StoreError::new(ErrorKind::Unsupported));
    }
    if access == Access::Recover {
        c.execute_batch("PRAGMA journal_mode=DELETE; PRAGMA synchronous=EXTRA; PRAGMA secure_delete=ON; PRAGMA max_page_count=16384;").map_err(sql)?;
    }
    let mode: String = c
        .query_row("PRAGMA journal_mode", [], |r| r.get(0))
        .map_err(sql)?;
    let temp: i64 = c
        .query_row("PRAGMA temp_store", [], |r| r.get(0))
        .map_err(sql)?;
    let header: String = c
        .query_row("PRAGMA cipher_plaintext_header_size", [], |r| r.get(0))
        .map_err(sql)?;
    let page: String = c
        .query_row("PRAGMA page_size", [], |r| r.get(0))
        .map_err(sql)?;
    if mode != "delete" || temp != 2 || header != "0" || page != "4096" {
        return Err(StoreError::new(ErrorKind::Unsupported));
    }
    if access == Access::Recover
        && c.query_row("PRAGMA synchronous", [], |r| r.get::<_, i64>(0))
            .map_err(sql)?
            != 3
    {
        return Err(StoreError::new(ErrorKind::Unsupported));
    }
    Ok(c)
}

fn identity(c: &Connection) -> Result<(StoreId, Vec<u8>), StoreError> {
    for (name, definition) in [("meta", META_SCHEMA), ("records", RECORD_SCHEMA)] {
        let stored: String = c
            .query_row(
                "SELECT sql FROM sqlite_schema WHERE type='table' AND name=?1",
                [name],
                |r| r.get(0),
            )
            .map_err(|_| StoreError::new(ErrorKind::Corrupt))?;
        if stored != definition {
            return Err(StoreError::new(ErrorKind::Corrupt));
        }
    }
    let mut schema = c
        .prepare("SELECT name FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%' ORDER BY name")
        .map_err(|_| StoreError::new(ErrorKind::Corrupt))?;
    let names = schema
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(sql)?
        .take(3)
        .collect::<Result<Vec<_>, _>>()
        .map_err(sql)?;
    if names != ["meta", "records"] {
        return Err(StoreError::new(ErrorKind::Corrupt));
    }
    let policy: String = c
        .query_row("SELECT policy FROM meta WHERE singleton=1", [], |r| {
            r.get(0)
        })
        .map_err(|_| StoreError::new(ErrorKind::Corrupt))?;
    if policy != POLICY {
        return Err(StoreError::new(ErrorKind::Unsupported));
    }
    if c.query_row("SELECT count(*) FROM meta", [], |r| r.get::<_, i64>(0))
        .map_err(sql)?
        != 1
    {
        return Err(StoreError::new(ErrorKind::Corrupt));
    }
    let (format, id, context) = c
        .query_row(
            "SELECT format,instance,context FROM meta WHERE singleton=1",
            [],
            |r| {
                let id = r.get_ref(1)?.as_blob()?;
                let context = r.get_ref(2)?.as_blob()?;
                if id.len() != 16 || context.len() > MAX_BYTES {
                    return Err(rusqlite::Error::InvalidQuery);
                }
                Ok((r.get::<_, i64>(0)?, id.to_vec(), context.to_vec()))
            },
        )
        .map_err(|_| StoreError::new(ErrorKind::Corrupt))?;
    if format != FORMAT {
        return Err(StoreError::new(ErrorKind::Unsupported));
    }
    let id = StoreId(
        id.try_into()
            .map_err(|_| StoreError::new(ErrorKind::Corrupt))?,
    );
    Ok((id, context))
}
fn load(c: &Connection) -> Result<(StoreId, CanonicalHistory), StoreError> {
    let integrity: String = c
        .query_row("PRAGMA quick_check", [], |r| r.get(0))
        .map_err(|_| StoreError::new(ErrorKind::Corrupt))?;
    if integrity != "ok" {
        return Err(StoreError::new(ErrorKind::Corrupt));
    }
    let (id, context) = identity(c)?;
    let mut history = CanonicalHistory::new(codec::decode_context(&context)?);
    let mut statement = c
        .prepare("SELECT ordinal,version,payload FROM records ORDER BY ordinal")
        .map_err(sql)?;
    let mut rows = statement.query([]).map_err(sql)?;
    while let Some(row) = rows.next().map_err(sql)? {
        if history.records().len() == MAX_RECORD_VERSIONS {
            return Err(StoreError::new(ErrorKind::Corrupt));
        }
        let ordinal: i64 = row.get(0).map_err(sql)?;
        if ordinal != (history.records().len() + 1) as i64 {
            return Err(StoreError::new(ErrorKind::Corrupt));
        }
        let payload = row
            .get_ref(2)
            .map_err(sql)?
            .as_blob()
            .map_err(|_| StoreError::new(ErrorKind::Corrupt))?;
        let record = codec::decode_record(payload).map_err(|e| {
            if e.kind() == ErrorKind::Domain {
                StoreError::new(ErrorKind::Corrupt)
            } else {
                e
            }
        })?;
        let name: &str = row
            .get_ref(1)
            .map_err(sql)?
            .as_str()
            .map_err(|_| StoreError::new(ErrorKind::Corrupt))?;
        if name != record.id().as_str() {
            return Err(StoreError::new(ErrorKind::Corrupt));
        }
        history
            .append(record)
            .map_err(|_| StoreError::new(ErrorKind::Corrupt))?;
    }
    Ok((id, history))
}

#[cfg(test)]
mod tests;
